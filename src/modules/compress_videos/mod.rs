mod logic;
mod preview_logic;
mod preview_runtime;
mod ui;

pub mod engine;
pub mod models;
pub mod processor;

use std::{collections::HashMap, path::PathBuf, sync::mpsc};

use eframe::egui::TextureHandle;

use crate::modules::compress_videos::{
    models::{
        EncoderAvailability, VideoMetadata, VideoPreviewState, VideoQueueItem, VideoThumbnail,
    },
    processor::BatchHandle,
};

use self::preview_runtime::RunningPreviewStream;

/// Video compression workspace state and queue orchestration.
pub struct CompressVideosPage {
    queue: Vec<VideoQueueItem>,
    next_id: u64,
    selected_id: Option<u64>,

    active_batch: Option<BatchHandle>,
    pending_compression_ids: Vec<u64>,
    pending_probes: Vec<PendingProbe>,

    output_dir: Option<PathBuf>,
    output_dir_user_set: bool,
    last_output_dir: Option<PathBuf>,

    banner: Option<BannerMessage>,
    show_cancel_all_confirm: bool,

    /// Cached GPU textures keyed by queue item id.
    thumbnail_textures: HashMap<u64, TextureHandle>,
    preview_state: VideoPreviewState,
    preview_texture: Option<TextureHandle>,
    preview_texture_dirty: bool,
    running_preview_stream: Option<RunningPreviewStream>,
}

struct PendingProbe {
    id: u64,
    encoders: EncoderAvailability,
    receiver: mpsc::Receiver<ProbeResult>,
}

/// Combined result of probing + thumbnail generation.
struct ProbeResult {
    metadata: Result<VideoMetadata, String>,
    thumbnail: Option<VideoThumbnail>,
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
