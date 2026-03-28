mod analysis;
mod batch;
mod files;
mod parsing;
mod process;

use std::{ffi::OsStr, path::Path};

pub use self::{
    analysis::{analyze_audio, estimate_output},
    batch::{AudioBatchEvent, AudioBatchHandle, AudioBatchItem, start_audio_batch},
    process::probe_audio,
};

const AUDIO_EXTENSIONS: [&str; 14] = [
    "aac", "aif", "aiff", "flac", "m4a", "m4b", "mka", "mp2", "mp3", "oga", "ogg", "opus", "wav",
    "wma",
];

/// Returns whether the given path looks like a supported audio file.
pub fn is_supported_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .map(|ext| AUDIO_EXTENSIONS.iter().any(|known| *known == ext))
        .unwrap_or(false)
}
