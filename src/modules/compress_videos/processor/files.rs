use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{modules::compress_videos::models::EngineInfo, runtime};

use super::execution::background_command;

pub(super) fn resolve_output_dir(base_output_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match base_output_dir {
        Some(path) => Ok(path),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("Clock error: {error}"))?
                .as_secs();
            let root = runtime::default_video_output_root();

            Ok(root.join(format!("run-{timestamp}")))
        }
    }
}

pub(super) fn build_output_name(source: &Path, suffix: &str, extension: &str) -> String {
    format!("{}-{suffix}.{extension}", safe_stem(source, "video"))
}

pub(super) fn build_unique_output_path(
    output_dir: &Path,
    source: &Path,
    suffix: &str,
    extension: &str,
) -> PathBuf {
    let base_name = build_output_name(source, suffix, extension);
    let candidate = output_dir.join(&base_name);
    if !candidate.exists() {
        return candidate;
    }

    let safe_stem = safe_stem(source, "video");
    for counter in 1..=999 {
        let name = format!("{safe_stem}-{suffix}-{counter}.{extension}");
        let path = output_dir.join(&name);
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

/// Extracts a single thumbnail frame from a video using FFmpeg.
/// Returns the raw RGBA bytes and dimensions (width, height) on success.
pub fn generate_thumbnail(
    engine: &EngineInfo,
    video_path: &Path,
    duration_secs: f32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let thumb_dir = std::env::temp_dir()
        .join("compressity")
        .join("video-thumbs");
    fs::create_dir_all(&thumb_dir)
        .map_err(|error| format!("Could not create thumbnail folder: {error}"))?;

    let stem = video_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("thumb");
    let thumb_path = thumb_dir.join(format!("{stem}-thumb.png"));
    let seek_secs = (duration_secs * 0.1).min(duration_secs).max(0.0);

    let mut command = background_command(&engine.ffmpeg_path);
    command
        .arg("-y")
        .arg("-ss")
        .arg(format!("{seek_secs:.2}"))
        .arg("-i")
        .arg(video_path)
        .arg("-vframes")
        .arg("1")
        .arg("-vf")
        .arg("scale=120:-1")
        .arg(&thumb_path);

    let output = command
        .output()
        .map_err(|error| format!("Could not run FFmpeg for thumbnail: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .last()
            .unwrap_or("Thumbnail extraction failed.");
        return Err(detail.to_owned());
    }

    let image =
        image::open(&thumb_path).map_err(|error| format!("Could not decode thumbnail: {error}"))?;
    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let bytes = rgba.into_raw();

    let _ = fs::remove_file(&thumb_path);
    Ok((bytes, width, height))
}

pub(super) fn cleanup_passlog(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("log.mbtree"));
    let _ = fs::remove_file(path.with_extension("log.temp"));
}

pub(super) fn null_output_path() -> &'static Path {
    if cfg!(windows) {
        Path::new("NUL")
    } else {
        Path::new("/dev/null")
    }
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
