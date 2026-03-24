use std::path::PathBuf;

/// Preset bundles exposed in the photo compression workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionPreset {
    MaximumQuality,
    Balanced,
    HighCompression,
    UltraCompression,
}

impl CompressionPreset {
    pub const ALL: [Self; 4] = [
        Self::MaximumQuality,
        Self::Balanced,
        Self::HighCompression,
        Self::UltraCompression,
    ];

    /// Returns the user-facing title for this preset.
    pub fn title(self) -> &'static str {
        match self {
            Self::MaximumQuality => "Maximum Quality",
            Self::Balanced => "Balanced",
            Self::HighCompression => "High Compression",
            Self::UltraCompression => "Ultra Compression",
        }
    }

    /// Returns the supporting copy shown below the preset title.
    pub fn description(self) -> &'static str {
        match self {
            Self::MaximumQuality => "Best fidelity with subtle size savings.",
            Self::Balanced => "A clean everyday setting for sharing and storage.",
            Self::HighCompression => "Smaller exports with modern lossy encoding.",
            Self::UltraCompression => "Aggressive reduction for lightweight delivery.",
        }
    }

    /// Returns the default settings bundle associated with this preset.
    pub fn defaults(self) -> PresetDefaults {
        match self {
            Self::MaximumQuality => PresetDefaults {
                quality: 92,
                resize_percent: 100,
                strip_metadata: false,
                format_choice: ConvertFormat::Original,
            },
            Self::Balanced => PresetDefaults {
                quality: 82,
                resize_percent: 100,
                strip_metadata: true,
                format_choice: ConvertFormat::Original,
            },
            Self::HighCompression => PresetDefaults {
                quality: 64,
                resize_percent: 88,
                strip_metadata: true,
                format_choice: ConvertFormat::Original,
            },
            Self::UltraCompression => PresetDefaults {
                quality: 42,
                resize_percent: 72,
                strip_metadata: true,
                format_choice: ConvertFormat::Original,
            },
        }
    }
}

/// Concrete setting values associated with a compression preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetDefaults {
    pub quality: u8,
    pub resize_percent: u8,
    pub strip_metadata: bool,
    pub format_choice: ConvertFormat,
}

/// Output format choices available in the photo workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvertFormat {
    Original,
    Jpeg,
    WebP,
    Avif,
}

impl ConvertFormat {
    pub const ALL: [Self; 4] = [Self::Original, Self::Jpeg, Self::WebP, Self::Avif];

    /// Returns the label used in the output format selector.
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "Original",
            Self::Jpeg => "JPEG",
            Self::WebP => "WebP",
            Self::Avif => "AVIF",
        }
    }
}

/// Source image formats currently accepted by the loader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhotoFormat {
    Png,
    Jpeg,
    WebP,
    Avif,
}

impl PhotoFormat {
    /// Infers the image format from a file path extension.
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();

        match ext.as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::WebP),
            "avif" => Some(Self::Avif),
            _ => None,
        }
    }

    /// Returns the short label shown in the queue and result UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::WebP => "WebP",
            Self::Avif => "AVIF",
        }
    }

    /// Returns the canonical file extension for this format.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
            Self::Avif => "avif",
        }
    }
}

/// Editable compression settings for the current photo batch.
#[derive(Clone, Debug)]
pub struct CompressionSettings {
    pub preset: CompressionPreset,
    pub advanced_mode: bool,
    pub quality: u8,
    pub resize_percent: u8,
    pub strip_metadata: bool,
    pub format_choice: ConvertFormat,
}

impl Default for CompressionSettings {
    fn default() -> Self {
        let preset = CompressionPreset::Balanced;
        let defaults = preset.defaults();

        Self {
            preset,
            advanced_mode: false,
            quality: defaults.quality,
            resize_percent: defaults.resize_percent,
            strip_metadata: defaults.strip_metadata,
            format_choice: defaults.format_choice,
        }
    }
}

impl CompressionSettings {
    /// Replaces the active settings with the defaults for the selected preset.
    pub fn apply_preset(&mut self, preset: CompressionPreset) {
        let defaults = preset.defaults();
        self.preset = preset;
        self.quality = defaults.quality;
        self.resize_percent = defaults.resize_percent;
        self.strip_metadata = defaults.strip_metadata;
        self.format_choice = defaults.format_choice;
    }
}

/// Metadata about a queued source image.
#[derive(Clone, Debug)]
pub struct PhotoAsset {
    pub id: u64,
    pub path: PathBuf,
    pub file_name: String,
    pub original_size: u64,
    pub format: PhotoFormat,
    pub dimensions: (u32, u32),
}

/// Raw RGBA preview pixels for a photo thumbnail or preview image.
#[derive(Clone, Debug)]
pub struct PhotoPreview {
    pub rgba: Vec<u8>,
    pub size: [usize; 2],
}

/// Fully loaded photo data ready to be inserted into the queue.
#[derive(Clone, Debug)]
pub struct LoadedPhoto {
    pub asset: PhotoAsset,
    pub preview: Option<PhotoPreview>,
}

/// Progress information for a file that is currently being compressed.
#[derive(Clone, Debug)]
pub struct FileProgress {
    pub progress: f32,
    pub stage: String,
}

/// Completed output information for a compressed photo.
#[derive(Clone, Debug)]
pub struct CompressionResult {
    pub output_path: PathBuf,
    pub output_format: PhotoFormat,
    pub original_size: u64,
    pub compressed_size: u64,
    pub reduction_percent: f32,
}

/// End-user visible state for a queue item in the photo workflow.
#[derive(Clone, Debug)]
pub enum CompressionState {
    Ready,
    Compressing(FileProgress),
    Completed(CompressionResult),
    Failed(String),
    Cancelled,
}
