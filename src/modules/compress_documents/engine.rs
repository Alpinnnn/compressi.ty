mod discovery;
mod install;
mod inventory;
mod process_utils;

use std::{path::PathBuf, sync::mpsc::Receiver, time::Duration};

use crate::runtime;

use self::inventory::{TaskAction, WorkerEvent, spawn_task};

pub(crate) use self::discovery::{
    discover_ghostscript_binary, discover_qpdf_binary, discover_seven_zip_binary,
};

/// Where a document compression engine was discovered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentEngineSource {
    ManagedUpdate,
    Bundled,
    SystemPath,
}

impl DocumentEngineSource {
    /// Returns the source label shown in the settings UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::ManagedUpdate => "Managed Update",
            Self::Bundled => "Bundled",
            Self::SystemPath => "System PATH",
        }
    }
}

/// Document engine role used by the compression pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentEngineKind {
    Pdf,
    PdfPolish,
    PackageZip,
}

impl DocumentEngineKind {
    /// Returns the engine role label shown in settings.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::PdfPolish => "PDF Polish",
            Self::PackageZip => "ZIP Package",
        }
    }

    /// Returns the binary family expected for this engine role.
    pub fn binary_label(self) -> &'static str {
        match self {
            Self::Pdf => "Ghostscript",
            Self::PdfPolish => "qpdf",
            Self::PackageZip => "7-Zip",
        }
    }
}

/// Resolved executable metadata for a document compression engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentEngineInfo {
    pub kind: DocumentEngineKind,
    pub source: DocumentEngineSource,
    pub version: String,
    pub path: PathBuf,
}

/// Complete document engine inventory for one source layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentEngineInventory {
    pub ghostscript: Option<DocumentEngineInfo>,
    pub qpdf: Option<DocumentEngineInfo>,
    pub seven_zip: Option<DocumentEngineInfo>,
}

impl DocumentEngineInventory {
    /// Returns true when every supported document engine is available.
    pub fn has_all_recommended(&self) -> bool {
        self.ghostscript.is_some() && self.qpdf.is_some() && self.seven_zip.is_some()
    }

    /// Returns true when compression can run for the core PDF and package families.
    pub fn is_ready(&self) -> bool {
        self.ghostscript.is_some() && self.seven_zip.is_some()
    }

    /// Returns all discovered engines in display order.
    pub fn infos(&self) -> Vec<&DocumentEngineInfo> {
        let mut infos = Vec::new();
        if let Some(info) = &self.ghostscript {
            infos.push(info);
        }
        if let Some(info) = &self.qpdf {
            infos.push(info);
        }
        if let Some(info) = &self.seven_zip {
            infos.push(info);
        }
        infos
    }
}

/// High-level status for document engine discovery and updates.
#[derive(Clone, Debug, PartialEq)]
pub enum DocumentEngineStatus {
    Checking,
    Downloading { progress: f32, stage: String },
    Ready(DocumentEngineInventory),
    Failed(String),
}

impl Default for DocumentEngineStatus {
    fn default() -> Self {
        Self::Checking
    }
}

/// Coordinates discovery, updates, and status tracking for document engines.
#[derive(Default)]
pub struct DocumentEngineController {
    status: DocumentEngineStatus,
    bundled: DocumentEngineInventory,
    managed: DocumentEngineInventory,
    system: DocumentEngineInventory,
    last_error: Option<String>,
    receiver: Option<Receiver<WorkerEvent>>,
}

