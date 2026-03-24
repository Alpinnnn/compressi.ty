use std::{collections::BTreeMap, ffi::OsStr, path::PathBuf};

use crate::modules::compress_videos::models::VideoMetadata;

pub(super) fn parse_ffprobe_output(path: PathBuf, output: &str) -> Result<VideoMetadata, String> {
    let mut format_values = BTreeMap::<String, String>::new();
    let mut streams = BTreeMap::<usize, BTreeMap<String, String>>::new();

    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };

        let key = raw_key.trim();
        let value = raw_value.trim().trim_matches('"').to_owned();

        if let Some(rest) = key.strip_prefix("streams_stream_") {
            let Some((index, field)) = rest.split_once('_') else {
                continue;
            };
            let Ok(index) = index.parse::<usize>() else {
                continue;
            };
            streams
                .entry(index)
                .or_default()
                .insert(field.to_owned(), value);
        } else if let Some(field) = key.strip_prefix("format_") {
            format_values.insert(field.to_owned(), value);
        }
    }

    let video_stream = streams
        .values()
        .find(|stream| stream.get("codec_type").map(String::as_str) == Some("video"))
        .ok_or_else(|| "The selected file does not contain a video stream.".to_owned())?;
    let audio_stream = streams
        .values()
        .find(|stream| stream.get("codec_type").map(String::as_str) == Some("audio"));

    let width = parse_u32(video_stream.get("width")).unwrap_or(0);
    let height = parse_u32(video_stream.get("height")).unwrap_or(0);
    let duration_secs = parse_f32(format_values.get("duration")).unwrap_or(0.0);
    let fps = parse_ratio(
        video_stream
            .get("avg_frame_rate")
            .or_else(|| video_stream.get("r_frame_rate")),
    )
    .unwrap_or(30.0)
    .max(1.0);

    if width == 0 || height == 0 || duration_secs <= 0.0 {
        return Err("The selected file could not be analyzed correctly.".to_owned());
    }

    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("video")
        .to_owned();

    Ok(VideoMetadata {
        path,
        file_name,
        size_bytes: parse_u64(format_values.get("size")).unwrap_or(0),
        duration_secs,
        width,
        height,
        fps,
        container_bitrate_kbps: parse_u32(format_values.get("bit_rate")).map(|value| value / 1000),
        video_bitrate_kbps: parse_u32(video_stream.get("bit_rate")).map(|value| value / 1000),
        audio_bitrate_kbps: audio_stream
            .and_then(|stream| parse_u32(stream.get("bit_rate")))
            .map(|value| value / 1000),
        video_codec: video_stream
            .get("codec_name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_owned()),
        has_audio: audio_stream.is_some(),
    })
}

fn parse_u32(value: Option<&String>) -> Option<u32> {
    value.and_then(|value| value.parse::<u32>().ok())
}

fn parse_u64(value: Option<&String>) -> Option<u64> {
    value.and_then(|value| value.parse::<u64>().ok())
}

fn parse_f32(value: Option<&String>) -> Option<f32> {
    value.and_then(|value| value.parse::<f32>().ok())
}

fn parse_ratio(value: Option<&String>) -> Option<f32> {
    let value = value?;
    if let Some((left, right)) = value.split_once('/') {
        let left = left.parse::<f32>().ok()?;
        let right = right.parse::<f32>().ok()?;
        if right == 0.0 {
            None
        } else {
            Some(left / right)
        }
    } else {
        value.parse::<f32>().ok()
    }
}

pub(super) fn parse_time_to_secs(value: &str) -> Option<f32> {
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<f32>().ok()?;
    let minutes = parts.next()?.parse::<f32>().ok()?;
    let seconds = parts.next()?.parse::<f32>().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

#[derive(Default)]
pub(super) struct ProgressParser {
    out_time_secs: f32,
    speed_x: f32,
}

impl ProgressParser {
    pub(super) fn push_line(&mut self, line: &str) -> Option<ProgressSnapshot> {
        let (key, value) = line.split_once('=')?;
        match key {
            "out_time_us" => {
                self.out_time_secs = value.parse::<f32>().ok()? / 1_000_000.0;
                None
            }
            "out_time_ms" => {
                let raw = value.parse::<f32>().ok()?;
                self.out_time_secs = if raw > 500_000.0 {
                    raw / 1_000_000.0
                } else {
                    raw / 1000.0
                };
                None
            }
            "out_time" => {
                self.out_time_secs = parse_time_to_secs(value)?;
                None
            }
            "speed" => {
                self.speed_x = value.trim_end_matches('x').parse::<f32>().unwrap_or(0.0);
                None
            }
            "progress" => Some(ProgressSnapshot {
                out_time_secs: self.out_time_secs,
                speed_x: self.speed_x.max(0.0),
            }),
            _ => None,
        }
    }
}

pub(super) struct ProgressSnapshot {
    pub(super) out_time_secs: f32,
    pub(super) speed_x: f32,
}
