pub mod compress_audio;
pub mod compress_documents;
pub mod compress_photos;
pub mod compress_videos;

use eframe::egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    CompressAudio,
    CompressDocuments,
    CompressPhotos,
    CompressVideos,
    ArchiveExtract,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconKind {
    Audio,
    Documents,
    Photo,
    Videos,
    Archive,
    Settings,
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleSpec {
    pub icon: IconKind,
    pub title: &'static str,
    pub detail: &'static str,
    pub accent: Color32,
}

impl ModuleKind {
    pub fn spec(self) -> ModuleSpec {
        match self {
            Self::CompressAudio => ModuleSpec {
                icon: IconKind::Audio,
                title: "Compress Audio",
                detail: "Compress music, podcasts, and voice notes with smart defaults, quick batch workflows, and advanced controls when needed.",
                accent: Color32::from_rgb(208, 208, 204),
            },
            Self::CompressDocuments => ModuleSpec {
                icon: IconKind::Documents,
                title: "Compress Documents",
                detail: "Compress PDFs, Office files, OpenDocument packages, EPUBs, XPS, and Visio documents in a queue-based workflow.",
                accent: Color32::from_rgb(196, 196, 192),
            },
            Self::CompressPhotos => ModuleSpec {
                icon: IconKind::Photo,
                title: "Compress Photos",
                detail: "Reduce photo size with presets, advanced controls, background batch jobs, and modern output formats.",
                accent: Color32::from_rgb(228, 228, 224),
            },
            Self::CompressVideos => ModuleSpec {
                icon: IconKind::Videos,
                title: "Compress Videos",
                detail: "Prepare high-resolution footage for sharing, archiving, or faster local playback with codec-aware presets.",
                accent: Color32::from_rgb(184, 184, 180),
            },
            Self::ArchiveExtract => ModuleSpec {
                icon: IconKind::Archive,
                title: "Archive / Extract",
                detail: "Handle compressed packages, archive formats, and extraction tasks from a single module built for speed.",
                accent: Color32::from_rgb(168, 168, 164),
            },
            Self::Settings => ModuleSpec {
                icon: IconKind::Settings,
                title: "Settings",
                detail: "Manage the app experience, automation preferences, and output destinations from a clean control hub.",
                accent: Color32::from_rgb(152, 152, 148),
            },
        }
    }
}

impl IconKind {
    pub fn glyph(self) -> char {
        match self {
            Self::Audio => crate::icons::PLAY,
            Self::Documents => crate::icons::DOCUMENT,
            Self::Photo => crate::icons::IMAGES,
            Self::Videos => crate::icons::VIDEO,
            Self::Archive => crate::icons::ARCHIVE,
            Self::Settings => crate::icons::SETTINGS,
        }
    }
}
