use std::{collections::BTreeMap, ffi::OsStr, path::PathBuf};

use crate::modules::compress_audio::models::AudioMetadata;

pub(super) fn parse_ffprobe_output(path: PathBuf, output: &str) -> Result<AudioMetadata, String> {
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

    let audio_stream = streams
        .values()
        .find(|stream| stream.get("codec_type").map(String::as_str) == Some("audio"))
        .ok_or_else(|| "The selected file does not contain an audio stream.".to_owned())?;

    let duration_secs = parse_f32(format_values.get("duration")).unwrap_or(0.0);
    if duration_secs <= 0.0 {
        return Err("The selected file could not be analyzed correctly.".to_owned());
    }

    let codec_name = audio_stream
        .get("codec_name")
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let container_name = format_values
        .get("format_name")
        .and_then(|value| value.split(',').next())
        .unwrap_or("audio")
        .to_owned();

    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("audio")
        .to_owned();

    Ok(AudioMetadata {
        path,
        file_name,
        size_bytes: parse_u64(format_values.get("size")).unwrap_or(0),
        duration_secs,
        audio_bitrate_kbps: parse_u32(audio_stream.get("bit_rate"))
            .or_else(|| parse_u32(format_values.get("bit_rate")))
            .map(|value| value / 1000),
        sample_rate_hz: parse_u32(audio_stream.get("sample_rate")).unwrap_or(44_100),
        channels: parse_u32(audio_stream.get("channels"))
            .unwrap_or(2)
            .clamp(1, 8) as u8,
        codec_name: codec_name.clone(),
        container_name,
        is_lossless: codec_looks_lossless(&codec_name),
    })
}

fn codec_looks_lossless(codec_name: &str) -> bool {
    matches!(
        codec_name,
        "alac"
            | "ape"
            | "flac"
            | "pcm_alaw"
            | "pcm_f32be"
            | "pcm_f32le"
            | "pcm_f64be"
            | "pcm_f64le"
            | "pcm_mulaw"
            | "pcm_s16be"
            | "pcm_s16le"
            | "pcm_s24be"
            | "pcm_s24le"
            | "pcm_s32be"
            | "pcm_s32le"
            | "wavpack"
    )
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

fn parse_time_to_secs(value: &str) -> Option<f32> {
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<f32>().ok()?;
    let minutes = parts.next()?.parse::<f32>().ok()?;
    let seconds = parts.next()?.parse::<f32>().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::parse_ffprobe_output;
    use std::path::PathBuf;

    #[test]
    fn parses_audio_probe_output() {
        let parsed = parse_ffprobe_output(
            PathBuf::from("song.flac"),
            r#"
format_duration="65.500000"
format_size="10485760"
format_bit_rate="1280000"
format_format_name="flac"
streams_stream_0_codec_type="audio"
streams_stream_0_codec_name="flac"
streams_stream_0_channels=2
streams_stream_0_sample_rate="44100"
streams_stream_0_bit_rate="960000"
"#,
        )
        .unwrap();

        assert_eq!(parsed.codec_name, "flac");
        assert_eq!(parsed.channels, 2);
        assert!(parsed.is_lossless);
    }
}
