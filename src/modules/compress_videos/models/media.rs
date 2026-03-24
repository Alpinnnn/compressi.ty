use std::path::PathBuf;

use crate::modules::compress_videos::models::{CompressionMode, VideoSettings};

/// Probed metadata for a selected video file.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoMetadata {
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
    pub duration_secs: f32,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub container_bitrate_kbps: Option<u32>,
    pub video_bitrate_kbps: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub video_codec: String,
    pub has_audio: bool,
}

impl VideoMetadata {
    /// Returns the original file size rounded up to whole megabytes.
    pub fn original_size_mb(&self) -> u32 {
        ((self.size_bytes as f64) / 1_048_576.0).ceil() as u32
    }
}

/// Live estimate shown during configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct CompressionEstimate {
    pub original_size_bytes: u64,
    pub estimated_size_bytes: u64,
    pub estimated_time_secs: f32,
    pub savings_percent: f32,
    pub target_width: u32,
    pub target_height: u32,
    pub pass_count: u8,
    pub recommendation: Option<CompressionRecommendation>,
}

/// Friendly recommendation shown above the action button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompressionRecommendation {
    pub headline: String,
    pub detail: String,
    pub mode: CompressionMode,
    pub target_size_mb: u32,
}

/// Progress updates emitted by background compression work.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessingProgress {
    pub progress: f32,
    pub stage: String,
    pub speed_x: f32,
    pub eta_secs: Option<f32>,
}

/// Final result shown on the done screen.
#[derive(Clone, Debug, PartialEq)]
pub struct CompressionResult {
    pub output_path: PathBuf,
    pub original_size_bytes: u64,
    pub output_size_bytes: u64,
    pub reduction_percent: f32,
    pub elapsed_secs: f32,
}

/// Per-video state in the queue.
#[derive(Clone, Debug)]
pub enum VideoCompressionState {
    /// Probing the file for metadata.
    Probing,
    /// Metadata ready, waiting for compression.
    Ready,
    /// Currently being compressed.
    Compressing(ProcessingProgress),
    /// Compression finished successfully.
    Completed(CompressionResult),
    /// Compression failed.
    Failed(String),
    /// Compression was cancelled.
    Cancelled,
}

/// Raw RGBA thumbnail data for a video.
#[derive(Clone, Debug)]
pub struct VideoThumbnail {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// A single video in the batch queue.
#[derive(Clone, Debug)]
pub struct VideoQueueItem {
    pub id: u64,
    pub metadata: Option<VideoMetadata>,
    pub settings: Option<VideoSettings>,
    pub state: VideoCompressionState,
    /// The original file path, available even before probe finishes.
    pub source_path: PathBuf,
    /// Display-friendly filename.
    pub file_name: String,
    /// Thumbnail extracted from the video via FFmpeg.
    pub thumbnail: Option<VideoThumbnail>,
}
