use std::path::PathBuf;

/// User-facing modes for the simple video workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMode {
    ReduceSize,
    GoodQuality,
    CustomAdvanced,
}

impl CompressionMode {
    pub const ALL: [Self; 3] = [
        Self::ReduceSize,
        Self::GoodQuality,
        Self::CustomAdvanced,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::ReduceSize => "Reduce Size",
            Self::GoodQuality => "Good Quality",
            Self::CustomAdvanced => "Custom (Advanced)",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::ReduceSize => "Make the file as small as possible.",
            Self::GoodQuality => "Keep quality while reducing size.",
            Self::CustomAdvanced => "Full control for experienced users.",
        }
    }
}

/// Quick targets for the size-first mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReducePreset {
    WhatsApp,
    Email,
    StorageSaver,
}

impl ReducePreset {
    pub const ALL: [Self; 3] = [Self::WhatsApp, Self::Email, Self::StorageSaver];

    pub fn label(self) -> &'static str {
        match self {
            Self::WhatsApp => "WhatsApp",
            Self::Email => "Email",
            Self::StorageSaver => "Storage Saver",
        }
    }
}

/// Output size caps used by the simple slider UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeSliderRange {
    pub min_mb: u32,
    pub max_mb: u32,
    pub recommended_mb: u32,
}

/// Resolution choices exposed by the quick and advanced flows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionChoice {
    Auto,
    Original,
    Hd1080,
    Hd720,
    Sd480,
}

impl ResolutionChoice {
    pub const QUICK: [Self; 4] = [Self::Auto, Self::Original, Self::Hd1080, Self::Hd720];
    pub const ADVANCED: [Self; 5] = [
        Self::Auto,
        Self::Original,
        Self::Hd1080,
        Self::Hd720,
        Self::Sd480,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Original => "Keep Original",
            Self::Hd1080 => "1080p",
            Self::Hd720 => "720p",
            Self::Sd480 => "480p",
        }
    }

    pub fn max_height(self) -> Option<u32> {
        match self {
            Self::Auto | Self::Original => None,
            Self::Hd1080 => Some(1080),
            Self::Hd720 => Some(720),
            Self::Sd480 => Some(480),
        }
    }
}

/// Codec choices shown only in advanced mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecChoice {
    H264,
    H265,
    Av1,
}

impl CodecChoice {
    pub const ALL: [Self; 3] = [Self::H264, Self::H265, Self::Av1];

    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::H265 => "HEVC (H.265)",
            Self::Av1 => "AV1",
        }
    }

    pub fn encoder_name(self) -> &'static str {
        match self {
            Self::H264 => "libx264",
            Self::H265 => "libx265",
            Self::Av1 => "libsvtav1",
        }
    }
}

/// The encoders discovered in the local FFmpeg build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncoderAvailability {
    pub h264: bool,
    pub h265: bool,
    pub av1: bool,
}

impl EncoderAvailability {
    pub fn supports(&self, codec: CodecChoice) -> bool {
        match codec {
            CodecChoice::H264 => self.h264,
            CodecChoice::H265 => self.h265,
            CodecChoice::Av1 => self.av1,
        }
    }

    pub fn fallback_codec(&self) -> CodecChoice {
        if self.h264 {
            CodecChoice::H264
        } else if self.h265 {
            CodecChoice::H265
        } else {
            CodecChoice::Av1
        }
    }

    pub fn reduce_size_codec(&self) -> CodecChoice {
        if self.h265 {
            CodecChoice::H265
        } else {
            self.fallback_codec()
        }
    }

    pub fn quality_codec(&self) -> CodecChoice {
        if self.h264 {
            CodecChoice::H264
        } else {
            self.fallback_codec()
        }
    }
}

/// Where the active FFmpeg toolchain comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineSource {
    ManagedUpdate,
    Bundled,
    SystemPath,
}

impl EngineSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::ManagedUpdate => "Managed Update",
            Self::Bundled => "Bundled",
            Self::SystemPath => "System PATH",
        }
    }
}

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
    pub fn original_size_mb(&self) -> u32 {
        ((self.size_bytes as f64) / 1_048_576.0).ceil() as u32
    }
}

/// Persistent settings for a video item in the queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoSettings {
    pub mode: CompressionMode,
    pub target_size_mb: u32,
    pub reduce_preset: Option<ReducePreset>,
    pub quality: u8,
    pub resolution: ResolutionChoice,
    pub custom_bitrate_kbps: u32,
    pub custom_codec: CodecChoice,
    pub custom_fps: u32,
}

impl VideoSettings {
    pub fn new(video: &VideoMetadata, encoders: &EncoderAvailability, range: SizeSliderRange) -> Self {
        Self {
            mode: CompressionMode::ReduceSize,
            target_size_mb: range.recommended_mb,
            reduce_preset: None,
            quality: 72,
            resolution: ResolutionChoice::Auto,
            custom_bitrate_kbps: default_custom_bitrate(video),
            custom_codec: encoders.quality_codec(),
            custom_fps: video.fps.round().clamp(12.0, 60.0) as u32,
        }
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

/// Resolved FFmpeg installation details for the current device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineInfo {
    pub version: String,
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
    pub encoders: EncoderAvailability,
    pub source: EngineSource,
}

/// High-level status for the local video engine.
#[derive(Clone, Debug, PartialEq)]
pub enum EngineStatus {
    Checking,
    Downloading { progress: f32, stage: String },
    Ready(EngineInfo),
    Failed(String),
}

impl Default for EngineStatus {
    fn default() -> Self {
        Self::Checking
    }
}

/// Progress updates emitted by preview and compression jobs.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessingProgress {
    pub progress: f32,
    pub stage: String,
    pub speed_x: f32,
    pub eta_secs: Option<f32>,
}

/// Preview artefacts generated from a 5 second sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewResult {
    pub original_clip_path: PathBuf,
    pub compressed_clip_path: PathBuf,
    pub original_size_bytes: u64,
    pub compressed_size_bytes: u64,
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
    /// The original file path (available even before probe finishes).
    pub source_path: PathBuf,
    /// Display-friendly filename.
    pub file_name: String,
    /// Thumbnail extracted from the video via FFmpeg.
    pub thumbnail: Option<VideoThumbnail>,
}

fn default_custom_bitrate(video: &VideoMetadata) -> u32 {
    let fallback = (((video.size_bytes as f64 * 8.0) / video.duration_secs.max(1.0) as f64) / 1000.0)
        .round() as u32;
    video
        .video_bitrate_kbps
        .or(video.container_bitrate_kbps)
        .unwrap_or(fallback)
        .clamp(900, 18_000)
}