impl DocumentEngineController {
    /// Refreshes document engine inventory from bundled, managed, and system locations.
    pub fn refresh(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::Refresh);
    }

    /// Ensures recommended document engines are available, installing managed engines if needed.
    pub fn ensure_ready(&mut self) {
        if self.is_busy()
            || self
                .active_inventory()
                .map(DocumentEngineInventory::has_all_recommended)
                .unwrap_or(false)
        {
            return;
        }

        self.start_task(TaskAction::EnsureReady);
    }

    /// Starts a managed update for Ghostscript, qpdf, and 7-Zip.
    pub fn update_to_latest(&mut self) {
        if self.is_busy() {
            return;
        }

        self.start_task(TaskAction::Update);
    }

    /// Switches back to bundled document engines when available.
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
                        self.status = DocumentEngineStatus::Downloading { progress, stage };
                    }
                    WorkerEvent::Inventory(inventory) => {
                        let friendly_error = inventory
                            .error
                            .as_deref()
                            .map(friendly_document_engine_error);
                        self.bundled = inventory.bundled;
                        self.managed = inventory.managed;
                        self.system = inventory.system;
                        self.last_error = friendly_error.clone();

                        if inventory.active.is_ready() {
                            self.status = DocumentEngineStatus::Ready(inventory.active);
                        } else {
                            self.status =
                                DocumentEngineStatus::Failed(friendly_error.unwrap_or_else(|| {
                                    "No complete document engine set is available yet.".to_owned()
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

    /// Returns the current document engine status for UI rendering.
    pub fn status(&self) -> &DocumentEngineStatus {
        &self.status
    }

    /// Returns the active engine set selected for document operations, if ready.
    pub fn active_inventory(&self) -> Option<&DocumentEngineInventory> {
        match &self.status {
            DocumentEngineStatus::Ready(inventory) => Some(inventory),
            _ => None,
        }
    }

    /// Returns bundled document engines discovered beside the app.
    pub fn bundled_inventory(&self) -> &DocumentEngineInventory {
        &self.bundled
    }

    /// Returns managed document engines installed in local app data.
    pub fn managed_inventory(&self) -> &DocumentEngineInventory {
        &self.managed
    }

    /// Returns document engines discovered on the system PATH.
    pub fn system_inventory(&self) -> &DocumentEngineInventory {
        &self.system
    }

    /// Returns the most recent discovery or update error message.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Returns true when the latest failure can be fixed by restarting as Administrator.
    pub fn needs_administrator_restart(&self) -> bool {
        self.last_error
            .as_deref()
            .map(is_elevation_error)
            .unwrap_or_else(|| match &self.status {
                DocumentEngineStatus::Failed(error) => is_elevation_error(error),
                _ => false,
            })
    }

    /// Stores a UI-level engine error so the user can see why an action failed.
    pub fn record_error(&mut self, error: String) {
        self.last_error = Some(error.clone());
        self.status = DocumentEngineStatus::Failed(error);
    }

    /// Returns whether an inventory or update task is still running.
    pub fn is_busy(&self) -> bool {
        self.receiver.is_some()
    }

    /// Returns the managed PDF engine directory used for downloaded toolchains.
    pub fn managed_pdf_engine_dir(&self) -> Option<PathBuf> {
        runtime::managed_pdf_engine_dir()
    }

    /// Returns the managed package-engine directory used for downloaded toolchains.
    pub fn managed_package_engine_dir(&self) -> Option<PathBuf> {
        runtime::managed_package_engine_dir()
    }

    /// Returns the bundled PDF engine directory shipped with the app.
    pub fn bundled_pdf_engine_dir(&self) -> Option<PathBuf> {
        runtime::bundled_pdf_engine_dir()
    }

    /// Returns the bundled package-engine directory shipped with the app.
    pub fn bundled_package_engine_dir(&self) -> Option<PathBuf> {
        runtime::bundled_package_engine_dir()
    }

    fn start_task(&mut self, action: TaskAction) {
        self.last_error = None;
        self.status = match action {
            TaskAction::Update => DocumentEngineStatus::Downloading {
                progress: 0.0,
                stage: "Preparing document engine update".to_owned(),
            },
            TaskAction::Refresh | TaskAction::EnsureReady | TaskAction::UseBundled => {
                DocumentEngineStatus::Checking
            }
        };
        self.receiver = Some(spawn_task(action));
    }
}

fn friendly_document_engine_error(error: &str) -> String {
    if is_elevation_error(error) {
        return "Windows needs Administrator permission to install the document engines. Restart Compressi.ty as Administrator, then the Ghostscript and 7-Zip installation will continue automatically.".to_owned();
    }

    error.to_owned()
}

fn is_elevation_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("os error 740")
        || lower.contains("requires elevation")
        || lower.contains("administrator permission")
        || lower.contains("as administrator")
}
