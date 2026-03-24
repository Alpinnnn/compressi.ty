use std::{
    mem,
    path::{Path, PathBuf},
};

use crate::modules::{
    ModuleKind, compress_photos::models::PhotoFormat, compress_videos::processor,
};

/// Supported files passed to the app during startup.
#[derive(Debug, Default)]
pub struct LaunchImport {
    preferred_module: Option<ModuleKind>,
    photo_paths: Vec<PathBuf>,
    video_paths: Vec<PathBuf>,
}

impl LaunchImport {
    /// Reads command-line file paths and groups them by the workspace that can handle them.
    pub fn collect_from_command_line() -> Self {
        let mut launch_import = Self::default();

        for path in std::env::args_os().skip(1).map(PathBuf::from) {
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
                ModuleKind::CompressPhotos => launch_import.photo_paths.push(path),
                ModuleKind::CompressVideos => launch_import.video_paths.push(path),
                _ => {}
            }
        }

        launch_import
    }

    /// Returns the workspace that best matches the first supported startup file.
    pub fn preferred_module(&self) -> Option<ModuleKind> {
        self.preferred_module
    }

    /// Returns true when there are startup photo files waiting to be imported.
    pub fn has_photo_paths(&self) -> bool {
        !self.photo_paths.is_empty()
    }

    /// Returns true when there are startup video files waiting to be imported.
    pub fn has_video_paths(&self) -> bool {
        !self.video_paths.is_empty()
    }

    /// Drains all pending startup photo paths.
    pub fn take_photo_paths(&mut self) -> Vec<PathBuf> {
        mem::take(&mut self.photo_paths)
    }

    /// Drains all pending startup video paths.
    pub fn take_video_paths(&mut self) -> Vec<PathBuf> {
        mem::take(&mut self.video_paths)
    }
}

fn supported_module_for_path(path: &Path) -> Option<ModuleKind> {
    if PhotoFormat::from_path(path).is_some() {
        return Some(ModuleKind::CompressPhotos);
    }

    if processor::is_supported_video_path(path) {
        return Some(ModuleKind::CompressVideos);
    }

    None
}
