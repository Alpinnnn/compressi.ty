use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use eframe::egui;
use ffmpeg_sidecar::download::{self, FfmpegDownloadProgressEvent};

use crate::{
    modules::compress_videos::models::{
        EncoderAvailability, EngineInfo, EngineSource, EngineStatus,
    },
    runtime,
};

#[derive(Default)]
pub struct VideoEngineController {
    status: EngineStatus,
    bundled: Option<EngineInfo>,
    managed: Option<EngineInfo>,
    system: Option<EngineInfo>,
    last_error: Option<String>,
    receiver: Option<Receiver<WorkerEvent>>,
}

impl VideoEngineController {
    pub fn refresh(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::Refresh);
    }

    pub fn ensure_ready(&mut self) {
        if self.is_busy() || self.active_info().is_some() {
            return;
        }

        self.start_task(TaskAction::EnsureReady);
    }

    pub fn update_to_latest(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::Update);
    }

    pub fn use_bundled_engine(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::UseBundled);
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        let mut finished = false;

        if let Some(receiver) = &self.receiver {
            while let Ok(event) = receiver.try_recv() {
                match event {
                    WorkerEvent::Progress { progress, stage } => {
                        self.status = EngineStatus::Downloading { progress, stage };
                    }
                    WorkerEvent::Inventory(inventory) => {
                        self.bundled = inventory.bundled;
                        self.managed = inventory.managed;
                        self.system = inventory.system;
                        self.last_error = inventory.error.clone();

                        if let Some(active) = inventory.active {
                            self.status = EngineStatus::Ready(active);
                        } else {
                            self.status =
                                EngineStatus::Failed(inventory.error.unwrap_or_else(|| {
                                    "No bundled or managed FFmpeg engine is available yet."
                                        .to_owned()
                                }));
                        }

                        finished = true;
                    }
                }
            }
        }

        if finished {
            self.receiver = None;
        }

        if self.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    pub fn status(&self) -> &EngineStatus {
        &self.status
    }

    pub fn active_info(&self) -> Option<&EngineInfo> {
        match &self.status {
            EngineStatus::Ready(info) => Some(info),
            _ => None,
        }
    }

    pub fn bundled_info(&self) -> Option<&EngineInfo> {
        self.bundled.as_ref()
    }

    pub fn managed_info(&self) -> Option<&EngineInfo> {
        self.managed.as_ref()
    }

    pub fn system_info(&self) -> Option<&EngineInfo> {
        self.system.as_ref()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn is_busy(&self) -> bool {
        self.receiver.is_some()
    }

    pub fn managed_engine_dir(&self) -> Option<PathBuf> {
        runtime::managed_engine_dir()
    }

    pub fn bundled_engine_dir(&self) -> Option<PathBuf> {
        runtime::bundled_engine_dir()
    }

    fn start_task(&mut self, action: TaskAction) {
        self.last_error = None;
        self.status = match action {
            TaskAction::Update => EngineStatus::Downloading {
                progress: 0.0,
                stage: "Preparing engine update".to_owned(),
            },
            TaskAction::Refresh | TaskAction::EnsureReady | TaskAction::UseBundled => {
                EngineStatus::Checking
            }
        };
        self.receiver = Some(spawn_task(action));
    }
}

#[derive(Clone, Copy)]
enum TaskAction {
    Refresh,
    EnsureReady,
    Update,
    UseBundled,
}

enum WorkerEvent {
    Progress { progress: f32, stage: String },
    Inventory(EngineInventory),
}

struct EngineInventory {
    active: Option<EngineInfo>,
    bundled: Option<EngineInfo>,
    managed: Option<EngineInfo>,
    system: Option<EngineInfo>,
    error: Option<String>,
}

fn spawn_task(action: TaskAction) -> Receiver<WorkerEvent> {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let inventory = match action {
            TaskAction::Refresh => scan_inventory(),
            TaskAction::EnsureReady => ensure_ready_inventory(&sender),
            TaskAction::Update => update_inventory(&sender),
            TaskAction::UseBundled => use_bundled_inventory(),
        };

        let _ = sender.send(WorkerEvent::Inventory(inventory));
    });

    receiver
}

fn ensure_ready_inventory(sender: &mpsc::Sender<WorkerEvent>) -> EngineInventory {
    let inventory = scan_inventory();
    if inventory.active.is_some() {
        return inventory;
    }

    let update_error = install_latest_managed_engine(sender).err();
    let mut refreshed = scan_inventory();
    if refreshed.active.is_none() {
        refreshed.error = update_error
            .or(refreshed.error)
            .or_else(|| Some("FFmpeg could not be installed automatically.".to_owned()));
    } else {
        refreshed.error = update_error;
    }
    refreshed
}

fn update_inventory(sender: &mpsc::Sender<WorkerEvent>) -> EngineInventory {
    let update_error = install_latest_managed_engine(sender).err();
    let mut refreshed = scan_inventory();
    refreshed.error = update_error.or(refreshed.error);
    refreshed
}

