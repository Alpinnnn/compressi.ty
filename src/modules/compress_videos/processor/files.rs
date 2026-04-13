use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    modules::compress_videos::models::{EngineInfo, VideoMetadata},
    runtime,
};

use super::execution::{background_command, format_time_arg};

const PREVIEW_MAX_WIDTH: u32 = 480;
const PREVIEW_MAX_HEIGHT: u32 = 270;
const PREVIEW_MIN_FRAME_RATE: f32 = 8.0;
const PREVIEW_MAX_FRAME_RATE: f32 = 24.0;

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
        .join("compressi.ty")
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

/// Preview decode settings chosen to keep inline playback responsive.
#[derive(Clone, Copy, Debug)]
pub struct PreviewStreamConfig {
    /// Output width for decoded RGBA frames.
    pub width: u32,
    /// Output height for decoded RGBA frames.
    pub height: u32,
    /// Playback frame rate used by the inline preview player.
    pub frame_rate: f32,
}

/// Resolves the scaled dimensions and playback frame rate used by the inline preview player.
pub fn preview_stream_config(video: &VideoMetadata) -> PreviewStreamConfig {
    let width = video.width.max(2);
    let height = video.height.max(2);
    let width_ratio = PREVIEW_MAX_WIDTH as f32 / width as f32;
    let height_ratio = PREVIEW_MAX_HEIGHT as f32 / height as f32;
    let scale = width_ratio.min(height_ratio).min(1.0);

    let scaled_width =
        make_even((width as f32 * scale).round() as u32).clamp(2, PREVIEW_MAX_WIDTH.max(2));
    let scaled_height =
        make_even((height as f32 * scale).round() as u32).clamp(2, PREVIEW_MAX_HEIGHT.max(2));
    let frame_rate = if video.fps.is_finite() && video.fps > 0.0 {
        video
            .fps
            .clamp(PREVIEW_MIN_FRAME_RATE, PREVIEW_MAX_FRAME_RATE)
    } else {
        12.0
    };

    PreviewStreamConfig {
        width: scaled_width,
        height: scaled_height,
        frame_rate,
    }
}

/// Builds the FFmpeg command used by the inline preview player.
pub fn build_preview_stream_command(
    engine: &EngineInfo,
    video_path: &Path,
    start_secs: f32,
    config: PreviewStreamConfig,
    single_frame: bool,
) -> Command {
    let mut command = background_command(&engine.ffmpeg_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-nostdin")
        .arg("-ss")
        .arg(format_time_arg(start_secs.max(0.0)))
        .arg("-i")
        .arg(video_path)
        .arg("-an")
        .arg("-sn")
        .arg("-dn")
        .arg("-vf")
        .arg(preview_filter(config, single_frame))
        .arg("-pix_fmt")
        .arg("rgba");

    if single_frame {
        command.arg("-frames:v").arg("1");
    }

    command.arg("-f").arg("rawvideo").arg("pipe:1");
    command
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

fn preview_filter(config: PreviewStreamConfig, single_frame: bool) -> String {
    let base_scale = format!(
        "scale={}:{}:flags=lanczos,format=rgba",
        config.width, config.height
    );
    if single_frame {
        base_scale
    } else {
        format!("fps={:.3},{}", config.frame_rate, base_scale)
    }
}

fn make_even(value: u32) -> u32 {
    if value <= 2 {
        2
    } else if value % 2 == 0 {
        value
    } else {
        value - 1
    }
}
