use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eframe::egui;

use crate::{launch::LaunchImport, runtime};

const INBOX_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub enum InstanceState {
    Primary(PrimaryInstance),
    SecondaryForwarded,
}

pub struct PrimaryInstance {
    inbox_dir: PathBuf,
    #[cfg(target_os = "windows")]
    guard: WindowsInstanceGuard,
}

pub struct ExternalLaunchReceiver {
    receiver: mpsc::Receiver<LaunchImport>,
    #[cfg(target_os = "windows")]
    _guard: WindowsInstanceGuard,
}

impl ExternalLaunchReceiver {
    pub fn try_recv(&mut self) -> Option<LaunchImport> {
        self.receiver.try_recv().ok()
    }
}

impl PrimaryInstance {
    pub fn start(self, ctx: &egui::Context) -> ExternalLaunchReceiver {
        let inbox_dir = self.inbox_dir.clone();
        let repaint_ctx = ctx.clone();
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || watch_launch_inbox(inbox_dir, sender, repaint_ctx));

        ExternalLaunchReceiver {
            receiver,
            #[cfg(target_os = "windows")]
            _guard: self.guard,
        }
    }
}

pub fn initialize(launch_import: &LaunchImport) -> io::Result<InstanceState> {
    let inbox_dir = launch_inbox_dir();
    fs::create_dir_all(&inbox_dir)?;

    #[cfg(target_os = "windows")]
    {
        let guard = WindowsInstanceGuard::acquire()?;
        if guard.is_primary() {
            return Ok(InstanceState::Primary(PrimaryInstance { inbox_dir, guard }));
        }

        forward_to_existing_instance(&inbox_dir, launch_import)?;
        return Ok(InstanceState::SecondaryForwarded);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = launch_import;
        Ok(InstanceState::Primary(PrimaryInstance { inbox_dir }))
    }
}

fn watch_launch_inbox(inbox_dir: PathBuf, sender: mpsc::Sender<LaunchImport>, ctx: egui::Context) {
    loop {
        let _ = drain_launch_inbox(&inbox_dir, &sender, &ctx);
        thread::sleep(INBOX_POLL_INTERVAL);
    }
}

fn drain_launch_inbox(
    inbox_dir: &Path,
    sender: &mpsc::Sender<LaunchImport>,
    ctx: &egui::Context,
) -> io::Result<()> {
    let mut launch_files: Vec<PathBuf> = fs::read_dir(inbox_dir)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("launch"))
        .collect();
    launch_files.sort();

    for path in launch_files {
        let payload = match fs::read_to_string(&path) {
            Ok(payload) => payload,
            Err(_) => {
                let _ = fs::remove_file(&path);
                continue;
            }
        };

        if let Some(launch_import) = LaunchImport::from_ipc_payload(&payload) {
            if sender.send(launch_import).is_err() {
                return Ok(());
            }
            ctx.request_repaint();
        }

        let _ = fs::remove_file(&path);
    }

    Ok(())
}

fn forward_to_existing_instance(inbox_dir: &Path, launch_import: &LaunchImport) -> io::Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let base_name = format!("{timestamp:020}-{}", std::process::id());
    let temp_path = inbox_dir.join(format!("{base_name}.tmp"));
    let final_path = inbox_dir.join(format!("{base_name}.launch"));

    fs::write(&temp_path, launch_import.to_ipc_payload())?;
    fs::rename(temp_path, final_path)
}

fn launch_inbox_dir() -> PathBuf {
    runtime::data_dir()
        .unwrap_or_else(|| std::env::temp_dir().join(runtime::APP_DIR_NAME))
        .join("launch-inbox")
}

#[cfg(target_os = "windows")]
struct WindowsInstanceGuard {
    handle: windows_sys::Win32::Foundation::HANDLE,
    primary: bool,
}

#[cfg(target_os = "windows")]
impl WindowsInstanceGuard {
    fn acquire() -> io::Result<Self> {
        use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};
        use windows_sys::Win32::{
            Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
            System::Threading::CreateMutexW,
        };

        let name = OsStr::new("Local\\Compressity.PrimaryInstance")
            .encode_wide()
            .chain(once(0))
            .collect::<Vec<u16>>();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }

        let primary = unsafe { GetLastError() } != ERROR_ALREADY_EXISTS;
        Ok(Self { handle, primary })
    }

    fn is_primary(&self) -> bool {
        self.primary
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.handle);
            }
        }
    }
}
