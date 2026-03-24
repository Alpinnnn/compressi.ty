use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    modules::compress_videos::models::{
        CodecChoice, CompressionEstimate, CompressionMode, CompressionRecommendation,
        CompressionResult, EncoderAvailability, EncoderBackend, EngineInfo, PreviewResult,
        ProcessingProgress, ResolutionChoice, ResolvedEncoder, SizeSliderRange, VideoMetadata,
        VideoSettings,
    },
    runtime,
};

const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "m4v"];

/// Events emitted by preview and compression jobs.
#[derive(Clone, Debug, PartialEq)]
pub enum JobEvent {
    Progress(ProcessingProgress),
    PreviewReady(PreviewResult),
    CompressionFinished(CompressionResult),
    Failed(String),
    Cancelled,
}

/// Handle for a background FFmpeg job.
pub struct JobHandle {
    pub receiver: Receiver<JobEvent>,
    cancel_flag: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl JobHandle {
    /// Cancels the running job and terminates the underlying FFmpeg process.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);

        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
            }
        }
    }
}

#[derive(Clone)]
struct EncodePlan {
    encoder: ResolvedEncoder,
    video_bitrate_kbps: u32,
    audio_bitrate_kbps: Option<u32>,
    crf: Option<u8>,
    preset: Option<String>,
    output_width: u32,
    output_height: u32,
    output_fps: f32,
    pass_count: u8,
}

#[derive(Clone, Copy)]
struct ProgressWeight {
    start: f32,
    span: f32,
}

/// Returns true when the path looks like a supported video file.
pub fn is_supported_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .map(|ext| VIDEO_EXTENSIONS.iter().any(|known| *known == ext))
        .unwrap_or(false)
}

