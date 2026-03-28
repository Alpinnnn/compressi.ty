use std::path::PathBuf;

use crate::modules::compress_videos::models::CodecChoice;

/// Encoder backends supported by the local FFmpeg installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncoderBackend {
    Software,
    Nvidia,
    Amd,
    IntelQuickSync,
}

/// The exact codec and backend selected for a compression plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedEncoder {
    pub codec: CodecChoice,
    pub backend: EncoderBackend,
}

impl ResolvedEncoder {
    /// Returns the FFmpeg encoder identifier for this backend and codec pair.
    pub fn ffmpeg_name(self) -> &'static str {
        match (self.backend, self.codec) {
            (EncoderBackend::Software, codec) => codec.software_encoder_name(),
            (EncoderBackend::Nvidia, CodecChoice::H264) => "h264_nvenc",
            (EncoderBackend::Nvidia, CodecChoice::H265) => "hevc_nvenc",
            (EncoderBackend::Nvidia, CodecChoice::Av1) => "av1_nvenc",
            (EncoderBackend::Amd, CodecChoice::H264) => "h264_amf",
            (EncoderBackend::Amd, CodecChoice::H265) => "hevc_amf",
            (EncoderBackend::Amd, CodecChoice::Av1) => "av1_amf",
            (EncoderBackend::IntelQuickSync, CodecChoice::H264) => "h264_qsv",
            (EncoderBackend::IntelQuickSync, CodecChoice::H265) => "hevc_qsv",
            (EncoderBackend::IntelQuickSync, CodecChoice::Av1) => "av1_qsv",
        }
    }

    /// Returns whether the encoder uses a hardware backend.
    pub fn is_hardware(self) -> bool {
        !matches!(self.backend, EncoderBackend::Software)
    }
}

/// The encoders discovered in the local FFmpeg build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncoderAvailability {
    pub h264: bool,
    pub h265: bool,
    pub av1: bool,
    pub aac: bool,
    pub libfdk_aac: bool,
    pub flac: bool,
    pub libopus: bool,
    pub opus: bool,
    pub libmp3lame: bool,
    pub libshine: bool,
    pub h264_nvidia: bool,
    pub h265_nvidia: bool,
    pub av1_nvidia: bool,
    pub h264_amd: bool,
    pub h265_amd: bool,
    pub av1_amd: bool,
    pub h264_intel_qsv: bool,
    pub h265_intel_qsv: bool,
    pub av1_intel_qsv: bool,
}

impl EncoderAvailability {
    /// Returns whether the requested codec has any usable encoder.
    pub fn supports(&self, codec: CodecChoice) -> bool {
        match codec {
            CodecChoice::H264 => self.h264,
            CodecChoice::H265 => self.h265,
            CodecChoice::Av1 => self.av1,
        }
    }

    /// Returns the safest fallback codec for this machine.
    pub fn fallback_codec(&self) -> CodecChoice {
        if self.h264 {
            CodecChoice::H264
        } else if self.h265 {
            CodecChoice::H265
        } else {
            CodecChoice::Av1
        }
    }

    /// Returns the codec preferred for size-first compression.
    pub fn reduce_size_codec(&self) -> CodecChoice {
        if self.h265 {
            CodecChoice::H265
        } else {
            self.fallback_codec()
        }
    }

    /// Returns the codec preferred for quality-first compression.
    pub fn quality_codec(&self) -> CodecChoice {
        if self.h264 {
            CodecChoice::H264
        } else {
            self.fallback_codec()
        }
    }

    /// Resolves the best backend for the requested codec.
    pub fn resolved_encoder(&self, codec: CodecChoice) -> ResolvedEncoder {
        let backend = match codec {
            CodecChoice::H264 if self.h264_nvidia => EncoderBackend::Nvidia,
            CodecChoice::H265 if self.h265_nvidia => EncoderBackend::Nvidia,
            CodecChoice::Av1 if self.av1_nvidia => EncoderBackend::Nvidia,
            CodecChoice::H264 if self.h264_amd => EncoderBackend::Amd,
            CodecChoice::H265 if self.h265_amd => EncoderBackend::Amd,
            CodecChoice::Av1 if self.av1_amd => EncoderBackend::Amd,
            CodecChoice::H264 if self.h264_intel_qsv => EncoderBackend::IntelQuickSync,
            CodecChoice::H265 if self.h265_intel_qsv => EncoderBackend::IntelQuickSync,
            CodecChoice::Av1 if self.av1_intel_qsv => EncoderBackend::IntelQuickSync,
            _ => EncoderBackend::Software,
        };

        ResolvedEncoder { codec, backend }
    }

