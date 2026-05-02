pub mod engine;
mod logic;
pub mod models;
pub mod processor;
mod ui;

use std::{collections::HashMap, path::PathBuf, sync::mpsc};

use eframe::egui::TextureHandle;

use crate::modules::compress_documents::{
    models::{DocumentCompressionSettings, DocumentQueueItem, LoadedDocument},
    processor::DocumentBatchHandle,
};

pub(crate) use self::processor::is_supported_path as is_supported_document_path;

/// Document compression workspace state and queue orchestration.
#[derive(Default)]
pub struct CompressDocumentsPage {
    queue: Vec<DocumentQueueItem>,
    settings: DocumentCompressionSettings,
    active_batch: Option<DocumentBatchHandle>,
    next_id: u64,
    selected_id: Option<u64>,
    output_dir: Option<PathBuf>,
    output_dir_user_set: bool,
    last_output_dir: Option<PathBuf>,
    banner: Option<BannerMessage>,
    file_loader_rx: Option<mpsc::Receiver<FileLoadResult>>,
    pending_add_count: usize,
    document_icon_textures: HashMap<&'static str, TextureHandle>,
}

struct FileLoadResult {
    results: Vec<Result<LoadedDocument, String>>,
}

pub(super) struct BannerMessage {
    tone: BannerTone,
    text: String,
}

pub(super) enum BannerTone {
    Info,
    Success,
    Error,
}