/// Reads metadata for the selected video through ffprobe.
pub fn probe_video(engine: &EngineInfo, path: PathBuf) -> Result<VideoMetadata, String> {
    let mut command = background_command(&engine.ffprobe_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg(
            "format=duration,size,bit_rate:stream=index,codec_type,codec_name,width,height,r_frame_rate,avg_frame_rate,bit_rate",
        )
        .arg("-of")
        .arg("flat=s=_")
        .arg(&path);

    let output = run_capture(command)?;

    parse_ffprobe_output(path, &output)
}

/// Computes the adaptive range used by the target size slider.
pub fn size_slider_range(video: &VideoMetadata) -> SizeSliderRange {
    let original_mb = video.original_size_mb().max(6);
    let min_mb =
        ((original_mb as f32 * 0.08).round() as u32).clamp(4, original_mb.saturating_sub(1).max(4));
    let max_mb =
        ((original_mb as f32 * 0.85).round() as u32).clamp(min_mb + 1, original_mb.max(min_mb + 1));
    let recommended_mb =
        ((original_mb as f32 * recommendation_ratio(video)).round() as u32).clamp(min_mb, max_mb);

    SizeSliderRange {
        min_mb,
        max_mb,
        recommended_mb,
    }
}

/// Builds the live estimate that powers the summary cards.
pub fn estimate_output(
    video: &VideoMetadata,
    settings: &VideoSettings,
    encoders: &EncoderAvailability,
) -> CompressionEstimate {
    let plan = build_plan(video, settings, encoders, false);
    let estimated_size_bytes = estimate_size_bytes(video.duration_secs, &plan);
    // Allow negative values so the UI can show "X% larger" when the output exceeds the original
    let savings_percent = if video.size_bytes == 0 {
        0.0
    } else {
        100.0 - (estimated_size_bytes as f32 / video.size_bytes as f32 * 100.0)
    };
    let recommendation = build_recommendation(video, size_slider_range(video));

    CompressionEstimate {
        original_size_bytes: video.size_bytes,
        estimated_size_bytes,
        estimated_time_secs: estimate_processing_time(video, &plan),
        savings_percent,
        target_width: plan.output_width,
        target_height: plan.output_height,
        pass_count: plan.pass_count,
        recommendation,
    }
}

/// Starts generation of a 5 second source-vs-result preview.
pub fn start_preview(
    engine: EngineInfo,
    video: VideoMetadata,
    settings: VideoSettings,
) -> Result<JobHandle, String> {
    let preview_dir = preview_dir()?;
    fs::create_dir_all(&preview_dir).map_err(|error| {
        format!(
            "Could not create preview folder {}: {error}",
            preview_dir.display()
        )
    })?;

    let original_path = preview_dir.join(build_output_name(&video.path, "preview-source", "mp4"));
    let compressed_path = preview_dir.join(build_output_name(&video.path, "preview-result", "mp4"));
    let start_secs = preview_start(video.duration_secs);

    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let child = Arc::new(Mutex::new(None));

    let thread_cancel = Arc::clone(&cancel_flag);
    let thread_child = Arc::clone(&child);

    thread::spawn(move || {
        let source_result = run_source_preview(
            &engine,
            &video,
            start_secs,
            &original_path,
            ProgressWeight {
                start: 0.0,
                span: 0.35,
            },
            &thread_cancel,
            &thread_child,
            &sender,
        );

        if matches!(source_result, Err(ref error) if error == "cancelled") {
            let _ = sender.send(JobEvent::Cancelled);
            return;
        }

        if let Err(error) = source_result {
            let _ = sender.send(JobEvent::Failed(error));
            return;
        }

        let plan = build_plan(&video, &settings, &engine.encoders, true);
        let encode_result = run_encode_pass(
            build_encode_command(
                &engine.ffmpeg_path,
                &video,
                &plan,
                start_secs,
                5.0,
                &compressed_path,
                None,
                false,
                true,
            ),
            video.duration_secs.min(5.0),
            ProgressWeight {
                start: 0.35,
                span: 0.65,
            },
            "Rendering preview",
            &thread_cancel,
            &thread_child,
            &sender,
        );

        if matches!(encode_result, Err(ref error) if error == "cancelled") {
            let _ = sender.send(JobEvent::Cancelled);
            return;
        }

        if let Err(error) = encode_result {
            let _ = sender.send(JobEvent::Failed(error));
            return;
        }

        let result = (|| -> Result<PreviewResult, String> {
            let original_size_bytes = fs::metadata(&original_path)
                .map_err(|error| format!("Could not read preview file: {error}"))?
                .len();
            let compressed_size_bytes = fs::metadata(&compressed_path)
                .map_err(|error| format!("Could not read preview file: {error}"))?
                .len();

            Ok(PreviewResult {
                original_clip_path: original_path,
                compressed_clip_path: compressed_path,
                original_size_bytes,
                compressed_size_bytes,
            })
        })();

        match result {
            Ok(result) => {
                let _ = sender.send(JobEvent::PreviewReady(result));
            }
            Err(error) => {
                let _ = sender.send(JobEvent::Failed(error));
            }
        }
    });

    Ok(JobHandle {
        receiver,
        cancel_flag,
        child,
    })
}

/// Starts the final compression job for the selected video.
pub fn start_compression(
    engine: EngineInfo,
    video: VideoMetadata,
    settings: VideoSettings,
    base_output_dir: Option<PathBuf>,
) -> Result<JobHandle, String> {
    let output_dir = resolve_output_dir(base_output_dir)?;
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "Could not create output folder {}: {error}",
            output_dir.display()
        )
    })?;

    let output_path = build_unique_output_path(&output_dir, &video.path, "compressed", "mp4");
    let plan = build_plan(&video, &settings, &engine.encoders, false);
    let passlog = output_dir.join(build_output_name(&video.path, "twopass", "log"));

    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let child = Arc::new(Mutex::new(None));

    let thread_cancel = Arc::clone(&cancel_flag);
    let thread_child = Arc::clone(&child);

    thread::spawn(move || {
        let started_at = Instant::now();

        if plan.pass_count == 2 {
            let first_result = run_encode_pass(
                build_encode_command(
                    &engine.ffmpeg_path,
                    &video,
                    &plan,
                    0.0,
                    video.duration_secs,
                    null_output_path(),
                    Some(&passlog),
                    true,
                    false,
                ),
                video.duration_secs,
                ProgressWeight {
                    start: 0.0,
                    span: 0.48,
                },
                "Analyzing video",
                &thread_cancel,
                &thread_child,
                &sender,
            );

            if matches!(first_result, Err(ref error) if error == "cancelled") {
                cleanup_passlog(&passlog);
                let _ = sender.send(JobEvent::Cancelled);
                return;
            }

            if let Err(error) = first_result {
                cleanup_passlog(&passlog);
                let _ = sender.send(JobEvent::Failed(error));
                return;
            }
        }

        let second_result = run_encode_pass(
            build_encode_command(
                &engine.ffmpeg_path,
                &video,
                &plan,
                0.0,
                video.duration_secs,
                &output_path,
                (plan.pass_count == 2).then_some(&passlog),
                false,
                false,
            ),
            video.duration_secs,
            if plan.pass_count == 2 {
                ProgressWeight {
                    start: 0.48,
                    span: 0.52,
                }
            } else {
                ProgressWeight {
                    start: 0.0,
                    span: 1.0,
                }
            },
            "Compressing video",
            &thread_cancel,
            &thread_child,
            &sender,
        );

        cleanup_passlog(&passlog);

        if matches!(second_result, Err(ref error) if error == "cancelled") {
            let _ = sender.send(JobEvent::Cancelled);
            return;
        }

        if let Err(error) = second_result {
            let _ = sender.send(JobEvent::Failed(error));
            return;
        }

        let result = (|| -> Result<CompressionResult, String> {
            let output_size_bytes = fs::metadata(&output_path)
                .map_err(|error| format!("Could not read compressed file: {error}"))?
                .len();
            let reduction_percent = if video.size_bytes == 0 {
                0.0
            } else {
                100.0 - (output_size_bytes as f32 / video.size_bytes as f32 * 100.0)
            };

            Ok(CompressionResult {
                output_path,
                original_size_bytes: video.size_bytes,
                output_size_bytes,
                reduction_percent,
                elapsed_secs: started_at.elapsed().as_secs_f32(),
            })
        })();

        match result {
            Ok(result) => {
                let _ = sender.send(JobEvent::CompressionFinished(result));
            }
            Err(error) => {
                let _ = sender.send(JobEvent::Failed(error));
            }
        }
    });

    Ok(JobHandle {
        receiver,
        cancel_flag,
        child,
    })
}

// ─── Batch compression API ──────────────────────────────────────────────────

/// Events emitted during batch video compression.
#[derive(Clone, Debug)]
pub enum BatchEvent {
    VideoStarted {
        id: u64,
    },
    VideoProgress {
        id: u64,
        progress: ProcessingProgress,
    },
    VideoFinished {
        id: u64,
        result: CompressionResult,
    },
    VideoFailed {
        id: u64,
        error: String,
    },
    BatchFinished {
        cancelled: bool,
    },
}

/// Handle for a running batch video compression job.
pub struct BatchHandle {
    pub receiver: Receiver<BatchEvent>,
    pub output_dir: PathBuf,
    cancel_flag: Arc<AtomicBool>,
}

