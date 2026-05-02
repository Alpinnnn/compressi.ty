use std::{
    fs,
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::{modules::compress_documents::engine::DocumentEngineInventory, runtime};

use super::{
    discovery::{
        discover_inventory_in_bundled_dirs, discover_inventory_in_managed_dirs,
        discover_system_inventory,
    },
    install::install_latest_managed_document_engines,
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
    pub(super) active: DocumentEngineInventory,
    pub(super) bundled: DocumentEngineInventory,
    pub(super) managed: DocumentEngineInventory,
    pub(super) system: DocumentEngineInventory,
    pub(super) error: Option<String>,
}

pub(super) fn spawn_task(action: TaskAction) -> Receiver<WorkerEvent> {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let inventory = match action {
            TaskAction::Refresh => scan_inventory(None),
            TaskAction::EnsureReady => ensure_ready_inventory(&sender),
            TaskAction::Update => update_inventory(&sender),
            TaskAction::UseBundled => use_bundled_inventory(),
        };

        let _ = sender.send(WorkerEvent::Inventory(inventory));
    });

    receiver
}

fn ensure_ready_inventory(sender: &mpsc::Sender<WorkerEvent>) -> EngineInventory {
    let inventory = scan_inventory(None);
    if inventory.active.has_all_recommended() {
        return inventory;
    }

    let update_error = install_latest_managed_document_engines(sender).err();
    let mut refreshed = scan_inventory(update_error);
    if !refreshed.active.is_ready() {
        refreshed.error = refreshed
            .error
            .or_else(|| Some("Document engines could not be installed automatically.".to_owned()));
    }
    refreshed
}

fn update_inventory(sender: &mpsc::Sender<WorkerEvent>) -> EngineInventory {
    let update_error = install_latest_managed_document_engines(sender).err();
    scan_inventory(update_error)
}

fn use_bundled_inventory() -> EngineInventory {
    let mut errors = Vec::new();

    for dir in [
        runtime::managed_pdf_engine_dir(),
        runtime::managed_package_engine_dir(),
    ]
    .into_iter()
    .flatten()
    {
        if dir.exists()
            && let Err(error) = fs::remove_dir_all(&dir)
        {
            errors.push(format!(
                "Could not remove managed document engine from {}: {error}",
                dir.display()
            ));
        }
    }

    scan_inventory(if errors.is_empty() {
        None
    } else {
        Some(errors.join("\n"))
    })
}

fn scan_inventory(error: Option<String>) -> EngineInventory {
    let managed = discover_inventory_in_managed_dirs();
    let bundled = discover_inventory_in_bundled_dirs();
    let system = discover_system_inventory();
    let active = combine_inventory(&managed, &bundled, &system);

    EngineInventory {
        active,
        bundled,
        managed,
        system,
        error,
    }
}

fn combine_inventory(
    managed: &DocumentEngineInventory,
    bundled: &DocumentEngineInventory,
    system: &DocumentEngineInventory,
) -> DocumentEngineInventory {
    DocumentEngineInventory {
        ghostscript: managed
            .ghostscript
            .clone()
            .or_else(|| bundled.ghostscript.clone())
            .or_else(|| system.ghostscript.clone()),
        qpdf: managed
            .qpdf
            .clone()
            .or_else(|| bundled.qpdf.clone())
            .or_else(|| system.qpdf.clone()),
        seven_zip: managed
            .seven_zip
            .clone()
            .or_else(|| bundled.seven_zip.clone())
            .or_else(|| system.seven_zip.clone()),
    }
}
