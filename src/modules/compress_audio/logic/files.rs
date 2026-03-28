use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::runtime;

pub(super) fn resolve_output_dir(base_output_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match base_output_dir {
        Some(path) => Ok(path),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("Clock error: {error}"))?
                .as_secs();
            Ok(runtime::default_audio_output_root().join(format!("run-{timestamp}")))
        }
    }
}

pub(super) fn build_unique_output_path(
    output_dir: &Path,
    source: &Path,
    suffix: &str,
    extension: &str,
) -> PathBuf {
    let candidate = output_dir.join(build_output_name(source, suffix, extension));
    if !candidate.exists() {
        return candidate;
    }

    let safe_stem = safe_stem(source, "audio");
    for counter in 1..=999 {
        let path = output_dir.join(format!("{safe_stem}-{suffix}-{counter}.{extension}"));
        if !path.exists() {
            return path;
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    output_dir.join(format!("{safe_stem}-{suffix}-{timestamp}.{extension}"))
}

fn build_output_name(source: &Path, suffix: &str, extension: &str) -> String {
    format!("{}-{suffix}.{extension}", safe_stem(source, "audio"))
}

fn safe_stem(source: &Path, fallback: &str) -> String {
    source
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or(fallback)
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}