impl BatchHandle {
    /// Signal that the batch should be cancelled.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

/// A single item to compress in a batch.
pub struct BatchItem {
    pub id: u64,
    pub video: VideoMetadata,
    pub settings: VideoSettings,
}

/// Starts sequential compression of multiple videos in a background thread.
pub fn start_video_batch(
    engine: EngineInfo,
    items: Vec<BatchItem>,
    base_output_dir: Option<PathBuf>,
) -> Result<BatchHandle, String> {
    let output_dir = resolve_output_dir(base_output_dir)?;
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "Could not create output folder {}: {error}",
            output_dir.display()
        )
    })?;

    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let thread_cancel = Arc::clone(&cancel_flag);
    let thread_output_dir = output_dir.clone();

    thread::spawn(move || {
        for item in &items {
            if thread_cancel.load(Ordering::Relaxed) {
                let _ = sender.send(BatchEvent::BatchFinished { cancelled: true });
                return;
            }

            let _ = sender.send(BatchEvent::VideoStarted { id: item.id });

            let result = compress_single_video(
                &engine,
                &item.video,
                &item.settings,
                &thread_output_dir,
                &thread_cancel,
                item.id,
                &sender,
            );

            match result {
                Ok(compression_result) => {
                    let _ = sender.send(BatchEvent::VideoFinished {
                        id: item.id,
                        result: compression_result,
                    });
                }
                Err(error) => {
                    if error == "cancelled" {
                        let _ = sender.send(BatchEvent::BatchFinished { cancelled: true });
                        return;
                    }
                    let _ = sender.send(BatchEvent::VideoFailed { id: item.id, error });
                }
            }
        }

        let _ = sender.send(BatchEvent::BatchFinished { cancelled: false });
    });

    Ok(BatchHandle {
        receiver,
        output_dir,
        cancel_flag,
    })
}

/// Compresses a single video inside a batch context. Returns Ok(result) or Err(error).
fn compress_single_video(
    engine: &EngineInfo,
    video: &VideoMetadata,
    settings: &VideoSettings,
    output_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
    id: u64,
    batch_sender: &mpsc::Sender<BatchEvent>,
) -> Result<CompressionResult, String> {
    let output_path = build_unique_output_path(output_dir, &video.path, "compressed", "mp4");
    let plan = build_plan(video, settings, &engine.encoders, false);
    let passlog = output_dir.join(build_output_name(&video.path, "twopass", "log"));

    let (job_sender, job_receiver) = mpsc::channel::<JobEvent>();
    let child = Arc::new(Mutex::new(None));
    let started_at = Instant::now();

    // Forward progress events from this single job to the batch sender
    let batch_tx = batch_sender.clone();
    let progress_cancel = Arc::clone(cancel_flag);
    let progress_thread = thread::spawn(move || {
        while let Ok(event) = job_receiver.recv() {
            if progress_cancel.load(Ordering::Relaxed) {
                break;
            }
            if let JobEvent::Progress(p) = event {
                let _ = batch_tx.send(BatchEvent::VideoProgress { id, progress: p });
            }
        }
    });

    if plan.pass_count == 2 {
        let first_result = run_encode_pass(
            build_encode_command(
                &engine.ffmpeg_path,
                video,
                &plan,
                0.0,
                video.duration_secs,
                null_output_path(),
                Some(&passlog),
                true,
                false,
            ),
            video.duration_secs,
            ProgressWeight {
                start: 0.0,
                span: 0.48,
            },
            "Analyzing video",
            cancel_flag,
            &child,
            &job_sender,
        );

        if matches!(first_result, Err(ref e) if e == "cancelled") {
            cleanup_passlog(&passlog);
            drop(job_sender);
            let _ = progress_thread.join();
            return Err("cancelled".to_owned());
        }

        if let Err(error) = first_result {
            cleanup_passlog(&passlog);
            drop(job_sender);
            let _ = progress_thread.join();
            return Err(error);
        }
    }

    let second_result = run_encode_pass(
        build_encode_command(
            &engine.ffmpeg_path,
            video,
            &plan,
            0.0,
            video.duration_secs,
            &output_path,
            (plan.pass_count == 2).then_some(&passlog),
            false,
            false,
        ),
        video.duration_secs,
        if plan.pass_count == 2 {
            ProgressWeight {
                start: 0.48,
                span: 0.52,
            }
        } else {
            ProgressWeight {
                start: 0.0,
                span: 1.0,
            }
        },
        "Compressing video",
        cancel_flag,
        &child,
        &job_sender,
    );

    cleanup_passlog(&passlog);
    drop(job_sender);
    let _ = progress_thread.join();

    if matches!(second_result, Err(ref e) if e == "cancelled") {
        return Err("cancelled".to_owned());
    }

    second_result?;

    let output_size_bytes = fs::metadata(&output_path)
        .map_err(|error| format!("Could not read compressed file: {error}"))?
        .len();
    let reduction_percent = if video.size_bytes == 0 {
        0.0
    } else {
        100.0 - (output_size_bytes as f32 / video.size_bytes as f32 * 100.0)
    };

    Ok(CompressionResult {
        output_path,
        original_size_bytes: video.size_bytes,
        output_size_bytes,
        reduction_percent,
        elapsed_secs: started_at.elapsed().as_secs_f32(),
    })
}

