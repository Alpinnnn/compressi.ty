mod discovery;
mod install;
mod inventory;
mod process_utils;

use std::{path::PathBuf, sync::mpsc::Receiver, time::Duration};

use crate::{
    modules::compress_videos::models::{EngineInfo, EngineStatus},
    runtime,
};

use self::inventory::{TaskAction, WorkerEvent, spawn_task};

/// Coordinates discovery, updates, and status tracking for the local FFmpeg toolchain.
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
    /// Refreshes engine inventory from bundled, managed, and system locations.
    pub fn refresh(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::Refresh);
    }

    /// Ensures an engine becomes available, installing or activating one if needed.
    pub fn ensure_ready(&mut self) {
        if self.is_busy() || self.active_info().is_some() {
            return;
        }

        self.start_task(TaskAction::EnsureReady);
    }

    /// Starts a managed update to the latest supported engine bundle.
    pub fn update_to_latest(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::Update);
    }

    /// Switches back to the bundled engine package when available.
    pub fn use_bundled_engine(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::UseBundled);
    }

    /// Polls the worker channel and returns the desired repaint cadence while work is active.
    pub fn poll(&mut self) -> Option<Duration> {
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
            Some(Duration::from_millis(50))
        } else {
            None
        }
    }

    /// Returns the current high-level engine status for UI rendering.
    pub fn status(&self) -> &EngineStatus {
        &self.status
    }

    /// Returns the active engine selected for video operations, if ready.
    pub fn active_info(&self) -> Option<&EngineInfo> {
        match &self.status {
            EngineStatus::Ready(info) => Some(info),
            _ => None,
        }
    }

    /// Returns metadata about the bundled engine, if one was discovered.
    pub fn bundled_info(&self) -> Option<&EngineInfo> {
        self.bundled.as_ref()
    }

    /// Returns metadata about the managed engine installation, if available.
    pub fn managed_info(&self) -> Option<&EngineInfo> {
        self.managed.as_ref()
    }

    /// Returns metadata about an FFmpeg installation found on the system PATH.
    pub fn system_info(&self) -> Option<&EngineInfo> {
        self.system.as_ref()
    }

    /// Returns the most recent discovery or update error message.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Returns whether an inventory or update task is still running.
    pub fn is_busy(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns the managed engine directory used for downloaded toolchains.
    pub fn managed_engine_dir(&self) -> Option<PathBuf> {
        runtime::managed_engine_dir()
    }

    /// Returns the bundled engine directory shipped with the app.
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