fn use_bundled_inventory() -> EngineInventory {
    let mut remove_error = None;

    if let Some(managed_dir) = runtime::managed_engine_dir() {
        if managed_dir.exists()
            && let Err(error) = fs::remove_dir_all(&managed_dir)
        {
            remove_error = Some(format!(
                "Could not remove managed engine from {}: {error}",
                managed_dir.display()
            ));
        }
    }

    let mut refreshed = scan_inventory();
    if refreshed.error.is_none() {
        refreshed.error = remove_error;
    }
    refreshed
}

fn scan_inventory() -> EngineInventory {
    let mut first_error = None;

    let managed = runtime::managed_engine_dir().and_then(|dir| {
        discover_engine_in_dir(&dir, EngineSource::ManagedUpdate)
            .map_err(|error| {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            })
            .ok()
            .flatten()
    });

    let bundled = runtime::bundled_engine_dir().and_then(|dir| {
        discover_engine_in_dir(&dir, EngineSource::Bundled)
            .map_err(|error| {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            })
            .ok()
            .flatten()
    });

    let system = discover_system_engine()
        .map_err(|error| {
            if first_error.is_none() {
                first_error = Some(error);
            }
        })
        .ok();

    let active = managed
        .clone()
        .or_else(|| bundled.clone())
        .or_else(|| system.clone());

    EngineInventory {
        active,
        bundled,
        managed,
        system,
        error: first_error,
    }
}

fn install_latest_managed_engine(sender: &mpsc::Sender<WorkerEvent>) -> Result<(), String> {
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

fn discover_engine_in_dir(dir: &Path, source: EngineSource) -> Result<Option<EngineInfo>, String> {
    if !runtime::engine_binaries_exist(dir) {
        return Ok(None);
    }

    let ffmpeg = dir.join(runtime::ffmpeg_binary_name());
    let ffprobe = dir.join(runtime::ffprobe_binary_name());
    let info = inspect_engine(ffmpeg, ffprobe, source)?;
    Ok(Some(info))
}

fn discover_system_engine() -> Result<EngineInfo, String> {
    inspect_engine(
        PathBuf::from(runtime::ffmpeg_binary_name()),
        PathBuf::from(runtime::ffprobe_binary_name()),
        EngineSource::SystemPath,
    )
}

fn inspect_engine(
    ffmpeg_path: PathBuf,
    ffprobe_path: PathBuf,
    source: EngineSource,
) -> Result<EngineInfo, String> {
    let mut version_command = background_command(&ffmpeg_path);
    version_command.arg("-version");
    let version_output = run_capture(version_command).map_err(|error| {
        format!(
            "Could not read FFmpeg version from {}: {error}",
            ffmpeg_path.display()
        )
    })?;

    let mut encoders_command = background_command(&ffmpeg_path);
    encoders_command.arg("-hide_banner").arg("-encoders");
    let encoders_output = run_capture(encoders_command)
        .map_err(|error| format!("Could not inspect FFmpeg encoders: {error}"))?;

    let h264_software = encoders_output.contains(" libx264 ");
    let h265_software = encoders_output.contains(" libx265 ");
    let av1_software = encoders_output.contains(" libsvtav1 ");

    let h264_nvidia = encoder_list_contains(&encoders_output, "h264_nvenc")
        && probe_hardware_encoder(&ffmpeg_path, "h264_nvenc");
    let h265_nvidia = encoder_list_contains(&encoders_output, "hevc_nvenc")
        && probe_hardware_encoder(&ffmpeg_path, "hevc_nvenc");
    let av1_nvidia = encoder_list_contains(&encoders_output, "av1_nvenc")
        && probe_hardware_encoder(&ffmpeg_path, "av1_nvenc");

    let h264_amd = encoder_list_contains(&encoders_output, "h264_amf")
        && probe_hardware_encoder(&ffmpeg_path, "h264_amf");
    let h265_amd = encoder_list_contains(&encoders_output, "hevc_amf")
        && probe_hardware_encoder(&ffmpeg_path, "hevc_amf");
    let av1_amd = encoder_list_contains(&encoders_output, "av1_amf")
        && probe_hardware_encoder(&ffmpeg_path, "av1_amf");

    Ok(EngineInfo {
        version: version_output
            .lines()
            .next()
            .map(str::trim)
            .unwrap_or("FFmpeg")
            .to_owned(),
        ffmpeg_path,
        ffprobe_path,
        encoders: EncoderAvailability {
            h264: h264_software || h264_nvidia || h264_amd,
            h265: h265_software || h265_nvidia || h265_amd,
            av1: av1_software || av1_nvidia || av1_amd,
            h264_nvidia,
            h265_nvidia,
            av1_nvidia,
            h264_amd,
            h265_amd,
            av1_amd,
        },
        source,
    })
}

fn encoder_list_contains(encoders_output: &str, encoder_name: &str) -> bool {
    encoders_output
        .lines()
        .any(|line| line.split_whitespace().any(|token| token == encoder_name))
}

fn probe_hardware_encoder(ffmpeg_path: &Path, encoder_name: &str) -> bool {
    let mut command = background_command(ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("color=c=black:s=64x64:d=0.1")
        .arg("-frames:v")
        .arg("1")
        .arg("-an")
        .arg("-c:v")
        .arg(encoder_name)
        .arg("-f")
        .arg("null")
        .arg("-");

    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