fn parse_ffprobe_output(path: PathBuf, output: &str) -> Result<VideoMetadata, String> {
    let mut format_values = BTreeMap::<String, String>::new();
    let mut streams = BTreeMap::<usize, BTreeMap<String, String>>::new();

    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };

        let key = raw_key.trim();
        let value = raw_value.trim().trim_matches('"').to_owned();

        if let Some(rest) = key.strip_prefix("streams_stream_") {
            let Some((index, field)) = rest.split_once('_') else {
                continue;
            };
            let Ok(index) = index.parse::<usize>() else {
                continue;
            };
            streams
                .entry(index)
                .or_default()
                .insert(field.to_owned(), value);
        } else if let Some(field) = key.strip_prefix("format_") {
            format_values.insert(field.to_owned(), value);
        }
    }

    let video_stream = streams
        .values()
        .find(|stream| stream.get("codec_type").map(String::as_str) == Some("video"))
        .ok_or_else(|| "The selected file does not contain a video stream.".to_owned())?;
    let audio_stream = streams
        .values()
        .find(|stream| stream.get("codec_type").map(String::as_str) == Some("audio"));

    let width = parse_u32(video_stream.get("width")).unwrap_or(0);
    let height = parse_u32(video_stream.get("height")).unwrap_or(0);
    let duration_secs = parse_f32(format_values.get("duration")).unwrap_or(0.0);
    let fps = parse_ratio(
        video_stream
            .get("avg_frame_rate")
            .or_else(|| video_stream.get("r_frame_rate")),
    )
    .unwrap_or(30.0)
    .max(1.0);

    if width == 0 || height == 0 || duration_secs <= 0.0 {
        return Err("The selected file could not be analyzed correctly.".to_owned());
    }

    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("video")
        .to_owned();

    Ok(VideoMetadata {
        path,
        file_name,
        size_bytes: parse_u64(format_values.get("size")).unwrap_or(0),
        duration_secs,
        width,
        height,
        fps,
        container_bitrate_kbps: parse_u32(format_values.get("bit_rate")).map(|value| value / 1000),
        video_bitrate_kbps: parse_u32(video_stream.get("bit_rate")).map(|value| value / 1000),
        audio_bitrate_kbps: audio_stream
            .and_then(|stream| parse_u32(stream.get("bit_rate")))
            .map(|value| value / 1000),
        video_codec: video_stream
            .get("codec_name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_owned()),
        has_audio: audio_stream.is_some(),
    })
}

fn build_recommendation(
    video: &VideoMetadata,
    range: SizeSliderRange,
) -> Option<CompressionRecommendation> {
    if video.original_size_mb() <= 8 {
        return None;
    }

    let target_size_mb = range.recommended_mb;
    let saving_percent = if video.original_size_mb() == 0 {
        0.0
    } else {
        100.0 - (target_size_mb as f32 / video.original_size_mb() as f32 * 100.0)
    };

    Some(CompressionRecommendation {
        headline: format!("Recommended: Reduce to about {target_size_mb} MB"),
        detail: format!(
            "Save about {:.0}% for easier sharing.",
            saving_percent.max(0.0)
        ),
        mode: CompressionMode::ReduceSize,
        target_size_mb,
    })
}

fn recommendation_ratio(video: &VideoMetadata) -> f32 {
    if video.duration_secs > 300.0 || video.height > 1440 {
        0.28
    } else if video.duration_secs > 120.0 {
        0.24
    } else {
        0.20
    }
}

fn build_plan(
    video: &VideoMetadata,
    settings: &VideoSettings,
    encoders: &EncoderAvailability,
    preview_mode: bool,
) -> EncodePlan {
    let codec = match settings.mode {
        CompressionMode::ReduceSize => encoders.reduce_size_codec(),
        CompressionMode::GoodQuality => encoders.quality_codec(),
        CompressionMode::CustomAdvanced => {
            if encoders.supports(settings.custom_codec) {
                settings.custom_codec
            } else {
                encoders.fallback_codec()
            }
        }
    };
    let encoder = encoders.resolved_encoder(codec);

    let resolution_choice = match settings.mode {
        CompressionMode::ReduceSize => reduce_size_resolution(video, settings.target_size_mb),
        CompressionMode::GoodQuality => settings.resolution,
        CompressionMode::CustomAdvanced => settings.resolution,
    };
    let (output_width, output_height) = resolve_dimensions(video, resolution_choice);
    let output_fps = resolve_fps(video, settings);

    match settings.mode {
        CompressionMode::ReduceSize => {
            let total_kbps = target_total_bitrate(settings.target_size_mb, video.duration_secs);
            let audio_bitrate_kbps = video.has_audio.then_some(aggressive_audio_bitrate(video));
            let video_bitrate_kbps = total_kbps
                .saturating_sub(audio_bitrate_kbps.unwrap_or(0))
                .clamp(220, 50_000);

            EncodePlan {
                encoder,
                video_bitrate_kbps,
                audio_bitrate_kbps,
                crf: None,
                preset: encoder_preset(encoder, preview_mode, true),
                output_width,
                output_height,
                output_fps,
                pass_count: if preview_mode || encoder.is_hardware() {
                    1
                } else {
                    2
                },
            }
        }
        CompressionMode::GoodQuality => {
            let audio_bitrate_kbps = video.has_audio.then_some(quality_audio_bitrate(video));
            let crf = quality_to_crf(settings.quality, codec);
            let video_bitrate_kbps =
                quality_estimated_bitrate(video, settings, codec, output_width, output_height);

            EncodePlan {
                encoder,
                video_bitrate_kbps,
                audio_bitrate_kbps,
                crf: if encoder.is_hardware() {
                    None
                } else {
                    Some(crf)
                },
                preset: encoder_preset(encoder, preview_mode, false),
                output_width,
                output_height,
                output_fps,
                pass_count: 1,
            }
        }
        CompressionMode::CustomAdvanced => {
            let audio_bitrate_kbps = if video.has_audio && settings.custom_audio_enabled {
                Some(settings.custom_audio_bitrate_kbps.clamp(64, 320))
            } else {
                None
            };

            EncodePlan {
                encoder,
                video_bitrate_kbps: settings.custom_bitrate_kbps.clamp(350, 80_000),
                audio_bitrate_kbps,
                crf: None,
                preset: encoder_preset(encoder, preview_mode, false),
                output_width,
                output_height,
                output_fps,
                pass_count: 1,
            }
        }
    }
}

