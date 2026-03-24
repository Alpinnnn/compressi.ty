use std::{
    fs,
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::{
    modules::compress_videos::models::{EngineInfo, EngineSource},
    runtime,
};

use super::{
    discovery::{discover_engine_in_dir, discover_system_engine},
    install::install_latest_managed_engine,
};

#[derive(Clone, Copy)]
pub(super) enum TaskAction {
    Refresh,
    EnsureReady,
    Update,
    UseBundled,
}

pub(super) enum WorkerEvent {
    Progress { progress: f32, stage: String },
    Inventory(EngineInventory),
}

pub(super) struct EngineInventory {
    pub(super) active: Option<EngineInfo>,
    pub(super) bundled: Option<EngineInfo>,
    pub(super) managed: Option<EngineInfo>,
    pub(super) system: Option<EngineInfo>,
    pub(super) error: Option<String>,
}

pub(super) fn spawn_task(action: TaskAction) -> Receiver<WorkerEvent> {
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
