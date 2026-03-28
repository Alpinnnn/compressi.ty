use std::path::PathBuf;

/// Main workflow mode for the audio workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioWorkflowMode {
    Auto,
    Manual,
}

/// Smart mode presets focused on the user's goal instead of codec jargon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioAutoPreset {
    HighQuality,
    Balanced,
    SmallSize,
}

impl AudioAutoPreset {
    /// Returns the short user-facing preset label.
    pub fn label(self) -> &'static str {
        match self {
            Self::HighQuality => "High Quality",
            Self::Balanced => "Balanced",
            Self::SmallSize => "Small Size",
        }
    }

    /// Returns the helper copy shown under the preset selector.
    pub fn detail(self) -> &'static str {
        match self {
            Self::HighQuality => "Keeps more detail while still shrinking the file.",
            Self::Balanced => "Best mix of smaller size and clean playback.",
            Self::SmallSize => "Pushes for the lightest file that still sounds good.",
        }
    }
}

/// Output formats exposed to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Aac,
    Opus,
    Flac,
}

impl AudioFormat {
    /// Returns the short user-facing format label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Mp3 => "MP3",
            Self::Aac => "AAC",
            Self::Opus => "OPUS",
            Self::Flac => "FLAC",
        }
    }

    /// Returns the output file extension used for this format.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Aac => "m4a",
            Self::Opus => "opus",
            Self::Flac => "flac",
        }
    }

    /// Returns whether the format preserves audio samples without quality loss.
    pub fn is_lossless(self) -> bool {
        matches!(self, Self::Flac)
    }
}

/// Lightweight content classification used by Smart Mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioContentKind {
    Voice,
    Music,
    Mixed,
}

impl AudioContentKind {
    /// Returns the short user-facing content label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Voice => "Voice",
            Self::Music => "Music",
            Self::Mixed => "Mixed",
        }
    }
}

/// User-configurable audio compression settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioCompressionSettings {
    pub mode: AudioWorkflowMode,
    pub auto_preset: AudioAutoPreset,
    pub manual_format: AudioFormat,
    pub manual_bitrate_kbps: u32,
    pub advanced_open: bool,
    pub manual_sample_rate_hz: Option<u32>,
    pub manual_channels: Option<u8>,
    pub normalize_volume: bool,
    pub remove_metadata: bool,
    pub convert_format_only: bool,
}

impl Default for AudioCompressionSettings {
    fn default() -> Self {
        Self {
            mode: AudioWorkflowMode::Auto,
            auto_preset: AudioAutoPreset::Balanced,
            manual_format: AudioFormat::Aac,
            manual_bitrate_kbps: 128,
            advanced_open: false,
            manual_sample_rate_hz: None,
            manual_channels: None,
            normalize_volume: false,
            remove_metadata: false,
            convert_format_only: false,
        }
    }
}

/// Probed metadata for a selected audio file.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioMetadata {
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
    pub duration_secs: f32,
    pub audio_bitrate_kbps: Option<u32>,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub codec_name: String,
    pub container_name: String,
    pub is_lossless: bool,
}

impl AudioMetadata {
    /// Returns whether the source is already compressed with a lossy codec.
    pub fn is_lossy(&self) -> bool {
        !self.is_lossless
    }
}

/// Result of Smart Mode analysis shown in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioAnalysis {
    pub content_kind: AudioContentKind,
    pub headline: String,
    pub detail: String,
}

/// Live estimate shown in the settings panel before encoding starts.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioEstimate {
    pub original_size_bytes: u64,
    pub estimated_size_bytes: u64,
    pub savings_percent: f32,
    pub output_format: AudioFormat,
    pub target_bitrate_kbps: Option<u32>,
    pub effective_sample_rate_hz: Option<u32>,
    pub effective_channels: Option<u8>,
    pub warnings: Vec<String>,
    pub recommendation: Option<String>,
    pub should_skip: bool,
    pub skip_reason: Option<String>,
}

/// Compression plan resolved from the current settings plus source analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioCompressionPlan {
    pub output_format: AudioFormat,
    pub encoder_name: &'static str,
    pub target_bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub content_kind: AudioContentKind,
    pub warnings: Vec<String>,
    pub recommendation: Option<String>,
    pub estimated_size_bytes: u64,
    pub should_skip: bool,
    pub skip_reason: Option<String>,
}

/// Progress updates emitted by the background audio worker.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioProcessingProgress {
    pub progress: f32,
    pub stage: String,
    pub speed_x: f32,
    pub eta_secs: Option<f32>,
}

/// Final output details after compression completes.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioCompressionResult {
    pub output_path: PathBuf,
    pub original_size_bytes: u64,
    pub output_size_bytes: u64,
    pub reduction_percent: f32,
    pub elapsed_secs: f32,
}

/// Per-item queue state in the audio module.
#[derive(Clone, Debug)]
pub enum AudioCompressionState {
    Analyzing,
    Ready,
    Compressing(AudioProcessingProgress),
    Completed(AudioCompressionResult),
    Skipped(String),
    Failed(String),
    Cancelled,
}

/// Queue item tracked by the audio module UI.
#[derive(Clone, Debug)]
pub struct AudioQueueItem {
    pub id: u64,
    pub source_path: PathBuf,
    pub file_name: String,
    pub metadata: Option<AudioMetadata>,
    pub analysis: Option<AudioAnalysis>,
    pub state: AudioCompressionState,
}