fn build_encode_command(
    ffmpeg_binary: &Path,
    video: &VideoMetadata,
    plan: &EncodePlan,
    start_secs: f32,
    clip_duration_secs: f32,
    output_path: &Path,
    passlog: Option<&Path>,
    first_pass: bool,
    preview_mode: bool,
) -> Command {
    let mut command = background_command(ffmpeg_binary);
    command
        .arg("-hide_banner")
        .arg("-y")
        .arg("-nostdin")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-stats_period")
        .arg("0.25");

    if start_secs > 0.0 {
        command.arg("-ss").arg(format_time_arg(start_secs));
    }

    command.arg("-i").arg(&video.path);

    if clip_duration_secs > 0.0 {
        command.arg("-t").arg(format_time_arg(clip_duration_secs));
    }

    command.arg("-map").arg("0:v:0");
    if video.has_audio && !first_pass {
        command.arg("-map").arg("0:a?");
    }

    command.arg("-sn").arg("-dn");

    let filter = build_filter_chain(video, plan);
    if !filter.is_empty() {
        command.arg("-vf").arg(filter);
    }

    command.arg("-c:v").arg(plan.encoder.ffmpeg_name());
    if let Some(preset) = &plan.preset {
        command.arg("-preset").arg(preset);
    }
    command.arg("-pix_fmt").arg("yuv420p");
    command.arg("-movflags").arg("+faststart");
    command.arg("-g").arg("240");

    match plan.crf {
        Some(crf) => {
            command.arg("-crf").arg(crf.to_string());
        }
        None => {
            command
                .arg("-b:v")
                .arg(format!("{}k", plan.video_bitrate_kbps))
                .arg("-maxrate")
                .arg(format!("{}k", plan.video_bitrate_kbps))
                .arg("-bufsize")
                .arg(format!("{}k", plan.video_bitrate_kbps.saturating_mul(2)));
        }
    }

    if let Some(passlog) = passlog {
        command.arg("-passlogfile").arg(passlog);
        command.arg("-pass").arg(if first_pass { "1" } else { "2" });
    }

    match plan.encoder.codec {
        CodecChoice::H264 => {}
        CodecChoice::H265 => {
            command.arg("-tag:v").arg("hvc1");
        }
        CodecChoice::Av1 => {
            if matches!(plan.encoder.backend, EncoderBackend::Software) {
                command.arg("-svtav1-params").arg("tune=0");
            }
        }
    }

    if first_pass {
        command.arg("-an");
        command.arg("-f").arg("mp4");
    } else if let Some(audio_bitrate_kbps) = plan.audio_bitrate_kbps {
        command
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg(format!("{}k", audio_bitrate_kbps))
            .arg("-ac")
            .arg("2");
    } else {
        command.arg("-an");
    }

    if preview_mode {
        command.arg("-map_metadata").arg("-1");
    }

    command.arg(output_path);
    command
}

fn run_source_preview(
    engine: &EngineInfo,
    video: &VideoMetadata,
    start_secs: f32,
    output_path: &Path,
    weight: ProgressWeight,
    cancel_flag: &AtomicBool,
    shared_child: &Arc<Mutex<Option<Child>>>,
    sender: &mpsc::Sender<JobEvent>,
) -> Result<(), String> {
    let mut command = background_command(&engine.ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-y")
        .arg("-nostdin")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-stats_period")
        .arg("0.25");

    if start_secs > 0.0 {
        command.arg("-ss").arg(format_time_arg(start_secs));
    }

    command
        .arg("-i")
        .arg(&video.path)
        .arg("-t")
        .arg("5")
        .arg("-map")
        .arg("0:v:0");

    if video.has_audio {
        command.arg("-map").arg("0:a?");
    }

    command
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("ultrafast")
        .arg("-crf")
        .arg("18")
        .arg("-pix_fmt")
        .arg("yuv420p");

    if video.has_audio {
        command
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("128k")
            .arg("-ac")
            .arg("2");
    } else {
        command.arg("-an");
    }

    command.arg(output_path);

    run_encode_pass(
        command,
        video.duration_secs.min(5.0),
        weight,
        "Preparing source preview",
        cancel_flag,
        shared_child,
        sender,
    )
}

fn run_encode_pass(
    mut command: Command,
    total_duration_secs: f32,
    weight: ProgressWeight,
    stage: &str,
    cancel_flag: &AtomicBool,
    shared_child: &Arc<Mutex<Option<Child>>>,
    sender: &mpsc::Sender<JobEvent>,
) -> Result<(), String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start FFmpeg: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not read FFmpeg progress.".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not read FFmpeg errors.".to_owned())?;

    if let Ok(mut slot) = shared_child.lock() {
        *slot = Some(child);
    }

    let stderr_handle = thread::spawn(move || read_stream(stderr));
    let reader = BufReader::new(stdout);
    let mut progress_parser = ProgressParser::default();
    let mut latest_speed = 0.0_f32;

    for line in reader.lines() {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("cancelled".to_owned());
        }

        let line = line.map_err(|error| format!("Could not read FFmpeg progress: {error}"))?;
        if let Some(snapshot) = progress_parser.push_line(&line) {
            latest_speed = snapshot.speed_x.max(latest_speed);
            let stage_progress = if total_duration_secs <= 0.0 {
                0.0
            } else {
                (snapshot.out_time_secs / total_duration_secs).clamp(0.0, 1.0)
            };
            let progress = (weight.start + stage_progress * weight.span).clamp(0.0, 1.0);
            let eta_secs = if snapshot.speed_x > 0.05 {
                Some((total_duration_secs - snapshot.out_time_secs).max(0.0) / snapshot.speed_x)
            } else {
                None
            };

            let _ = sender.send(JobEvent::Progress(ProcessingProgress {
                progress,
                stage: stage.to_owned(),
                speed_x: snapshot.speed_x,
                eta_secs,
            }));
        }
    }

    let stderr_output = stderr_handle
        .join()
        .map_err(|_| "Could not read FFmpeg error output.".to_owned())?;

    let status = {
        let mut child = shared_child
            .lock()
            .map_err(|_| "Could not finalize FFmpeg process.".to_owned())?;
        let mut child = child
            .take()
            .ok_or_else(|| "Could not finalize FFmpeg process.".to_owned())?;
        child
            .wait()
            .map_err(|error| format!("Could not wait for FFmpeg: {error}"))?
    };

    if cancel_flag.load(Ordering::Relaxed) {
        return Err("cancelled".to_owned());
    }

    if !status.success() {
        let detail = stderr_output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("FFmpeg exited before completing the job.");
        return Err(detail.to_owned());
    }

    let _ = sender.send(JobEvent::Progress(ProcessingProgress {
        progress: (weight.start + weight.span).clamp(0.0, 1.0),
        stage: stage.to_owned(),
        speed_x: latest_speed,
        eta_secs: Some(0.0),
    }));

    Ok(())
}

