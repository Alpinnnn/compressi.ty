use std::{fs, sync::mpsc};

use ffmpeg_sidecar::download::{self, FfmpegDownloadProgressEvent};

use crate::runtime;

use super::inventory::WorkerEvent;

pub(super) fn install_latest_managed_engine(
    sender: &mpsc::Sender<WorkerEvent>,
) -> Result<(), String> {
    let destination = runtime::managed_engine_dir()
        .ok_or_else(|| "Could not resolve the managed engine directory.".to_owned())?;

    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|error| {
            format!(
                "Could not clear the managed engine folder {}: {error}",
                destination.display()
            )
        })?;
    }

    fs::create_dir_all(&destination).map_err(|error| {
        format!(
            "Could not create the managed engine folder {}: {error}",
            destination.display()
        )
    })?;

    let download_url = download::ffmpeg_download_url()
        .map_err(|error| format!("Could not resolve the FFmpeg download URL: {error}"))?;

    let progress_sender = sender.clone();
    let archive_path =
        download::download_ffmpeg_package_with_progress(download_url, &destination, move |event| {
            let (progress, stage) = match event {
                FfmpegDownloadProgressEvent::Starting => {
                    (0.05, "Preparing engine update".to_owned())
                }
                FfmpegDownloadProgressEvent::Downloading {
                    total_bytes,
                    downloaded_bytes,
                } => {
                    let progress = if total_bytes == 0 {
                        0.55
                    } else {
                        (downloaded_bytes as f32 / total_bytes as f32).clamp(0.0, 1.0) * 0.82
                    };
                    (
                        0.08 + progress,
                        format!("Downloading FFmpeg ({})", format_bytes(downloaded_bytes)),
                    )
                }
                FfmpegDownloadProgressEvent::UnpackingArchive => {
                    (0.94, "Installing engine update".to_owned())
                }
                FfmpegDownloadProgressEvent::Done => (1.0, "Engine update ready".to_owned()),
            };
            let _ = progress_sender.send(WorkerEvent::Progress { progress, stage });
        })
        .map_err(|error| format!("Could not download the latest FFmpeg package: {error}"))?;

    let _ = sender.send(WorkerEvent::Progress {
        progress: 0.94,
        stage: "Installing engine update".to_owned(),
    });

    download::unpack_ffmpeg(&archive_path, &destination)
        .map_err(|error| format!("Could not unpack the latest FFmpeg package: {error}"))?;

    let _ = sender.send(WorkerEvent::Progress {
        progress: 1.0,
        stage: "Engine update ready".to_owned(),
    });

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_index = 0usize;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}
