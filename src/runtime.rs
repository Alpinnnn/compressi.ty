use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "compressity";
pub const OUTPUT_DIR_NAME: &str = "compressity-output";

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(APP_DIR_NAME))
}

pub fn data_dir() -> Option<PathBuf> {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .map(|dir| dir.join(APP_DIR_NAME))
}

pub fn managed_engine_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("engine").join(platform_dirname()))
}

pub fn bundled_engine_dir() -> Option<PathBuf> {
    current_exe_dir()
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

pub fn ffmpeg_binary_name() -> &'static str {
    if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }
}

pub fn ffprobe_binary_name() -> &'static str {
    if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" }
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