fn target_total_bitrate(target_size_mb: u32, duration_secs: f32) -> u32 {
    let bytes = target_size_mb.max(1) as f64 * 1_048_576.0;
    (((bytes * 8.0) / duration_secs.max(1.0) as f64) / 1000.0 * 0.96)
        .round()
        .max(280.0) as u32
}

fn aggressive_audio_bitrate(video: &VideoMetadata) -> u32 {
    video.audio_bitrate_kbps.unwrap_or(128).clamp(64, 96)
}

fn quality_audio_bitrate(video: &VideoMetadata) -> u32 {
    video.audio_bitrate_kbps.unwrap_or(128).clamp(96, 160)
}

fn quality_to_crf(quality: u8, codec: CodecChoice) -> u8 {
    let quality = quality as f32 / 100.0;
    match codec {
        CodecChoice::H264 => (31.0 - quality * 13.0).round() as u8,
        CodecChoice::H265 => (34.0 - quality * 12.0).round() as u8,
        CodecChoice::Av1 => (40.0 - quality * 14.0).round() as u8,
    }
}

fn quality_estimated_bitrate(
    video: &VideoMetadata,
    settings: &VideoSettings,
    codec: CodecChoice,
    output_width: u32,
    output_height: u32,
) -> u32 {
    let source_kbps = video
        .video_bitrate_kbps
        .or(video.container_bitrate_kbps)
        .unwrap_or_else(|| {
            ((video.size_bytes as f64 * 8.0) / video.duration_secs.max(1.0) as f64 / 1000.0).round()
                as u32
        })
        .max(500);
    let quality_factor = 0.30 + (settings.quality as f32 / 100.0) * 0.52;
    let scale_factor = (output_width as f32 * output_height as f32)
        / (video.width as f32 * video.height as f32).max(1.0);
    let codec_factor = match codec {
        CodecChoice::H264 => 1.0,
        CodecChoice::H265 => 0.82,
        CodecChoice::Av1 => 0.72,
    };

    (source_kbps as f32 * quality_factor * scale_factor.powf(0.85) * codec_factor)
        .round()
        .clamp(400.0, source_kbps as f32 * 0.97) as u32
}

fn reduce_size_resolution(video: &VideoMetadata, target_size_mb: u32) -> ResolutionChoice {
    if target_size_mb <= 12 {
        ResolutionChoice::Sd480
    } else if target_size_mb <= 28 {
        ResolutionChoice::Hd720
    } else if video.height > 1080 {
        ResolutionChoice::Hd1080
    } else {
        ResolutionChoice::Original
    }
}

fn resolve_dimensions(video: &VideoMetadata, choice: ResolutionChoice) -> (u32, u32) {
    let max_height = match choice {
        ResolutionChoice::Auto => Some(auto_height(video)),
        ResolutionChoice::Original => None,
        _ => choice.max_height(),
    };

    let Some(max_height) = max_height else {
        return (make_even(video.width), make_even(video.height));
    };

    if video.height <= max_height {
        return (make_even(video.width), make_even(video.height));
    }

    let ratio = max_height as f32 / video.height as f32;
    let width = make_even((video.width as f32 * ratio).round() as u32).max(2);
    let height = make_even(max_height).max(2);

    (width, height)
}

fn auto_height(video: &VideoMetadata) -> u32 {
    if video.height > 1080 {
        1080
    } else {
        video.height
    }
}

fn resolve_fps(video: &VideoMetadata, settings: &VideoSettings) -> f32 {
    match settings.mode {
        CompressionMode::CustomAdvanced => settings
            .custom_fps
            .max(12)
            .min(video.fps.round().max(12.0) as u32)
            as f32,
        _ => video.fps,
    }
}

fn encoder_preset(
    encoder: ResolvedEncoder,
    preview_mode: bool,
    aggressive: bool,
) -> Option<String> {
    if encoder.is_hardware() {
        return None;
    }

    Some(match encoder.codec {
        CodecChoice::H264 => {
            if preview_mode {
                "veryfast".to_owned()
            } else if aggressive {
                "slow".to_owned()
            } else {
                "medium".to_owned()
            }
        }
        CodecChoice::H265 => {
            if preview_mode {
                "faster".to_owned()
            } else {
                "medium".to_owned()
            }
        }
        CodecChoice::Av1 => {
            if preview_mode {
                "8".to_owned()
            } else {
                "6".to_owned()
            }
        }
    })
}

