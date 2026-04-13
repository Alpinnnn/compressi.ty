use std::{
    mem,
    path::{Path, PathBuf},
};

use crate::modules::{
    ModuleKind, compress_audio, compress_photos::models::PhotoFormat, compress_videos::processor,
};

const IPC_MAGIC: &str = "COMPRESSITY_LAUNCH_V1";

/// Supported files passed to the app during startup.
#[derive(Debug, Default)]
pub struct LaunchImport {
    preferred_module: Option<ModuleKind>,
    audio_paths: Vec<PathBuf>,
    photo_paths: Vec<PathBuf>,
    video_paths: Vec<PathBuf>,
}

impl LaunchImport {
    /// Reads command-line file paths and groups them by the workspace that can handle them.
    pub fn collect_from_command_line() -> Self {
        Self::collect_from_paths(std::env::args_os().skip(1).map(PathBuf::from))
    }

    /// Groups a set of paths by the workspace that can handle them.
    pub fn collect_from_paths<I>(paths: I) -> Self
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut launch_import = Self::default();

        for path in paths {
            if !path.is_file() {
                continue;
            }

            let Some(module) = supported_module_for_path(&path) else {
                continue;
            };

            if launch_import.preferred_module.is_none() {
                launch_import.preferred_module = Some(module);
            }

            match module {
                ModuleKind::CompressAudio => launch_import.audio_paths.push(path),
                ModuleKind::CompressPhotos => launch_import.photo_paths.push(path),
                ModuleKind::CompressVideos => launch_import.video_paths.push(path),
                _ => {}
            }
        }

        launch_import
    }

    /// Merges a new launch request into the pending import queue.
    pub fn merge(&mut self, mut other: Self) {
        if let Some(module) = other.preferred_module {
            self.preferred_module = Some(module);
        }
        self.audio_paths.append(&mut other.audio_paths);
        self.photo_paths.append(&mut other.photo_paths);
        self.video_paths.append(&mut other.video_paths);
    }

    /// Returns the workspace that best matches the first supported startup file.
    pub fn preferred_module(&self) -> Option<ModuleKind> {
        self.preferred_module
    }

    /// Returns true when there are startup audio files waiting to be imported.
    pub fn has_audio_paths(&self) -> bool {
        !self.audio_paths.is_empty()
    }

    /// Returns true when there are startup photo files waiting to be imported.
    pub fn has_photo_paths(&self) -> bool {
        !self.photo_paths.is_empty()
    }

    /// Returns true when there are startup video files waiting to be imported.
    pub fn has_video_paths(&self) -> bool {
        !self.video_paths.is_empty()
    }

    /// Drains all pending startup audio paths.
    pub fn take_audio_paths(&mut self) -> Vec<PathBuf> {
        mem::take(&mut self.audio_paths)
    }

    /// Drains all pending startup photo paths.
    pub fn take_photo_paths(&mut self) -> Vec<PathBuf> {
        mem::take(&mut self.photo_paths)
    }

    /// Drains all pending startup video paths.
    pub fn take_video_paths(&mut self) -> Vec<PathBuf> {
        mem::take(&mut self.video_paths)
    }

    #[cfg(target_os = "windows")]
    /// Serializes the launch request for inter-process handoff.
    pub fn to_ipc_payload(&self) -> String {
        let mut payload = String::from(IPC_MAGIC);

        if let Some(module_name) = self.preferred_module.and_then(module_to_ipc_name) {
            payload.push('\n');
            payload.push_str("M\t");
            payload.push_str(module_name);
        }

        for path in &self.audio_paths {
            payload.push('\n');
            payload.push_str("A\t");
            payload.push_str(&path.to_string_lossy());
        }

        for path in &self.photo_paths {
            payload.push('\n');
            payload.push_str("P\t");
            payload.push_str(&path.to_string_lossy());
        }

        for path in &self.video_paths {
            payload.push('\n');
            payload.push_str("V\t");
            payload.push_str(&path.to_string_lossy());
        }

        payload
    }

    /// Parses a launch request received from another app instance.
    pub fn from_ipc_payload(payload: &str) -> Option<Self> {
        let mut lines = payload.lines();
        if lines.next()? != IPC_MAGIC {
            return None;
        }

        let mut launch_import = Self::default();
        for line in lines {
            let Some((kind, value)) = line.split_once('\t') else {
                continue;
            };

            match kind {
                "M" => {
                    if launch_import.preferred_module.is_none() {
                        launch_import.preferred_module = module_from_ipc_name(value);
                    }
                }
                "A" => launch_import.audio_paths.push(PathBuf::from(value)),
                "P" => launch_import.photo_paths.push(PathBuf::from(value)),
                "V" => launch_import.video_paths.push(PathBuf::from(value)),
                _ => {}
            }
        }

        if launch_import.preferred_module.is_none() {
            launch_import.preferred_module = if !launch_import.audio_paths.is_empty() {
                Some(ModuleKind::CompressAudio)
            } else if !launch_import.photo_paths.is_empty() {
                Some(ModuleKind::CompressPhotos)
            } else if !launch_import.video_paths.is_empty() {
                Some(ModuleKind::CompressVideos)
            } else {
                None
            };
        }

        Some(launch_import)
    }
}

fn supported_module_for_path(path: &Path) -> Option<ModuleKind> {
    if compress_audio::is_supported_audio_path(path) {
        return Some(ModuleKind::CompressAudio);
    }

    if PhotoFormat::from_path(path).is_some() {
        return Some(ModuleKind::CompressPhotos);
    }

    if processor::is_supported_video_path(path) {
        return Some(ModuleKind::CompressVideos);
    }

    None
}

#[cfg(target_os = "windows")]
fn module_to_ipc_name(module: ModuleKind) -> Option<&'static str> {
    match module {
        ModuleKind::CompressAudio => Some("audio"),
        ModuleKind::CompressPhotos => Some("photos"),
        ModuleKind::CompressVideos => Some("videos"),
        _ => None,
    }
}

fn module_from_ipc_name(value: &str) -> Option<ModuleKind> {
    match value {
        "audio" => Some(ModuleKind::CompressAudio),
        "photos" => Some(ModuleKind::CompressPhotos),
        "videos" => Some(ModuleKind::CompressVideos),
        _ => None,
    }
}
