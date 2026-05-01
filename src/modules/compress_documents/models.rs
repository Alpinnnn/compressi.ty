use std::path::{Path, PathBuf};

const MICROSOFT_OPEN_XML_EXTENSIONS: &[&str] = &[
    "docx", "docm", "dotx", "dotm", "xlsx", "xlsm", "xltx", "xltm", "xlam", "pptx", "pptm", "potx",
    "potm", "ppsx", "ppsm", "ppam", "sldx", "sldm",
];
const OPEN_DOCUMENT_EXTENSIONS: &[&str] = &[
    "odt", "ott", "oth", "odm", "ods", "ots", "odp", "otp", "odg", "otg", "odf", "odc", "odi",
    "odb",
];
const OPEN_PACKAGING_EXTENSIONS: &[&str] = &[
    "xps", "oxps", "vsdx", "vsdm", "vsstx", "vsstm", "vssx", "vssm", "vstx", "vstm",
];
const EPUB_EXTENSIONS: &[&str] = &["epub"];
const SUPPORTED_DOCUMENT_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "docm", "dotx", "dotm", "xlsx", "xlsm", "xltx", "xltm", "xlam", "pptx", "pptm",
    "potx", "potm", "ppsx", "ppsm", "ppam", "sldx", "sldm", "odt", "ott", "oth", "odm", "ods",
    "ots", "odp", "otp", "odg", "otg", "odf", "odc", "odi", "odb", "xps", "oxps", "vsdx", "vsdm",
    "vsstx", "vsstm", "vssx", "vssm", "vstx", "vstm", "epub",
];

/// Supported document container family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Pdf,
    MicrosoftOpenXml,
    OpenDocument,
    OpenPackaging,
    Epub,
}

impl DocumentKind {
    /// Resolves the document family from a file path extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = lowercase_extension(path)?;
        if extension == "pdf" {
            return Some(Self::Pdf);
        }
        if MICROSOFT_OPEN_XML_EXTENSIONS.contains(&extension.as_str()) {
            return Some(Self::MicrosoftOpenXml);
        }
        if OPEN_DOCUMENT_EXTENSIONS.contains(&extension.as_str()) {
            return Some(Self::OpenDocument);
        }
        if OPEN_PACKAGING_EXTENSIONS.contains(&extension.as_str()) {
            return Some(Self::OpenPackaging);
        }
        if EPUB_EXTENSIONS.contains(&extension.as_str()) {
            return Some(Self::Epub);
        }
        None
    }

    /// Short label rendered in queue rows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::MicrosoftOpenXml => "Office",
            Self::OpenDocument => "ODF",
            Self::OpenPackaging => "OPC",
            Self::Epub => "EPUB",
        }
    }

    /// User-facing description for the compression engine used by this family.
    pub fn engine_label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF stream optimizer",
            Self::MicrosoftOpenXml => "Office ZIP package",
            Self::OpenDocument => "OpenDocument ZIP package",
            Self::OpenPackaging => "Open Packaging package",
            Self::Epub => "EPUB ZIP package",
        }
    }

    /// Returns true when the document family is a ZIP-based package.
    pub fn is_zip_package(self) -> bool {
        !matches!(self, Self::Pdf)
    }
}

/// Returns every extension accepted by the document compression workspace.
pub fn supported_document_extensions() -> &'static [&'static str] {
    SUPPORTED_DOCUMENT_EXTENSIONS
}

/// Returns true when a path can be queued in the document workspace.
pub fn is_supported_document_path(path: &Path) -> bool {
    DocumentKind::from_path(path).is_some()
}

/// Preset bundles exposed in the document compression workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentCompressionPreset {
    MaximumCompatibility,
    Balanced,
    HighCompression,
    UltraCompression,
}

impl DocumentCompressionPreset {
    pub const ALL: [Self; 4] = [
        Self::MaximumCompatibility,
        Self::Balanced,
        Self::HighCompression,
        Self::UltraCompression,
    ];

