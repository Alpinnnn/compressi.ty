mod logic;
mod ui;

pub mod compressor;
pub mod models;

use std::{path::PathBuf, sync::mpsc};

use eframe::egui::{TextureHandle, Vec2};

use crate::modules::compress_photos::{
    compressor::CompressionHandle,
    models::{CompressionSettings, CompressionState, LoadedPhoto, PhotoAsset, PhotoPreview},
};

/// Photo compression workspace state and preview orchestration.
pub struct CompressPhotosPage {
    files: Vec<PhotoListItem>,
    settings: CompressionSettings,
    active_batch: Option<CompressionHandle>,
    next_file_id: u64,
    last_output_dir: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    output_dir_user_set: bool,
    banner: Option<BannerMessage>,
    selected_file_id: Option<u64>,
    preview_zoom: f32,
    preview_offset: Vec2,
    preview_output_texture: Option<(u64, TextureHandle)>,
    preview_input_texture: Option<(u64, TextureHandle)>,
    before_after_split: f32,
    preview_loader_rx: Option<mpsc::Receiver<PreviewLoadEvent>>,
    preview_loading: bool,
    preview_load_progress: f32,
    /// True when the output image was attempted but failed to decode.
    preview_output_failed: bool,
    /// Background channel for async file loading to avoid UI freezes.
    file_loader_rx: Option<mpsc::Receiver<FileLoadResult>>,
    pending_add_count: usize,
    pending_loaded_photos: Vec<LoadedPhoto>,
}

struct FileLoadResult {
    results: Vec<Result<LoadedPhoto, String>>,
}

struct PhotoListItem {
    asset: PhotoAsset,
    preview_texture: Option<TextureHandle>,
    state: CompressionState,
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

impl Default for BannerTone {
    fn default() -> Self {
        Self::Info
    }
}

struct PreviewLoadEvent {
    id: u64,
    kind: PreviewLoadKind,
    preview: Option<PhotoPreview>,
    progress: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum PreviewLoadKind {
    Input,
    Output,
    Progress,
}

impl Default for CompressPhotosPage {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            settings: crate::modules::compress_photos::models::CompressionSettings::default(),
            active_batch: None,
            next_file_id: 0,
            last_output_dir: None,
            output_dir: None,
            output_dir_user_set: false,
            banner: None,
            selected_file_id: None,
            preview_zoom: 1.0,
            preview_offset: Vec2::ZERO,
            preview_output_texture: None,
            preview_input_texture: None,
            before_after_split: 0.5,
            preview_loader_rx: None,
            preview_loading: false,
            preview_load_progress: 0.0,
            preview_output_failed: false,
            file_loader_rx: None,
            pending_add_count: 0,
            pending_loaded_photos: Vec::new(),
        }
    }
}
