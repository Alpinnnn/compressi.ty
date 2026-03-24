use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    modules::compress_photos::models::{PhotoAsset, PhotoFormat},
    runtime,
};

pub(super) fn build_output_path(
    output_dir: &Path,
    asset: &PhotoAsset,
    output_format: PhotoFormat,
) -> PathBuf {
    let stem = asset
        .path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let safe_stem = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    output_dir.join(format!(
        "{}-compressed-{}.{}",
        safe_stem,
        asset.id,
        output_format.extension()
    ))
}

pub(super) fn create_output_dir() -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Clock error: {error}"))?
        .as_secs();
    let base = runtime::default_photo_output_root();

    Ok(base.join(format!("run-{timestamp}")))
}
