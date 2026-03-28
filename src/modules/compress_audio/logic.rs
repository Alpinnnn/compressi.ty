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
    modules::{
        compress_audio::models::{
            AudioAnalysis, AudioAutoPreset, AudioCompressionPlan, AudioCompressionResult,
            AudioCompressionSettings, AudioContentKind, AudioEstimate, AudioFormat, AudioMetadata,
            AudioProcessingProgress, AudioWorkflowMode,
        },
        compress_videos::models::{EncoderAvailability, EngineInfo},
    },
    runtime,
};

const AUDIO_EXTENSIONS: [&str; 14] = [
    "aac", "aif", "aiff", "flac", "m4a", "m4b", "mka", "mp2", "mp3", "oga", "ogg", "opus", "wav",
    "wma",
];

/// Returns whether the given path looks like a supported audio file.
pub fn is_supported_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .map(|ext| AUDIO_EXTENSIONS.iter().any(|known| *known == ext))
        .unwrap_or(false)
}

/// Probes an audio file with ffprobe and returns the metadata needed by the UI and planner.
pub fn probe_audio(engine: &EngineInfo, path: PathBuf) -> Result<AudioMetadata, String> {
    let mut command = background_command(&engine.ffprobe_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg(
            "format=duration,size,bit_rate,format_name:stream=index,codec_type,codec_name,channels,sample_rate,bit_rate",
        )
        .arg("-of")
        .arg("flat=s=_")
        .arg(&path);

    let output = run_capture(command)?;
    parse_ffprobe_output(path, &output)
}

/// Builds the lightweight smart analysis summary shown above the settings.
pub fn analyze_audio(metadata: &AudioMetadata, encoders: &EncoderAvailability) -> AudioAnalysis {
    let content_kind = detect_content_kind(metadata);
    let auto_format = choose_auto_format(content_kind, AudioAutoPreset::Balanced, encoders);

    let detail = match content_kind {
        AudioContentKind::Voice => match auto_format {
            AudioFormat::Opus => {
                "Detected a voice-focused recording, so Smart Mode will prefer OPUS for smaller files with clear speech."
            }
            AudioFormat::Aac => {
                "Detected a voice-focused recording, so Smart Mode will keep speech clear with an AAC fallback."
            }
            AudioFormat::Mp3 => {
                "Detected a voice-focused recording. MP3 fallback is available for compatibility, but file size may not shrink as much."
            }
            AudioFormat::Flac => {
                "Detected a voice-focused recording, but only lossless output is available from the current FFmpeg build."
            }
        },
        AudioContentKind::Music => match auto_format {
            AudioFormat::Aac => {
                "Detected a music-heavy file, so Smart Mode will favor AAC for a strong quality-to-size balance."
            }
            AudioFormat::Opus => {
                "Detected a music-heavy file. Smart Mode can still use OPUS when it offers better savings on this device."
            }
            AudioFormat::Mp3 => {
                "Detected a music-heavy file. MP3 is available as the safest compatibility fallback."
            }
            AudioFormat::Flac => {
                "Detected a music-heavy file, but the current FFmpeg build only exposes lossless output."
            }
        },
        AudioContentKind::Mixed => match auto_format {
            AudioFormat::Aac => {
                "Detected a mixed file, so Smart Mode will stay conservative with AAC unless a smaller preset is chosen."
            }
            AudioFormat::Opus => {
                "Detected a mixed file, so Smart Mode will use OPUS when low-bitrate efficiency matters most."
            }
            AudioFormat::Mp3 => {
                "Detected a mixed file. MP3 fallback keeps output widely compatible across older devices."
            }
            AudioFormat::Flac => {
                "Detected a mixed file, but only lossless output is currently available from the FFmpeg runtime."
            }
        },
    };

    AudioAnalysis {
        content_kind,
        headline: format!("Detected {}", content_kind.label()),
        detail: detail.to_owned(),
    }
}

/// Computes the output estimate shown in the settings panel.
pub fn estimate_output(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    encoders: &EncoderAvailability,
) -> AudioEstimate {
    let plan = build_plan(metadata, settings, encoders);
    let savings_percent = if metadata.size_bytes == 0 {
        0.0
    } else {
        100.0 - (plan.estimated_size_bytes as f32 / metadata.size_bytes as f32 * 100.0)
    };

    AudioEstimate {
        original_size_bytes: metadata.size_bytes,
        estimated_size_bytes: plan.estimated_size_bytes,
        savings_percent,
        output_format: plan.output_format,
        target_bitrate_kbps: plan.target_bitrate_kbps,
        effective_sample_rate_hz: plan.sample_rate_hz,
        effective_channels: plan.channels,
        warnings: plan.warnings.clone(),
        recommendation: plan.recommendation.clone(),
        should_skip: plan.should_skip,
        skip_reason: plan.skip_reason.clone(),
    }
}

/// Resolves the format, bitrate, and safety warnings for the current file and settings.
pub fn build_plan(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    encoders: &EncoderAvailability,
) -> AudioCompressionPlan {
    let content_kind = detect_content_kind(metadata);
    let mut warnings = Vec::new();
    let mut recommendation = None;

    let requested_format = match settings.mode {
        AudioWorkflowMode::Auto => choose_auto_format(content_kind, settings.auto_preset, encoders),
        AudioWorkflowMode::Manual => settings.manual_format,
    };

    let (output_format, encoder_name) = resolve_encoder(requested_format, content_kind, encoders);
    if output_format != requested_format {
        warnings.push(format!(
            "{} is not available in the current FFmpeg build. Using {} instead.",
            requested_format.label(),
            output_format.label()
        ));
    }

    let target_bitrate_kbps = resolve_target_bitrate(
        metadata,
        settings,
        output_format,
        content_kind,
        encoder_name,
    );
    let sample_rate_hz = resolve_sample_rate(metadata, settings, content_kind);
    let channels = resolve_channels(metadata, settings, content_kind);

    if metadata.is_lossy() && !output_format.is_lossless() && !settings.convert_format_only {
        warnings.push(
            "This will recompress a lossy source. The file can get smaller, but some detail may be lost."
                .to_owned(),
        );
    }

    if metadata.size_bytes <= 256 * 1024 || metadata.duration_secs <= 10.0 {
        warnings
            .push("This file is already small, so compression savings may be minimal.".to_owned());
    }

    if settings.convert_format_only {
        warnings.push(
            "Convert format only keeps quality first, so the output can stay close to the original size or grow slightly."
                .to_owned(),
        );
    }

    if let Some(target_bitrate_kbps) = target_bitrate_kbps {
        let effective_channels = channels.unwrap_or(metadata.channels).max(1) as u32;
        let per_channel_bitrate = target_bitrate_kbps / effective_channels.max(1);
        if is_bitrate_too_aggressive(content_kind, per_channel_bitrate) {
            recommendation = Some(match content_kind {
                AudioContentKind::Voice => {
                    "Speech may sound thin with this target. Try Balanced or High Quality for a safer result."
                        .to_owned()
                }
                AudioContentKind::Music | AudioContentKind::Mixed => {
                    "This bitrate is aggressive for music. Try a higher bitrate or the High Quality preset for cleaner output."
                        .to_owned()
                }
            });
        }
    }

    let estimated_size_bytes = estimate_size_bytes(
        metadata,
        output_format,
        target_bitrate_kbps,
        settings.convert_format_only,
    );

    let mut should_skip = false;
    let mut skip_reason = None;
    if !settings.convert_format_only && !output_format.is_lossless() {
        let would_not_help = estimated_size_bytes >= metadata.size_bytes.saturating_mul(96) / 100;
        let source_bitrate = source_bitrate_kbps(metadata);
        let target_close_to_source = target_bitrate_kbps
            .zip(source_bitrate)
            .map(|(target, source)| target >= source.saturating_sub(8))
            .unwrap_or(false);

        if would_not_help || target_close_to_source {
            should_skip = true;
            skip_reason = Some(
                "The current settings are unlikely to shrink this file in a meaningful way."
                    .to_owned(),
            );
            warnings.push(
                "The file is already compact for the chosen mode. Consider Small Size or a different format if you need stronger savings."
                    .to_owned(),
            );
        }
    }

    AudioCompressionPlan {
        output_format,
        encoder_name,
        target_bitrate_kbps,
        sample_rate_hz,
        channels,
        content_kind,
        warnings,
        recommendation,
        estimated_size_bytes,
        should_skip,
        skip_reason,
    }
}

/// Events emitted by the background batch worker.
#[derive(Clone, Debug)]
pub enum AudioBatchEvent {
    ItemStarted {
        id: u64,
    },
    ItemProgress {
        id: u64,
        progress: AudioProcessingProgress,
    },
    ItemFinished {
        id: u64,
        result: AudioCompressionResult,
    },
    ItemSkipped {
        id: u64,
        reason: String,
    },
    ItemFailed {
        id: u64,
        error: String,
    },
    BatchFinished {
        cancelled: bool,
    },
}

/// Handle for a running audio compression batch.
pub struct AudioBatchHandle {
    pub receiver: Receiver<AudioBatchEvent>,
    pub output_dir: PathBuf,
    pub item_ids: Vec<u64>,
    cancel_flag: Arc<AtomicBool>,
}

impl AudioBatchHandle {
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

/// A single item to compress in the audio batch.
pub struct AudioBatchItem {
    pub id: u64,
    pub metadata: AudioMetadata,
    pub settings: AudioCompressionSettings,
}

/// Starts sequential audio compression work in a background thread.
pub fn start_audio_batch(
    engine: EngineInfo,
    items: Vec<AudioBatchItem>,
    base_output_dir: Option<PathBuf>,
) -> Result<AudioBatchHandle, String> {
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
    let item_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();

    thread::spawn(move || {
        for item in &items {
            if thread_cancel.load(Ordering::Relaxed) {
                let _ = sender.send(AudioBatchEvent::BatchFinished { cancelled: true });
                return;
            }

            let _ = sender.send(AudioBatchEvent::ItemStarted { id: item.id });
            match compress_single_audio(
                &engine,
                &item.metadata,
                &item.settings,
                &thread_output_dir,
                &thread_cancel,
                item.id,
                &sender,
            ) {
                Ok(CompressionDisposition::Completed(result)) => {
                    let _ = sender.send(AudioBatchEvent::ItemFinished {
                        id: item.id,
                        result,
                    });
                }
                Ok(CompressionDisposition::Skipped(reason)) => {
                    let _ = sender.send(AudioBatchEvent::ItemSkipped {
                        id: item.id,
                        reason,
                    });
                }
                Err(error) => {
                    if error == "cancelled" {
                        let _ = sender.send(AudioBatchEvent::BatchFinished { cancelled: true });
                        return;
                    }

                    let _ = sender.send(AudioBatchEvent::ItemFailed { id: item.id, error });
                }
            }
        }

        let _ = sender.send(AudioBatchEvent::BatchFinished { cancelled: false });
    });

    Ok(AudioBatchHandle {
        receiver,
        output_dir,
        item_ids,
        cancel_flag,
    })
}

enum CompressionDisposition {
    Completed(AudioCompressionResult),
    Skipped(String),
}

fn compress_single_audio(
    engine: &EngineInfo,
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    output_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
    id: u64,
    batch_sender: &mpsc::Sender<AudioBatchEvent>,
) -> Result<CompressionDisposition, String> {
    let plan = build_plan(metadata, settings, &engine.encoders);
    if plan.should_skip && !settings.convert_format_only {
        return Ok(CompressionDisposition::Skipped(
            plan.skip_reason
                .unwrap_or_else(|| "This file is already compact enough.".to_owned()),
        ));
    }

    let output_path = build_unique_output_path(
        output_dir,
        &metadata.path,
        "compressed",
        plan.output_format.extension(),
    );

    let (job_sender, job_receiver) = mpsc::channel::<AudioProcessingProgress>();
    let child = Arc::new(Mutex::new(None::<Child>));
    let started_at = Instant::now();
    let progress_cancel = Arc::clone(cancel_flag);
    let batch_tx = batch_sender.clone();
    let progress_thread = thread::spawn(move || {
        while let Ok(progress) = job_receiver.recv() {
            if progress_cancel.load(Ordering::Relaxed) {
                break;
            }
            let _ = batch_tx.send(AudioBatchEvent::ItemProgress { id, progress });
        }
    });

    let result = run_encode_pass(
        build_encode_command(&engine.ffmpeg_path, metadata, settings, &plan, &output_path),
        metadata.duration_secs,
        "Compressing audio",
        cancel_flag,
        &child,
        &job_sender,
    );

    drop(job_sender);
    let _ = progress_thread.join();

    if matches!(result, Err(ref error) if error == "cancelled") {
        let _ = fs::remove_file(&output_path);
        return Err("cancelled".to_owned());
    }
    result?;

    let output_size_bytes = fs::metadata(&output_path)
        .map_err(|error| format!("Could not read compressed file: {error}"))?
        .len();
    let reduction_percent = if metadata.size_bytes == 0 {
        0.0
    } else {
        100.0 - (output_size_bytes as f32 / metadata.size_bytes as f32 * 100.0)
    };

    Ok(CompressionDisposition::Completed(AudioCompressionResult {
        output_path,
        original_size_bytes: metadata.size_bytes,
        output_size_bytes,
        reduction_percent,
        elapsed_secs: started_at.elapsed().as_secs_f32(),
    }))
}

fn build_encode_command(
    ffmpeg_path: &Path,
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    plan: &AudioCompressionPlan,
    output_path: &Path,
) -> Command {
    let mut command = background_command(ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-nostdin")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg("-i")
        .arg(&metadata.path)
        .arg("-map")
        .arg("0:a:0")
        .arg("-vn")
        .arg("-sn")
        .arg("-dn");

    if settings.remove_metadata {
        command.arg("-map_metadata").arg("-1");
    }

    if let Some(sample_rate_hz) = plan.sample_rate_hz {
        command.arg("-ar").arg(sample_rate_hz.to_string());
    }

    if let Some(channels) = plan.channels {
        command.arg("-ac").arg(channels.to_string());
    }

    let filter_chain = build_filter_chain(settings);
    if !filter_chain.is_empty() {
        command.arg("-af").arg(filter_chain.join(","));
    }

    command.arg("-c:a").arg(plan.encoder_name);

    match plan.output_format {
        AudioFormat::Aac => {
            command.arg("-profile:a").arg("aac_low");
            if let Some(target_bitrate_kbps) = plan.target_bitrate_kbps {
                command.arg("-b:a").arg(format!("{target_bitrate_kbps}k"));
            }
        }
        AudioFormat::Opus => {
            if let Some(target_bitrate_kbps) = plan.target_bitrate_kbps {
                command.arg("-b:a").arg(format!("{target_bitrate_kbps}k"));
            }
            command.arg("-vbr").arg("on");
            if plan.encoder_name == "libopus" {
                command.arg("-application").arg(match plan.content_kind {
                    AudioContentKind::Voice => "voip",
                    AudioContentKind::Music | AudioContentKind::Mixed => "audio",
                });
            }
        }
        AudioFormat::Mp3 => {
            if let Some(target_bitrate_kbps) = plan.target_bitrate_kbps {
                command.arg("-b:a").arg(format!("{target_bitrate_kbps}k"));
            }
        }
        AudioFormat::Flac => {
            command.arg("-compression_level").arg("5");
        }
    }

    command.arg(output_path);
    command
}

fn build_filter_chain(settings: &AudioCompressionSettings) -> Vec<&'static str> {
    let mut filters = Vec::new();
    if settings.normalize_volume {
        // Loudness normalization improves perceived consistency without forcing the user
        // to understand mastering targets or peak measurements.
        filters.push("loudnorm=I=-16:LRA=11:TP=-1.5");
    }
    filters
}

fn run_encode_pass(
    mut command: Command,
    total_duration_secs: f32,
    stage: &str,
    cancel_flag: &AtomicBool,
    shared_child: &Arc<Mutex<Option<Child>>>,
    sender: &mpsc::Sender<AudioProcessingProgress>,
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
            kill_active_child(shared_child);
            return Err("cancelled".to_owned());
        }

        let line = line.map_err(|error| format!("Could not read FFmpeg progress: {error}"))?;
        if let Some(snapshot) = progress_parser.push_line(&line) {
            latest_speed = snapshot.speed_x.max(latest_speed);
            let progress = if total_duration_secs <= 0.0 {
                0.0
            } else {
                (snapshot.out_time_secs / total_duration_secs).clamp(0.0, 1.0)
            };
            let eta_secs = if snapshot.speed_x > 0.05 {
                Some((total_duration_secs - snapshot.out_time_secs).max(0.0) / snapshot.speed_x)
            } else {
                None
            };

            let _ = sender.send(AudioProcessingProgress {
                progress,
                stage: stage.to_owned(),
                speed_x: snapshot.speed_x,
                eta_secs,
            });
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

    let _ = sender.send(AudioProcessingProgress {
        progress: 1.0,
        stage: stage.to_owned(),
        speed_x: latest_speed,
        eta_secs: Some(0.0),
    });

    Ok(())
}

fn kill_active_child(shared_child: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut child_slot) = shared_child.lock()
        && let Some(child) = child_slot.as_mut()
    {
        let _ = child.kill();
    }
}

fn detect_content_kind(metadata: &AudioMetadata) -> AudioContentKind {
    let bitrate_kbps = source_bitrate_kbps(metadata).unwrap_or(128);
    if metadata.channels <= 1 && (metadata.sample_rate_hz <= 32_000 || bitrate_kbps <= 96) {
        AudioContentKind::Voice
    } else if metadata.channels >= 2 && metadata.sample_rate_hz >= 44_100 && bitrate_kbps >= 96 {
        AudioContentKind::Music
    } else {
        AudioContentKind::Mixed
    }
}

fn choose_auto_format(
    content_kind: AudioContentKind,
    preset: AudioAutoPreset,
    encoders: &EncoderAvailability,
) -> AudioFormat {
    let preferred_formats = match (content_kind, preset) {
        (AudioContentKind::Voice, _) => [
            AudioFormat::Opus,
            AudioFormat::Aac,
            AudioFormat::Mp3,
            AudioFormat::Flac,
        ],
        (_, AudioAutoPreset::SmallSize) => [
            AudioFormat::Opus,
            AudioFormat::Aac,
            AudioFormat::Mp3,
            AudioFormat::Flac,
        ],
        (_, AudioAutoPreset::HighQuality) => [
            AudioFormat::Aac,
            AudioFormat::Opus,
            AudioFormat::Mp3,
            AudioFormat::Flac,
        ],
        (_, AudioAutoPreset::Balanced) => [
            AudioFormat::Aac,
            AudioFormat::Opus,
            AudioFormat::Mp3,
            AudioFormat::Flac,
        ],
    };

    preferred_formats
        .into_iter()
        .find(|format| format_supported(*format, encoders))
        .unwrap_or(AudioFormat::Flac)
}

fn resolve_encoder(
    requested_format: AudioFormat,
    content_kind: AudioContentKind,
    encoders: &EncoderAvailability,
) -> (AudioFormat, &'static str) {
    if let Some(encoder_name) = encoder_name_for_format(requested_format, encoders) {
        return (requested_format, encoder_name);
    }

    let fallback = choose_auto_format(content_kind, AudioAutoPreset::Balanced, encoders);
    if let Some(encoder_name) = encoder_name_for_format(fallback, encoders) {
        return (fallback, encoder_name);
    }

    // The bundled runtime should always expose at least one audio encoder. If the current build
    // is unusually limited, fall back to the native AAC encoder name so the final FFmpeg error is
    // explicit instead of silently hiding the problem.
    (AudioFormat::Aac, "aac")
}

fn resolve_target_bitrate(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    output_format: AudioFormat,
    content_kind: AudioContentKind,
    encoder_name: &str,
) -> Option<u32> {
    if output_format.is_lossless() {
        return None;
    }

    let bitrate = match settings.mode {
        AudioWorkflowMode::Auto => {
            auto_target_bitrate(output_format, content_kind, settings.auto_preset)
        }
        AudioWorkflowMode::Manual => Some(settings.manual_bitrate_kbps.clamp(24, 320)),
    }?;

    let source_bitrate = source_bitrate_kbps(metadata);
    let format_floor = high_quality_floor(output_format, content_kind);
    let adjusted = if settings.convert_format_only {
        source_bitrate.map(|source| bitrate.max(source).max(format_floor))
    } else {
        Some(bitrate)
    };

    adjusted.map(|value| {
        if encoder_name == "libshine" {
            // libshine is more limited than libmp3lame and behaves best with common CBR steps.
            round_to_nearest(value, 16).clamp(32, 320)
        } else {
            value
        }
    })
}

fn resolve_sample_rate(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    content_kind: AudioContentKind,
) -> Option<u32> {
    match settings.mode {
        AudioWorkflowMode::Manual => settings.manual_sample_rate_hz,
        AudioWorkflowMode::Auto => match (content_kind, settings.auto_preset) {
            (AudioContentKind::Voice, AudioAutoPreset::SmallSize) => Some(24_000),
            (AudioContentKind::Voice, AudioAutoPreset::Balanced) => Some(32_000),
            (_, AudioAutoPreset::SmallSize) if metadata.sample_rate_hz > 44_100 => Some(44_100),
            _ => None,
        },
    }
}

fn resolve_channels(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    content_kind: AudioContentKind,
) -> Option<u8> {
    match settings.mode {
        AudioWorkflowMode::Manual => settings.manual_channels,
        AudioWorkflowMode::Auto => match content_kind {
            AudioContentKind::Voice if metadata.channels > 1 => Some(1),
            _ => None,
        },
    }
}

fn auto_target_bitrate(
    output_format: AudioFormat,
    content_kind: AudioContentKind,
    preset: AudioAutoPreset,
) -> Option<u32> {
    let bitrate = match (output_format, content_kind, preset) {
        (AudioFormat::Flac, _, _) => return None,
        (AudioFormat::Opus, AudioContentKind::Voice, AudioAutoPreset::HighQuality) => 48,
        (AudioFormat::Opus, AudioContentKind::Voice, AudioAutoPreset::Balanced) => 32,
        (AudioFormat::Opus, AudioContentKind::Voice, AudioAutoPreset::SmallSize) => 24,
        (AudioFormat::Opus, AudioContentKind::Music, AudioAutoPreset::HighQuality) => 128,
        (AudioFormat::Opus, AudioContentKind::Music, AudioAutoPreset::Balanced) => 96,
        (AudioFormat::Opus, AudioContentKind::Music, AudioAutoPreset::SmallSize) => 72,
        (AudioFormat::Opus, AudioContentKind::Mixed, AudioAutoPreset::HighQuality) => 112,
        (AudioFormat::Opus, AudioContentKind::Mixed, AudioAutoPreset::Balanced) => 80,
        (AudioFormat::Opus, AudioContentKind::Mixed, AudioAutoPreset::SmallSize) => 64,
        (AudioFormat::Aac, AudioContentKind::Voice, AudioAutoPreset::HighQuality) => 72,
        (AudioFormat::Aac, AudioContentKind::Voice, AudioAutoPreset::Balanced) => 64,
        (AudioFormat::Aac, AudioContentKind::Voice, AudioAutoPreset::SmallSize) => 48,
        (AudioFormat::Aac, AudioContentKind::Music, AudioAutoPreset::HighQuality) => 160,
        (AudioFormat::Aac, AudioContentKind::Music, AudioAutoPreset::Balanced) => 128,
        (AudioFormat::Aac, AudioContentKind::Music, AudioAutoPreset::SmallSize) => 96,
        (AudioFormat::Aac, AudioContentKind::Mixed, AudioAutoPreset::HighQuality) => 144,
        (AudioFormat::Aac, AudioContentKind::Mixed, AudioAutoPreset::Balanced) => 112,
        (AudioFormat::Aac, AudioContentKind::Mixed, AudioAutoPreset::SmallSize) => 80,
        (AudioFormat::Mp3, AudioContentKind::Voice, AudioAutoPreset::HighQuality) => 96,
        (AudioFormat::Mp3, AudioContentKind::Voice, AudioAutoPreset::Balanced) => 64,
        (AudioFormat::Mp3, AudioContentKind::Voice, AudioAutoPreset::SmallSize) => 48,
        (AudioFormat::Mp3, AudioContentKind::Music, AudioAutoPreset::HighQuality) => 192,
        (AudioFormat::Mp3, AudioContentKind::Music, AudioAutoPreset::Balanced) => 128,
        (AudioFormat::Mp3, AudioContentKind::Music, AudioAutoPreset::SmallSize) => 96,
        (AudioFormat::Mp3, AudioContentKind::Mixed, AudioAutoPreset::HighQuality) => 160,
        (AudioFormat::Mp3, AudioContentKind::Mixed, AudioAutoPreset::Balanced) => 112,
        (AudioFormat::Mp3, AudioContentKind::Mixed, AudioAutoPreset::SmallSize) => 80,
    };

    Some(bitrate)
}

fn high_quality_floor(output_format: AudioFormat, content_kind: AudioContentKind) -> u32 {
    match (output_format, content_kind) {
        (AudioFormat::Opus, AudioContentKind::Voice) => 48,
        (AudioFormat::Opus, _) => 112,
        (AudioFormat::Aac, AudioContentKind::Voice) => 72,
        (AudioFormat::Aac, _) => 144,
        (AudioFormat::Mp3, AudioContentKind::Voice) => 96,
        (AudioFormat::Mp3, _) => 160,
        (AudioFormat::Flac, _) => 0,
    }
}

fn is_bitrate_too_aggressive(content_kind: AudioContentKind, per_channel_bitrate: u32) -> bool {
    match content_kind {
        AudioContentKind::Voice => per_channel_bitrate < 24,
        AudioContentKind::Music => per_channel_bitrate < 48,
        AudioContentKind::Mixed => per_channel_bitrate < 40,
    }
}

fn estimate_size_bytes(
    metadata: &AudioMetadata,
    output_format: AudioFormat,
    target_bitrate_kbps: Option<u32>,
    convert_format_only: bool,
) -> u64 {
    if output_format.is_lossless() {
        if metadata.is_lossless {
            return ((metadata.size_bytes as f32) * 0.58).round() as u64;
        }
        return ((metadata.size_bytes as f32) * 1.18).round() as u64;
    }

    let duration_secs = metadata.duration_secs.max(1.0);
    let bitrate_kbps =
        target_bitrate_kbps.unwrap_or_else(|| source_bitrate_kbps(metadata).unwrap_or(128));
    let container_overhead = match output_format {
        AudioFormat::Aac => 1.04,
        AudioFormat::Opus => 1.02,
        AudioFormat::Mp3 => 1.01,
        AudioFormat::Flac => 1.0,
    };
    let estimated = ((duration_secs * bitrate_kbps as f32 * 1000.0 / 8.0) * container_overhead)
        .max(8_192.0)
        .round() as u64;

    if convert_format_only && !output_format.is_lossless() {
        estimated.max(metadata.size_bytes.saturating_mul(98) / 100)
    } else {
        estimated
    }
}

fn source_bitrate_kbps(metadata: &AudioMetadata) -> Option<u32> {
    metadata.audio_bitrate_kbps.or_else(|| {
        if metadata.duration_secs > 0.0 {
            Some(
                ((metadata.size_bytes as f32 * 8.0) / metadata.duration_secs / 1000.0).round()
                    as u32,
            )
        } else {
            None
        }
    })
}

fn format_supported(format: AudioFormat, encoders: &EncoderAvailability) -> bool {
    encoder_name_for_format(format, encoders).is_some()
}

fn encoder_name_for_format(
    format: AudioFormat,
    encoders: &EncoderAvailability,
) -> Option<&'static str> {
    match format {
        AudioFormat::Mp3 => encoders.preferred_mp3_encoder_name(),
        AudioFormat::Aac => encoders.preferred_aac_encoder_name(),
        AudioFormat::Opus => encoders.preferred_opus_encoder_name(),
        AudioFormat::Flac => encoders.supports_flac().then_some("flac"),
    }
}