fn estimate_size_bytes(duration_secs: f32, plan: &EncodePlan) -> u64 {
    let total_kbps = plan.video_bitrate_kbps + plan.audio_bitrate_kbps.unwrap_or(0);
    ((total_kbps as f64 * 1000.0 * duration_secs.max(1.0) as f64) / 8.0 * 1.02).round() as u64
}

fn estimate_processing_time(video: &VideoMetadata, plan: &EncodePlan) -> f32 {
    let pixel_factor = (plan.output_width as f32 * plan.output_height as f32) / (1920.0 * 1080.0);
    let fps_factor = (plan.output_fps / 30.0).max(0.75);
    let complexity = (pixel_factor * fps_factor).max(0.35);
    let base_speed = match plan.encoder.backend {
        EncoderBackend::Software => match plan.encoder.codec {
            CodecChoice::H264 => {
                if plan.crf.is_some() {
                    1.45
                } else {
                    1.05
                }
            }
            CodecChoice::H265 => {
                if plan.pass_count == 2 {
                    0.62
                } else {
                    0.48
                }
            }
            CodecChoice::Av1 => 0.22,
        },
        EncoderBackend::Nvidia => match plan.encoder.codec {
            CodecChoice::H264 => 5.8,
            CodecChoice::H265 => 4.4,
            CodecChoice::Av1 => 3.0,
        },
        EncoderBackend::Amd => match plan.encoder.codec {
            CodecChoice::H264 => 4.6,
            CodecChoice::H265 => 3.5,
            CodecChoice::Av1 => 2.4,
        },
    };
    let speed_x = (base_speed / complexity).clamp(0.08, 10.0);
    (video.duration_secs * plan.pass_count as f32) / speed_x
}

fn build_filter_chain(video: &VideoMetadata, plan: &EncodePlan) -> String {
    let mut filters = Vec::new();

    if plan.output_width != video.width || plan.output_height != video.height {
        filters.push(format!(
            "scale={}:{}",
            plan.output_width, plan.output_height
        ));
    }

    if plan.output_fps + 0.25 < video.fps {
        filters.push(format!("fps={:.2}", plan.output_fps));
    }

    filters.join(",")
}

fn resolve_output_dir(base_output_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match base_output_dir {
        Some(path) => Ok(path),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("Clock error: {error}"))?
                .as_secs();
            let root = runtime::default_video_output_root();

            Ok(root.join(format!("run-{timestamp}")))
        }
    }
}

fn preview_dir() -> Result<PathBuf, String> {
    Ok(std::env::temp_dir()
        .join("compressity")
        .join("video-previews"))
}

fn build_output_name(source: &Path, suffix: &str, extension: &str) -> String {
    let stem = source
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("video");
    let safe_stem = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{safe_stem}-{suffix}.{extension}")
}

/// Produces a unique output path by appending -1, -2, … when a file already exists.
fn build_unique_output_path(
    output_dir: &Path,
    source: &Path,
    suffix: &str,
    extension: &str,
) -> PathBuf {
    let base_name = build_output_name(source, suffix, extension);
    let candidate = output_dir.join(&base_name);
    if !candidate.exists() {
        return candidate;
    }

    let stem = source
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("video");
    let safe_stem: String = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();

    for counter in 1..=999 {
        let name = format!("{safe_stem}-{suffix}-{counter}.{extension}");
        let path = output_dir.join(&name);
        if !path.exists() {
            return path;
        }
    }

    // Fallback: use a timestamp to guarantee uniqueness
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    output_dir.join(format!("{safe_stem}-{suffix}-{timestamp}.{extension}"))
}

/// Extracts a single thumbnail frame from a video using FFmpeg.
/// Returns the raw RGBA bytes and dimensions (width, height) on success.
pub fn generate_thumbnail(
    engine: &EngineInfo,
    video_path: &Path,
    duration_secs: f32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let thumb_dir = std::env::temp_dir()
        .join("compressity")
        .join("video-thumbs");
    fs::create_dir_all(&thumb_dir)
        .map_err(|error| format!("Could not create thumbnail folder: {error}"))?;

    let stem = video_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("thumb");
    let thumb_path = thumb_dir.join(format!("{stem}-thumb.png"));

    // Seek to 10% of the video duration for a meaningful thumbnail
    let seek_secs = (duration_secs * 0.1).min(duration_secs).max(0.0);

    let mut command = background_command(&engine.ffmpeg_path);
    command
        .arg("-y")
        .arg("-ss")
        .arg(format!("{seek_secs:.2}"))
        .arg("-i")
        .arg(video_path)
        .arg("-vframes")
        .arg("1")
        .arg("-vf")
        .arg("scale=120:-1")
        .arg(&thumb_path);

    let output = command
        .output()
        .map_err(|error| format!("Could not run FFmpeg for thumbnail: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .last()
            .unwrap_or("Thumbnail extraction failed.");
        return Err(detail.to_owned());
    }

    let img =
        image::open(&thumb_path).map_err(|error| format!("Could not decode thumbnail: {error}"))?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let bytes = rgba.into_raw();

    // Cleanup temporary file
    let _ = fs::remove_file(&thumb_path);

    Ok((bytes, width, height))
}

fn preview_start(duration_secs: f32) -> f32 {
    if duration_secs <= 7.5 {
        0.0
    } else {
        (duration_secs * 0.2).min(duration_secs - 5.0).max(0.0)
    }
}

fn cleanup_passlog(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("log.mbtree"));
    let _ = fs::remove_file(path.with_extension("log.temp"));
}

