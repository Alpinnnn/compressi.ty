use std::{
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use crate::modules::compress_videos::models::ProcessingProgress;

use super::{events::EncodeEvent, parsing::ProgressParser};

#[derive(Clone, Copy)]
pub(super) struct ProgressWeight {
    pub(super) start: f32,
    pub(super) span: f32,
}

pub(super) fn run_encode_pass(
    mut command: Command,
    total_duration_secs: f32,
    weight: ProgressWeight,
    stage: &str,
    cancel_flag: &AtomicBool,
    shared_child: &Arc<Mutex<Option<Child>>>,
    sender: &mpsc::Sender<EncodeEvent>,
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

            let _ = sender.send(EncodeEvent::Progress(ProcessingProgress {
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

    let _ = sender.send(EncodeEvent::Progress(ProcessingProgress {
        progress: (weight.start + weight.span).clamp(0.0, 1.0),
        stage: stage.to_owned(),
        speed_x: latest_speed,
        eta_secs: Some(0.0),
    }));

    Ok(())
}

pub(super) fn background_command(program: &Path) -> Command {
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

pub(super) fn run_capture(mut command: Command) -> Result<String, String> {
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

pub(super) fn format_time_arg(seconds: f32) -> String {
    format!("{seconds:.2}")
}

fn read_stream<R: Read>(reader: R) -> String {
    let mut buffer = String::new();
    let mut reader = BufReader::new(reader);
    let _ = reader.read_to_string(&mut buffer);
    buffer
}
