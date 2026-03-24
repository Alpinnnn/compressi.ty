use crate::modules::compress_videos::models::{EncoderAvailability, VideoMetadata};

/// User-facing modes for the simple video workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMode {
    ReduceSize,
    GoodQuality,
    CustomAdvanced,
}

impl CompressionMode {
    pub const ALL: [Self; 3] = [Self::ReduceSize, Self::GoodQuality, Self::CustomAdvanced];

    /// Returns the short label shown in the mode switcher.
    pub fn title(self) -> &'static str {
        match self {
            Self::ReduceSize => "Reduce Size",
            Self::GoodQuality => "Good Quality",
            Self::CustomAdvanced => "Custom (Advanced)",
        }
    }

    /// Returns the helper copy shown below the mode label.
    pub fn description(self) -> &'static str {
        match self {
            Self::ReduceSize => "Make the file as small as possible.",
            Self::GoodQuality => "Keep quality while reducing size.",
            Self::CustomAdvanced => "Full control for experienced users.",
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

    /// Returns the dropdown label for this resolution target.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Original => "Keep Original",
            Self::Hd1080 => "1080p",
            Self::Hd720 => "720p",
            Self::Sd480 => "480p",
        }
    }

    /// Returns the maximum output height to enforce for this choice.
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

    /// Returns the codec label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::H265 => "HEVC (H.265)",
            Self::Av1 => "AV1",
        }
    }

    /// Returns the software encoder name expected by FFmpeg.
    pub fn software_encoder_name(self) -> &'static str {
        match self {
            Self::H264 => "libx264",
            Self::H265 => "libx265",
            Self::Av1 => "libsvtav1",
        }
    }
}

/// Persistent settings for a video item in the queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoSettings {
    pub mode: CompressionMode,
    pub target_size_mb: u32,
    pub quality: u8,
    pub resolution: ResolutionChoice,
    pub custom_bitrate_kbps: u32,
    pub custom_codec: CodecChoice,
    pub custom_fps: u32,
    pub custom_audio_enabled: bool,
    pub custom_audio_bitrate_kbps: u32,
}

impl VideoSettings {
    /// Builds the default settings for a newly probed video file.
    pub fn new(
        video: &VideoMetadata,
        encoders: &EncoderAvailability,
        range: SizeSliderRange,
    ) -> Self {
        Self {
            mode: CompressionMode::ReduceSize,
            target_size_mb: range.recommended_mb,
            quality: 72,
            resolution: ResolutionChoice::Auto,
            custom_bitrate_kbps: default_custom_bitrate(video),
            custom_codec: encoders.quality_codec(),
            custom_fps: video.fps.round().clamp(12.0, 60.0) as u32,
            custom_audio_enabled: video.has_audio,
            custom_audio_bitrate_kbps: video.audio_bitrate_kbps.unwrap_or(128).clamp(64, 256),
        }
    }
}

fn default_custom_bitrate(video: &VideoMetadata) -> u32 {
    let fallback = (((video.size_bytes as f64 * 8.0) / video.duration_secs.max(1.0) as f64)
        / 1000.0)
        .round() as u32;
    video
        .video_bitrate_kbps
        .or(video.container_bitrate_kbps)
        .unwrap_or(fallback)
        .clamp(900, 18_000)
}
