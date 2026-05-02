use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub const APP_DIR_NAME: &str = "compressi.ty";
pub const LEGACY_APP_DIR_NAME: &str = "compressity";
pub const OUTPUT_DIR_NAME: &str = "compressi.ty-output";

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(APP_DIR_NAME))
}

pub fn legacy_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(LEGACY_APP_DIR_NAME))
}

pub fn data_dir() -> Option<PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .map(|dir| dir.join(APP_DIR_NAME))
}

pub fn managed_engine_dir() -> Option<PathBuf> {
    data_dir().map(|dir| {
        dir.join("engine")
            .join("video-engine")
            .join(platform_dirname())
    })
}

/// Returns the per-platform folder for managed PDF compression engines.
pub fn managed_pdf_engine_dir() -> Option<PathBuf> {
    data_dir().map(|dir| {
        dir.join("engine")
            .join("pdf-engine")
            .join(platform_dirname())
    })
}

/// Returns the per-platform folder for managed ZIP-package document engines.
pub fn managed_package_engine_dir() -> Option<PathBuf> {
    data_dir().map(|dir| {
        dir.join("engine")
            .join("package-engine")
            .join(platform_dirname())
    })
}

pub fn bundled_engine_dir() -> Option<PathBuf> {
    current_exe_dir().map(|dir| dir.join("engine").join("video-engine"))
}

/// Returns the folder used for PDF engines shipped beside the app binary.
pub fn bundled_pdf_engine_dir() -> Option<PathBuf> {
    current_exe_dir().map(|dir| dir.join("engine").join("pdf-engine"))
}

/// Returns the folder used for package document engines shipped beside the app binary.
pub fn bundled_package_engine_dir() -> Option<PathBuf> {
    current_exe_dir().map(|dir| dir.join("engine").join("package-engine"))
}

pub fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|dir| dir.to_path_buf())
}

pub fn default_output_root() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::picture_dir)
        .or_else(dirs::document_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join(OUTPUT_DIR_NAME)
}

pub fn default_photo_output_root() -> PathBuf {
    default_output_root().join("photos")
}

pub fn default_video_output_root() -> PathBuf {
    default_output_root().join("videos")
}

/// Returns the default base folder used for generated audio compression outputs.
pub fn default_audio_output_root() -> PathBuf {
    default_output_root().join("audio")
}

/// Returns the default base folder used for generated document compression outputs.
pub fn default_document_output_root() -> PathBuf {
    default_output_root().join("documents")
}

pub fn collect_matching_paths<F>(paths: Vec<PathBuf>, predicate: F) -> Vec<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let mut collected = BTreeSet::new();
    for path in paths {
        collect_matching_paths_from_entry(&path, &predicate, &mut collected);
    }
    collected.into_iter().collect()
}

fn collect_matching_paths_from_entry<F>(
    path: &Path,
    predicate: &F,
    collected: &mut BTreeSet<PathBuf>,
) where
    F: Fn(&Path) -> bool,
{
    if path.is_file() {
        if predicate(path) {
            collected.insert(path.to_path_buf());
        }
        return;
    }

    if !path.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        collect_matching_paths_from_entry(&entry.path(), predicate, collected);
    }
}

pub fn ffmpeg_binary_name() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

pub fn ffprobe_binary_name() -> &'static str {
    if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    }
}

pub fn engine_binaries_exist(dir: &Path) -> bool {
    dir.join(ffmpeg_binary_name()).exists() && dir.join(ffprobe_binary_name()).exists()
}

fn platform_dirname() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unknown"
    }
}