    /// Returns the user-facing preset name.
    pub fn title(self) -> &'static str {
        match self {
            Self::MaximumCompatibility => "Maximum Compatibility",
            Self::Balanced => "Balanced",
            Self::HighCompression => "High Compression",
            Self::UltraCompression => "Ultra Compression",
        }
    }

    /// Returns the supporting copy shown beside the preset.
    pub fn description(self) -> &'static str {
        match self {
            Self::MaximumCompatibility => "Lossless structural cleanup for older readers.",
            Self::Balanced => "Everyday image-aware compression with readable output.",
            Self::HighCompression => "Stronger PDF downsampling and media recompression.",
            Self::UltraCompression => "Aggressive image reduction for the smallest files.",
        }
    }

    /// Returns the concrete defaults represented by this preset.
    pub fn defaults(self) -> DocumentPresetDefaults {
        match self {
            Self::MaximumCompatibility => DocumentPresetDefaults {
                compression_level: 4,
                pdf_object_streams: false,
                pdf_image_quality: 100,
                pdf_image_resolution_dpi: 300,
                package_image_quality: 100,
                package_image_resize_percent: 100,
            },
            Self::Balanced => DocumentPresetDefaults {
                compression_level: 7,
                pdf_object_streams: true,
                pdf_image_quality: 82,
                pdf_image_resolution_dpi: 160,
                package_image_quality: 82,
                package_image_resize_percent: 100,
            },
            Self::HighCompression => DocumentPresetDefaults {
                compression_level: 8,
                pdf_object_streams: true,
                pdf_image_quality: 68,
                pdf_image_resolution_dpi: 120,
                package_image_quality: 68,
                package_image_resize_percent: 88,
            },
            Self::UltraCompression => DocumentPresetDefaults {
                compression_level: 9,
                pdf_object_streams: true,
                pdf_image_quality: 52,
                pdf_image_resolution_dpi: 96,
                package_image_quality: 52,
                package_image_resize_percent: 72,
            },
        }
    }
}

/// Concrete setting values associated with a document preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentPresetDefaults {
    pub compression_level: u8,
    pub pdf_object_streams: bool,
    pub pdf_image_quality: u8,
    pub pdf_image_resolution_dpi: u16,
    pub package_image_quality: u8,
    pub package_image_resize_percent: u8,
}

/// Editable settings for the current document batch.
#[derive(Clone, Debug)]
pub struct DocumentCompressionSettings {
    pub preset: DocumentCompressionPreset,
    pub advanced_mode: bool,
    pub compression_level: u8,
    pub pdf_object_streams: bool,
    pub pdf_image_quality: u8,
    pub pdf_image_resolution_dpi: u16,
    pub package_image_quality: u8,
    pub package_image_resize_percent: u8,
}

impl Default for DocumentCompressionSettings {
    fn default() -> Self {
        let preset = DocumentCompressionPreset::Balanced;
        let defaults = preset.defaults();
        Self {
            preset,
            advanced_mode: false,
            compression_level: defaults.compression_level,
            pdf_object_streams: defaults.pdf_object_streams,
            pdf_image_quality: defaults.pdf_image_quality,
            pdf_image_resolution_dpi: defaults.pdf_image_resolution_dpi,
            package_image_quality: defaults.package_image_quality,
            package_image_resize_percent: defaults.package_image_resize_percent,
        }
    }
}

impl DocumentCompressionSettings {
    /// Replaces the active settings with the defaults for the selected preset.
    pub fn apply_preset(&mut self, preset: DocumentCompressionPreset) {
        let defaults = preset.defaults();
        self.preset = preset;
        self.compression_level = defaults.compression_level;
        self.pdf_object_streams = defaults.pdf_object_streams;
        self.pdf_image_quality = defaults.pdf_image_quality;
        self.pdf_image_resolution_dpi = defaults.pdf_image_resolution_dpi;
        self.package_image_quality = defaults.package_image_quality;
        self.package_image_resize_percent = defaults.package_image_resize_percent;
    }

    /// Returns a PDF-compatible zlib compression level.
    pub fn pdf_compression_level(&self) -> u8 {
        self.compression_level.clamp(0, 9)
    }

    /// Returns a ZIP-compatible deflate compression level.
    pub fn zip_compression_level(&self) -> i64 {
        i64::from(self.compression_level.clamp(0, 9))
    }