fn null_output_path() -> &'static Path {
    if cfg!(windows) {
        Path::new("NUL")
    } else {
        Path::new("/dev/null")
    }
}

fn background_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn run_capture(mut command: Command) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|error| format!("Could not start process: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .last()
            .unwrap_or("Process exited unexpectedly.");
        return Err(detail.to_owned());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read_stream<R: Read>(reader: R) -> String {
    let mut buffer = String::new();
    let mut reader = BufReader::new(reader);
    let _ = reader.read_to_string(&mut buffer);
    buffer
}

fn parse_u32(value: Option<&String>) -> Option<u32> {
    value.and_then(|value| value.parse::<u32>().ok())
}

fn parse_u64(value: Option<&String>) -> Option<u64> {
    value.and_then(|value| value.parse::<u64>().ok())
}

fn parse_f32(value: Option<&String>) -> Option<f32> {
    value.and_then(|value| value.parse::<f32>().ok())
}

fn parse_ratio(value: Option<&String>) -> Option<f32> {
    let value = value?;
    if let Some((left, right)) = value.split_once('/') {
        let left = left.parse::<f32>().ok()?;
        let right = right.parse::<f32>().ok()?;
        if right == 0.0 {
            None
        } else {
            Some(left / right)
        }
    } else {
        value.parse::<f32>().ok()
    }
}

fn parse_time_to_secs(value: &str) -> Option<f32> {
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<f32>().ok()?;
    let minutes = parts.next()?.parse::<f32>().ok()?;
    let seconds = parts.next()?.parse::<f32>().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn make_even(value: u32) -> u32 {
    if value % 2 == 0 { value } else { value - 1 }
}

fn format_time_arg(seconds: f32) -> String {
    format!("{seconds:.2}")
}

#[derive(Default)]
struct ProgressParser {
    out_time_secs: f32,
    speed_x: f32,
}

impl ProgressParser {
    fn push_line(&mut self, line: &str) -> Option<ProgressSnapshot> {
        let (key, value) = line.split_once('=')?;
        match key {
            "out_time_us" => {
                self.out_time_secs = value.parse::<f32>().ok()? / 1_000_000.0;
                None
            }
            "out_time_ms" => {
                let raw = value.parse::<f32>().ok()?;
                self.out_time_secs = if raw > 500_000.0 {
                    raw / 1_000_000.0
                } else {
                    raw / 1000.0
                };
                None
            }
            "out_time" => {
                self.out_time_secs = parse_time_to_secs(value)?;
                None
            }
            "speed" => {
                self.speed_x = value.trim_end_matches('x').parse::<f32>().unwrap_or(0.0);
                None
            }
            "progress" => Some(ProgressSnapshot {
                out_time_secs: self.out_time_secs,
                speed_x: self.speed_x.max(0.0),
            }),
            _ => None,
        }
    }
}

struct ProgressSnapshot {
    out_time_secs: f32,
    speed_x: f32,
}

#[cfg(test)]
mod tests {
    use super::{
        build_recommendation, parse_ffprobe_output, parse_time_to_secs, size_slider_range,
    };
    use crate::modules::compress_videos::models::{
        CompressionMode, SizeSliderRange, VideoMetadata,
    };
    use std::path::PathBuf;

    #[test]
    fn parses_ffprobe_flat_output() {
        let parsed = parse_ffprobe_output(
            PathBuf::from("clip.mp4"),
            r#"
format_duration="14.500000"
format_size="52428800"
format_bit_rate="28949760"
streams_stream_0_codec_type="video"
streams_stream_0_codec_name="h264"
streams_stream_0_width=1920
streams_stream_0_height=1080
streams_stream_0_avg_frame_rate="30000/1001"
streams_stream_0_bit_rate="28000000"
streams_stream_1_codec_type="audio"
streams_stream_1_bit_rate="128000"
"#,
        )
        .unwrap();

        assert_eq!(parsed.width, 1920);
        assert_eq!(parsed.height, 1080);
        assert_eq!(parsed.video_codec, "h264");
        assert!(parsed.has_audio);
    }

    #[test]
    fn parses_progress_time() {
        let seconds = parse_time_to_secs("00:01:05.50").unwrap();
        assert!((seconds - 65.5).abs() < 0.01);
    }

    #[test]
    fn builds_reasonable_size_range() {
        let video = sample_video();
        let range = size_slider_range(&video);
        assert!(range.min_mb < range.max_mb);
        assert!(range.recommended_mb >= range.min_mb);
        assert!(range.recommended_mb <= range.max_mb);
    }

    #[test]
    fn builds_recommendation_for_large_video() {
        let video = sample_video();
        let recommendation = build_recommendation(
            &video,
            SizeSliderRange {
                min_mb: 6,
                max_mb: 120,
                recommended_mb: 20,
            },
        )
        .unwrap();

        assert_eq!(recommendation.mode, CompressionMode::ReduceSize);
        assert!(recommendation.headline.contains("20 MB"));
    }

    fn sample_video() -> VideoMetadata {
        VideoMetadata {
            path: PathBuf::from("clip.mp4"),
            file_name: "clip.mp4".to_owned(),
            size_bytes: 80 * 1_048_576,
            duration_secs: 42.0,
            width: 1920,
            height: 1080,
            fps: 30.0,
            container_bitrate_kbps: Some(12_000),
            video_bitrate_kbps: Some(11_400),
            audio_bitrate_kbps: Some(128),
            video_codec: "h264".to_owned(),
            has_audio: true,
        }
    }
}
