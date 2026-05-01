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
            Self::MaximumCompatibility => "Gentle lossless optimization for older readers.",
            Self::Balanced => "Everyday smaller files with modern-safe defaults.",
            Self::HighCompression => "Stronger ZIP and PDF stream compression.",
            Self::UltraCompression => "Slowest setting for the smallest Rust-native output.",
        }
    }

    /// Returns the concrete defaults represented by this preset.
    pub fn defaults(self) -> DocumentPresetDefaults {
        match self {
            Self::MaximumCompatibility => DocumentPresetDefaults {
                compression_level: 4,
                pdf_object_streams: false,
            },
            Self::Balanced => DocumentPresetDefaults {
                compression_level: 6,
                pdf_object_streams: true,
            },
            Self::HighCompression => DocumentPresetDefaults {
                compression_level: 8,
                pdf_object_streams: true,
            },
            Self::UltraCompression => DocumentPresetDefaults {
                compression_level: 9,
                pdf_object_streams: true,
            },
        }
    }
}

/// Concrete setting values associated with a document preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentPresetDefaults {
    pub compression_level: u8,
    pub pdf_object_streams: bool,
}

/// Editable settings for the current document batch.
#[derive(Clone, Debug)]
pub struct DocumentCompressionSettings {
    pub preset: DocumentCompressionPreset,
    pub advanced_mode: bool,
    pub compression_level: u8,
    pub pdf_object_streams: bool,
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
    }

    /// Returns a PDF-compatible zlib compression level.
    pub fn pdf_compression_level(&self) -> u8 {
        self.compression_level.clamp(0, 9)
    }

    /// Returns a ZIP-compatible deflate compression level.
    pub fn zip_compression_level(&self) -> i64 {
        i64::from(self.compression_level.clamp(0, 9))
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
    }
}
