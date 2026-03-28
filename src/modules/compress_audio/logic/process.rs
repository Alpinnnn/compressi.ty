use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use crate::modules::{
    compress_audio::{
        logic::parsing::parse_ffprobe_output,
        models::{
            AudioCompressionPlan, AudioCompressionSettings, AudioContentKind, AudioMetadata,
            AudioProcessingProgress,
        },
    },
    compress_videos::models::EngineInfo,
};

use super::parsing::ProgressParser;

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

pub(super) fn build_encode_command(
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
        crate::modules::compress_audio::models::AudioFormat::Aac => {
            command.arg("-profile:a").arg("aac_low");
            if let Some(target_bitrate_kbps) = plan.target_bitrate_kbps {
                command.arg("-b:a").arg(format!("{target_bitrate_kbps}k"));
            }
        }
        crate::modules::compress_audio::models::AudioFormat::Opus => {
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
        crate::modules::compress_audio::models::AudioFormat::Mp3 => {
            if let Some(target_bitrate_kbps) = plan.target_bitrate_kbps {
                command.arg("-b:a").arg(format!("{target_bitrate_kbps}k"));
            }
        }
        crate::modules::compress_audio::models::AudioFormat::Flac => {
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

pub(super) fn run_encode_pass(
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
