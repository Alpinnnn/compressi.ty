use std::{
    io::{BufRead, BufReader},
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
            if let Some(aac_vbr_mode) = plan.aac_vbr_mode {
                command.arg("-vbr").arg(aac_vbr_mode.to_string());
            } else if let Some(target_bitrate_kbps) = plan.target_bitrate_kbps {
                command.arg("-b:a").arg(format!("{target_bitrate_kbps}k"));
            }
            if plan.encoder_name == "libfdk_aac" {
                command.arg("-afterburner").arg("1");
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
            if plan.mp3_use_abr {
                command.arg("-abr").arg("1");
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
    if cancel_flag.load(Ordering::Relaxed) {
        return Err("cancelled".to_owned());
    }

    let mut child = crate::process_lifecycle::spawn_child(&mut command)
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
    let mut cancelled = cancel_flag.load(Ordering::Relaxed);
    let mut read_error = None;
    if cancelled {
        kill_active_child(shared_child);
    }

    for line in reader.lines() {
        if cancel_flag.load(Ordering::Relaxed) {
            cancelled = true;
            kill_active_child(shared_child);
            break;
        }

        let line = match line {
            Ok(line) => line,
            Err(error) => {
                read_error = Some(format!("Could not read FFmpeg progress: {error}"));
                kill_active_child(shared_child);
                break;
            }
        };
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

    if cancel_flag.load(Ordering::Relaxed) {
        cancelled = true;
        kill_active_child(shared_child);
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

    if cancelled {
        return Err("cancelled".to_owned());
    }

    if let Some(error) = read_error {
        return Err(error);
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
    let output = crate::process_lifecycle::output(&mut command)
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

fn read_stream<R: std::io::Read>(reader: R) -> String {
    crate::process_lifecycle::read_pipe_to_string(reader)
}

#[cfg(test)]
mod tests {
    use super::build_encode_command;
    use crate::modules::compress_audio::models::{
        AudioCompressionPlan, AudioCompressionSettings, AudioContentKind, AudioFormat,
        AudioMetadata,
    };
    use std::{path::Path, process::Command};

    #[test]
    fn fdk_aac_auto_mode_uses_vbr_and_afterburner() {
        let metadata = sample_metadata();
        let settings = AudioCompressionSettings::default();
        let plan = AudioCompressionPlan {
            output_format: AudioFormat::Aac,
            encoder_name: "libfdk_aac",
            target_bitrate_kbps: Some(128),
            aac_vbr_mode: Some(4),
            mp3_use_abr: false,
            sample_rate_hz: None,
            channels: None,
            content_kind: AudioContentKind::Music,
            warnings: Vec::new(),
            recommendation: None,
            estimated_size_bytes: 1_000_000,
            should_skip: false,
            skip_reason: None,
        };

        let command = build_encode_command(
            Path::new("ffmpeg"),
            &metadata,
            &settings,
            &plan,
            Path::new("out.m4a"),
        );
        let args = command_args(&command);

        assert!(contains_arg_pair(&args, "-vbr", "4"));
        assert!(contains_arg_pair(&args, "-afterburner", "1"));
        assert!(!contains_flag(&args, "-b:a"));
    }

    #[test]
    fn mp3_lame_uses_abr_for_target_bitrate() {
        let metadata = sample_metadata();
        let settings = AudioCompressionSettings::default();
        let plan = AudioCompressionPlan {
            output_format: AudioFormat::Mp3,
            encoder_name: "libmp3lame",
            target_bitrate_kbps: Some(128),
            aac_vbr_mode: None,
            mp3_use_abr: true,
            sample_rate_hz: None,
            channels: None,
            content_kind: AudioContentKind::Music,
            warnings: Vec::new(),
            recommendation: None,
            estimated_size_bytes: 1_000_000,
            should_skip: false,
            skip_reason: None,
        };

        let command = build_encode_command(
            Path::new("ffmpeg"),
            &metadata,
            &settings,
            &plan,
            Path::new("out.mp3"),
        );
        let args = command_args(&command);

        assert!(contains_arg_pair(&args, "-b:a", "128k"));
        assert!(contains_arg_pair(&args, "-abr", "1"));
    }

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn contains_flag(args: &[String], flag: &str) -> bool {
        args.iter().any(|arg| arg == flag)
    }

    fn contains_arg_pair(args: &[String], key: &str, value: &str) -> bool {
        args.windows(2)
            .any(|window| window[0] == key && window[1] == value)
    }

    fn sample_metadata() -> AudioMetadata {
        AudioMetadata {
            path: Path::new("track.wav").to_path_buf(),
            file_name: "track.wav".to_owned(),
            size_bytes: 40 * 1_048_576,
            duration_secs: 240.0,
            audio_bitrate_kbps: Some(1_411),
            sample_rate_hz: 44_100,
            channels: 2,
            codec_name: "pcm_s16le".to_owned(),
            container_name: "wav".to_owned(),
            is_lossless: true,
        }
    }
}