    /// Returns a bounded PDF image quality for external PDF engines.
    pub fn pdf_image_quality(&self) -> u8 {
        self.pdf_image_quality.clamp(35, 100)
    }

    /// Returns a bounded target DPI for external PDF image downsampling.
    pub fn pdf_image_resolution_dpi(&self) -> u16 {
        self.pdf_image_resolution_dpi.clamp(72, 300)
    }

    /// Returns true when PDF compression may use lossy image optimization.
    pub fn pdf_image_optimization_enabled(&self) -> bool {
        self.pdf_image_quality() < 100 || self.pdf_image_resolution_dpi() < 300
    }

    /// Returns a bounded JPEG quality for media embedded in ZIP packages.
    pub fn package_image_quality(&self) -> u8 {
        self.package_image_quality.clamp(35, 100)
    }

    /// Returns a bounded resize percentage for media embedded in ZIP packages.
    pub fn package_image_resize_percent(&self) -> u8 {
        self.package_image_resize_percent.clamp(40, 100)
    }

    /// Returns true when package media should be decoded and recompressed.
    pub fn package_image_optimization_enabled(&self) -> bool {
        self.package_image_quality() < 100 || self.package_image_resize_percent() < 100
    }
}

/// Metadata about a queued source document.
#[derive(Clone, Debug)]
pub struct DocumentAsset {
    pub id: u64,
    pub path: PathBuf,
    pub file_name: String,
    pub original_size: u64,
    pub kind: DocumentKind,
}

/// A document item ready to be inserted into the queue.
#[derive(Clone, Debug)]
pub struct LoadedDocument {
    pub asset: DocumentAsset,
}

/// Progress information for a document that is currently being compressed.
#[derive(Clone, Debug)]
pub struct DocumentProgress {
    pub progress: f32,
    pub stage: String,
}

/// Completed output information for a compressed document.
#[derive(Clone, Debug)]
pub struct DocumentCompressionResult {
    pub output_path: PathBuf,
    pub original_size: u64,
    pub compressed_size: u64,
    pub reduction_percent: f32,
}

/// End-user visible state for a queue item in the document workflow.
#[derive(Clone, Debug)]
pub enum DocumentCompressionState {
    Ready,
    Compressing(DocumentProgress),
    Completed(DocumentCompressionResult),
    Failed(String),
    Cancelled,
}

/// Full queue row state for a document.
#[derive(Clone, Debug)]
pub struct DocumentQueueItem {
    pub asset: DocumentAsset,
    pub state: DocumentCompressionState,
}

fn lowercase_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{
        DocumentCompressionPreset, DocumentCompressionSettings, DocumentKind,
        is_supported_document_path,
    };
    use std::path::Path;

    #[test]
    fn detects_supported_document_families() {
        assert_eq!(
            DocumentKind::from_path(Path::new("report.PDF")),
            Some(DocumentKind::Pdf)
        );
        assert_eq!(
            DocumentKind::from_path(Path::new("deck.pptx")),
            Some(DocumentKind::MicrosoftOpenXml)
        );
        assert_eq!(
            DocumentKind::from_path(Path::new("sheet.ods")),
            Some(DocumentKind::OpenDocument)
        );
        assert_eq!(
            DocumentKind::from_path(Path::new("manual.epub")),
            Some(DocumentKind::Epub)
        );
        assert!(is_supported_document_path(Path::new("diagram.vsdx")));
        assert!(!is_supported_document_path(Path::new("legacy.doc")));
    }

    #[test]
    fn preset_updates_compression_controls() {
        let mut settings = DocumentCompressionSettings::default();
        settings.apply_preset(DocumentCompressionPreset::UltraCompression);

        assert_eq!(settings.compression_level, 9);
        assert!(settings.pdf_object_streams);
        assert_eq!(settings.pdf_compression_level(), 9);
        assert_eq!(settings.zip_compression_level(), 9);
        assert_eq!(settings.pdf_image_quality(), 52);
        assert_eq!(settings.pdf_image_resolution_dpi(), 96);
        assert!(settings.pdf_image_optimization_enabled());
        assert_eq!(settings.package_image_quality(), 52);
        assert_eq!(settings.package_image_resize_percent(), 72);
        assert!(settings.package_image_optimization_enabled());
    }
}