fn resolve_output_dir(base_output_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match base_output_dir {
        Some(path) => Ok(path),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("Clock error: {error}"))?
                .as_secs();
            Ok(runtime::default_audio_output_root().join(format!("run-{timestamp}")))
        }
    }
}

fn build_output_name(source: &Path, suffix: &str, extension: &str) -> String {
    format!("{}-{suffix}.{extension}", safe_stem(source, "audio"))
}

fn build_unique_output_path(
    output_dir: &Path,
    source: &Path,
    suffix: &str,
    extension: &str,
) -> PathBuf {
    let candidate = output_dir.join(build_output_name(source, suffix, extension));
    if !candidate.exists() {
        return candidate;
    }

    let safe_stem = safe_stem(source, "audio");
    for counter in 1..=999 {
        let path = output_dir.join(format!("{safe_stem}-{suffix}-{counter}.{extension}"));
        if !path.exists() {
            return path;
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    output_dir.join(format!("{safe_stem}-{suffix}-{timestamp}.{extension}"))
}

fn safe_stem(source: &Path, fallback: &str) -> String {
    source
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or(fallback)
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn parse_ffprobe_output(path: PathBuf, output: &str) -> Result<AudioMetadata, String> {
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

    let audio_stream = streams
        .values()
        .find(|stream| stream.get("codec_type").map(String::as_str) == Some("audio"))
        .ok_or_else(|| "The selected file does not contain an audio stream.".to_owned())?;

    let duration_secs = parse_f32(format_values.get("duration")).unwrap_or(0.0);
    if duration_secs <= 0.0 {
        return Err("The selected file could not be analyzed correctly.".to_owned());
    }

    let codec_name = audio_stream
        .get("codec_name")
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let container_name = format_values
        .get("format_name")
        .and_then(|value| value.split(',').next())
        .unwrap_or("audio")
        .to_owned();

    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("audio")
        .to_owned();

    Ok(AudioMetadata {
        path,
        file_name,
        size_bytes: parse_u64(format_values.get("size")).unwrap_or(0),
        duration_secs,
        audio_bitrate_kbps: parse_u32(audio_stream.get("bit_rate"))
            .or_else(|| parse_u32(format_values.get("bit_rate")))
            .map(|value| value / 1000),
        sample_rate_hz: parse_u32(audio_stream.get("sample_rate")).unwrap_or(44_100),
        channels: parse_u32(audio_stream.get("channels"))
            .unwrap_or(2)
            .clamp(1, 8) as u8,
        codec_name: codec_name.clone(),
        container_name,
        is_lossless: codec_looks_lossless(&codec_name),
    })
}

fn codec_looks_lossless(codec_name: &str) -> bool {
    matches!(
        codec_name,
        "alac"
            | "ape"
            | "flac"
            | "pcm_alaw"
            | "pcm_f32be"
            | "pcm_f32le"
            | "pcm_f64be"
            | "pcm_f64le"
            | "pcm_mulaw"
            | "pcm_s16be"
            | "pcm_s16le"
            | "pcm_s24be"
            | "pcm_s24le"
            | "pcm_s32be"
            | "pcm_s32le"
            | "wavpack"
    )
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

fn parse_time_to_secs(value: &str) -> Option<f32> {
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<f32>().ok()?;
    let minutes = parts.next()?.parse::<f32>().ok()?;
    let seconds = parts.next()?.parse::<f32>().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
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

fn round_to_nearest(value: u32, step: u32) -> u32 {
    if step == 0 {
        return value;
    }

    let lower = value / step * step;
    let upper = lower + step;
    if value - lower < upper.saturating_sub(value) {
        lower.max(step)
    } else {
        upper
    }
}

#[cfg(test)]
mod tests {
    use super::{
        choose_auto_format, detect_content_kind, estimate_size_bytes, parse_ffprobe_output,
    };
    use crate::modules::{
        compress_audio::models::{AudioAutoPreset, AudioContentKind, AudioFormat, AudioMetadata},
        compress_videos::models::EncoderAvailability,
    };
    use std::path::PathBuf;

    #[test]
    fn detects_voice_from_mono_low_rate_audio() {
        let metadata = AudioMetadata {
            path: PathBuf::from("voice.wav"),
            file_name: "voice.wav".to_owned(),
            size_bytes: 4_000_000,
            duration_secs: 180.0,
            audio_bitrate_kbps: Some(64),
            sample_rate_hz: 24_000,
            channels: 1,
            codec_name: "pcm_s16le".to_owned(),
            container_name: "wav".to_owned(),
            is_lossless: true,
        };

        assert_eq!(detect_content_kind(&metadata), AudioContentKind::Voice);
    }

    #[test]
    fn prefers_aac_for_music_balanced_when_available() {
        let encoders = EncoderAvailability {
            aac: true,
            libopus: true,
            ..Default::default()
        };

        let format = choose_auto_format(
            AudioContentKind::Music,
            AudioAutoPreset::Balanced,
            &encoders,
        );

        assert_eq!(format, AudioFormat::Aac);
    }

    #[test]
    fn estimates_lossy_outputs_smaller_than_large_pcm_inputs() {
        let metadata = AudioMetadata {
            path: PathBuf::from("track.wav"),
            file_name: "track.wav".to_owned(),
            size_bytes: 40 * 1_048_576,
            duration_secs: 240.0,
            audio_bitrate_kbps: Some(1_411),
            sample_rate_hz: 44_100,
            channels: 2,
            codec_name: "pcm_s16le".to_owned(),
            container_name: "wav".to_owned(),
            is_lossless: true,
        };

        let estimate = estimate_size_bytes(&metadata, AudioFormat::Aac, Some(128), false);

        assert!(estimate < metadata.size_bytes);
    }

    #[test]
    fn parses_audio_probe_output() {
        let parsed = parse_ffprobe_output(
            PathBuf::from("song.flac"),
            r#"
format_duration="65.500000"
format_size="10485760"
format_bit_rate="1280000"
format_format_name="flac"
streams_stream_0_codec_type="audio"
streams_stream_0_codec_name="flac"
streams_stream_0_channels=2
streams_stream_0_sample_rate="44100"
streams_stream_0_bit_rate="960000"
"#,
        )
        .unwrap();

        assert_eq!(parsed.codec_name, "flac");
        assert_eq!(parsed.channels, 2);
        assert!(parsed.is_lossless);
    }
}