    /// Returns encoder availability filtered by the global hardware acceleration preference.
    pub fn with_hardware_acceleration(&self, enabled: bool) -> Self {
        if enabled {
            return self.clone();
        }

        Self {
            h264: self.h264,
            h265: self.h265,
            av1: self.av1,
            aac: self.aac,
            libfdk_aac: self.libfdk_aac,
            flac: self.flac,
            libopus: self.libopus,
            opus: self.opus,
            libmp3lame: self.libmp3lame,
            libshine: self.libshine,
            ..Default::default()
        }
    }

    /// Returns whether an AAC encoder is available.
    pub fn supports_aac(&self) -> bool {
        self.aac || self.libfdk_aac
    }

    /// Returns whether a FLAC encoder is available.
    pub fn supports_flac(&self) -> bool {
        self.flac
    }

    /// Returns whether an Opus encoder is available.
    pub fn supports_opus(&self) -> bool {
        self.libopus || self.opus
    }

    /// Returns whether an MP3 encoder is available.
    pub fn supports_mp3(&self) -> bool {
        self.libmp3lame || self.libshine
    }

    /// Returns the preferred AAC encoder identifier for FFmpeg.
    pub fn preferred_aac_encoder_name(&self) -> Option<&'static str> {
        if self.aac {
            Some("aac")
        } else if self.libfdk_aac {
            Some("libfdk_aac")
        } else {
            None
        }
    }

    /// Returns the preferred Opus encoder identifier for FFmpeg.
    pub fn preferred_opus_encoder_name(&self) -> Option<&'static str> {
        if self.libopus {
            Some("libopus")
        } else if self.opus {
            Some("opus")
        } else {
            None
        }
    }

    /// Returns the preferred MP3 encoder identifier for FFmpeg.
    pub fn preferred_mp3_encoder_name(&self) -> Option<&'static str> {
        if self.libmp3lame {
            Some("libmp3lame")
        } else if self.libshine {
            Some("libshine")
        } else {
            None
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
    /// Returns the source label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::ManagedUpdate => "Managed Update",
            Self::Bundled => "Bundled",
            Self::SystemPath => "System PATH",
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{EncoderAvailability, EncoderBackend};
    use crate::modules::compress_videos::models::CodecChoice;

    #[test]
    fn prefers_nvidia_when_available() {
        let encoders = EncoderAvailability {
            h264: true,
            h264_nvidia: true,
            h264_amd: true,
            h264_intel_qsv: true,
            ..Default::default()
        };

        let resolved = encoders.resolved_encoder(CodecChoice::H264);

        assert_eq!(resolved.backend, EncoderBackend::Nvidia);
    }

    #[test]
    fn falls_back_to_software_when_gpu_backend_is_missing() {
        let encoders = EncoderAvailability {
            h265: true,
            ..Default::default()
        };

        let resolved = encoders.resolved_encoder(CodecChoice::H265);

        assert_eq!(resolved.backend, EncoderBackend::Software);
    }

    #[test]
    fn keeps_amd_priority_over_intel_quick_sync() {
        let encoders = EncoderAvailability {
            h265: true,
            h265_amd: true,
            h265_intel_qsv: true,
            ..Default::default()
        };

        let resolved = encoders.resolved_encoder(CodecChoice::H265);

        assert_eq!(resolved.backend, EncoderBackend::Amd);
    }

    #[test]
    fn uses_intel_quick_sync_when_it_is_the_only_gpu_backend() {
        let encoders = EncoderAvailability {
            av1: true,
            av1_intel_qsv: true,
            ..Default::default()
        };

        let resolved = encoders.resolved_encoder(CodecChoice::Av1);

        assert_eq!(resolved.backend, EncoderBackend::IntelQuickSync);
    }

    #[test]
    fn strips_gpu_backends_when_hardware_acceleration_is_disabled() {
        let encoders = EncoderAvailability {
            h264: true,
            h264_nvidia: true,
            h264_amd: true,
            h264_intel_qsv: true,
            ..Default::default()
        };

        let resolved = encoders
            .with_hardware_acceleration(false)
            .resolved_encoder(CodecChoice::H264);

        assert_eq!(resolved.backend, EncoderBackend::Software);
    }
}
