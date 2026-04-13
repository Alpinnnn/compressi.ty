use crate::modules::{
    compress_audio::models::{
        AudioAutoPreset, AudioCompressionSettings, AudioContentKind, AudioFormat, AudioMetadata,
        AudioWorkflowMode,
    },
    compress_videos::models::EncoderAvailability,
};

use super::source_bitrate_kbps;

pub(super) fn choose_auto_format(
    content_kind: AudioContentKind,
    preset: AudioAutoPreset,
    encoders: &EncoderAvailability,
) -> AudioFormat {
    let preferred_formats = match (content_kind, preset) {
        (AudioContentKind::Voice, _) | (_, AudioAutoPreset::SmallSize) => [
            AudioFormat::Opus,
            AudioFormat::Aac,
            AudioFormat::Mp3,
            AudioFormat::Flac,
        ],
        (_, _) => [
            AudioFormat::Aac,
            AudioFormat::Opus,
            AudioFormat::Mp3,
            AudioFormat::Flac,
        ],
    };

    preferred_formats
        .into_iter()
        .find(|format| format_supported(*format, encoders))
        .unwrap_or(AudioFormat::Flac)
}

pub(super) fn resolve_encoder(
    requested_format: AudioFormat,
    content_kind: AudioContentKind,
    encoders: &EncoderAvailability,
) -> (AudioFormat, &'static str) {
    if let Some(encoder_name) = encoder_name_for_format(requested_format, encoders) {
        return (requested_format, encoder_name);
    }

    let fallback = choose_auto_format(content_kind, AudioAutoPreset::Balanced, encoders);
    if let Some(encoder_name) = encoder_name_for_format(fallback, encoders) {
        return (fallback, encoder_name);
    }

    // The bundled runtime should always expose at least one audio encoder. If the current build
    // is unusually limited, fall back to the native AAC encoder name so the final FFmpeg error is
    // explicit instead of silently hiding the problem.
    (AudioFormat::Aac, "aac")
}

pub(super) fn resolve_target_bitrate(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    output_format: AudioFormat,
    content_kind: AudioContentKind,
    encoder_name: &str,
) -> Option<u32> {
    if output_format.is_lossless() {
        return None;
    }

    let bitrate = match settings.mode {
        AudioWorkflowMode::Auto => {
            auto_target_bitrate(output_format, content_kind, settings.auto_preset)
        }
        AudioWorkflowMode::Manual => Some(settings.manual_bitrate_kbps.clamp(24, 320)),
    }?;

    let source_bitrate = source_bitrate_kbps(metadata);
    let format_floor = high_quality_floor(output_format, content_kind);
    let adjusted = if settings.convert_format_only {
        source_bitrate.map(|source| bitrate.max(source).max(format_floor))
    } else {
        Some(bitrate)
    };

    adjusted.map(|value| {
        if encoder_name == "libshine" {
            round_to_nearest(value, 16).clamp(32, 320)
        } else {
            value
        }
    })
}

pub(super) fn resolve_aac_vbr_mode(
    settings: &AudioCompressionSettings,
    output_format: AudioFormat,
    encoder_name: &str,
    target_bitrate_kbps: Option<u32>,
    channels: u8,
) -> Option<u8> {
    if settings.mode != AudioWorkflowMode::Auto
        || settings.convert_format_only
        || output_format != AudioFormat::Aac
        || encoder_name != "libfdk_aac"
    {
        return None;
    }

    let per_channel_kbps = target_bitrate_kbps? / channels.max(1) as u32;
    let candidates = [(1_u8, 32_u32), (2, 40), (3, 52), (4, 64), (5, 88)];

    candidates
        .into_iter()
        .min_by_key(|(_, approx_kbps)| per_channel_kbps.abs_diff(*approx_kbps))
        .map(|(mode, _)| mode)
}

pub(super) fn estimate_size_bytes(
    metadata: &AudioMetadata,
    output_format: AudioFormat,
    target_bitrate_kbps: Option<u32>,
    convert_format_only: bool,
) -> u64 {
    if output_format.is_lossless() {
        if metadata.is_lossless {
            return ((metadata.size_bytes as f32) * 0.58).round() as u64;
        }
        return ((metadata.size_bytes as f32) * 1.18).round() as u64;
    }

    let duration_secs = metadata.duration_secs.max(1.0);
    let bitrate_kbps =
        target_bitrate_kbps.unwrap_or_else(|| source_bitrate_kbps(metadata).unwrap_or(128));
    let container_overhead = match output_format {
        AudioFormat::Aac => 1.04,
        AudioFormat::Opus => 1.02,
        AudioFormat::Mp3 => 1.01,
        AudioFormat::Flac => 1.0,
    };
    let estimated = ((duration_secs * bitrate_kbps as f32 * 1000.0 / 8.0) * container_overhead)
        .max(8_192.0)
        .round() as u64;

    if convert_format_only && !output_format.is_lossless() {
        estimated.max(metadata.size_bytes.saturating_mul(98) / 100)
    } else {
        estimated
    }
}

pub(super) fn preferred_audio_aac_encoder_name(
    encoders: &EncoderAvailability,
) -> Option<&'static str> {
    if encoders.libfdk_aac {
        Some("libfdk_aac")
    } else if encoders.aac {
        Some("aac")
    } else {
        None
    }
}

fn auto_target_bitrate(
    output_format: AudioFormat,
    content_kind: AudioContentKind,
    preset: AudioAutoPreset,
) -> Option<u32> {
    let bitrate = match (output_format, content_kind, preset) {
        (AudioFormat::Flac, _, _) => return None,
        (AudioFormat::Opus, AudioContentKind::Voice, AudioAutoPreset::HighQuality) => 48,
        (AudioFormat::Opus, AudioContentKind::Voice, AudioAutoPreset::Balanced) => 32,
        (AudioFormat::Opus, AudioContentKind::Voice, AudioAutoPreset::SmallSize) => 24,
        (AudioFormat::Opus, AudioContentKind::Music, AudioAutoPreset::HighQuality) => 128,
        (AudioFormat::Opus, AudioContentKind::Music, AudioAutoPreset::Balanced) => 96,
        (AudioFormat::Opus, AudioContentKind::Music, AudioAutoPreset::SmallSize) => 72,
        (AudioFormat::Opus, AudioContentKind::Mixed, AudioAutoPreset::HighQuality) => 112,
        (AudioFormat::Opus, AudioContentKind::Mixed, AudioAutoPreset::Balanced) => 80,
        (AudioFormat::Opus, AudioContentKind::Mixed, AudioAutoPreset::SmallSize) => 64,
        (AudioFormat::Aac, AudioContentKind::Voice, AudioAutoPreset::HighQuality) => 72,
        (AudioFormat::Aac, AudioContentKind::Voice, AudioAutoPreset::Balanced) => 64,
        (AudioFormat::Aac, AudioContentKind::Voice, AudioAutoPreset::SmallSize) => 48,
        (AudioFormat::Aac, AudioContentKind::Music, AudioAutoPreset::HighQuality) => 160,
        (AudioFormat::Aac, AudioContentKind::Music, AudioAutoPreset::Balanced) => 128,
        (AudioFormat::Aac, AudioContentKind::Music, AudioAutoPreset::SmallSize) => 96,
        (AudioFormat::Aac, AudioContentKind::Mixed, AudioAutoPreset::HighQuality) => 144,
        (AudioFormat::Aac, AudioContentKind::Mixed, AudioAutoPreset::Balanced) => 112,
        (AudioFormat::Aac, AudioContentKind::Mixed, AudioAutoPreset::SmallSize) => 80,
        (AudioFormat::Mp3, AudioContentKind::Voice, AudioAutoPreset::HighQuality) => 96,
        (AudioFormat::Mp3, AudioContentKind::Voice, AudioAutoPreset::Balanced) => 64,
        (AudioFormat::Mp3, AudioContentKind::Voice, AudioAutoPreset::SmallSize) => 48,
        (AudioFormat::Mp3, AudioContentKind::Music, AudioAutoPreset::HighQuality) => 192,
        (AudioFormat::Mp3, AudioContentKind::Music, AudioAutoPreset::Balanced) => 128,
        (AudioFormat::Mp3, AudioContentKind::Music, AudioAutoPreset::SmallSize) => 96,
        (AudioFormat::Mp3, AudioContentKind::Mixed, AudioAutoPreset::HighQuality) => 160,
        (AudioFormat::Mp3, AudioContentKind::Mixed, AudioAutoPreset::Balanced) => 112,
        (AudioFormat::Mp3, AudioContentKind::Mixed, AudioAutoPreset::SmallSize) => 80,
    };

    Some(bitrate)
}

fn high_quality_floor(output_format: AudioFormat, content_kind: AudioContentKind) -> u32 {
    match (output_format, content_kind) {
        (AudioFormat::Opus, AudioContentKind::Voice) => 48,
        (AudioFormat::Opus, _) => 112,
        (AudioFormat::Aac, AudioContentKind::Voice) => 72,
        (AudioFormat::Aac, _) => 144,
        (AudioFormat::Mp3, AudioContentKind::Voice) => 96,
        (AudioFormat::Mp3, _) => 160,
        (AudioFormat::Flac, _) => 0,
    }
}

fn format_supported(format: AudioFormat, encoders: &EncoderAvailability) -> bool {
    encoder_name_for_format(format, encoders).is_some()
}

fn encoder_name_for_format(
    format: AudioFormat,
    encoders: &EncoderAvailability,
) -> Option<&'static str> {
    match format {
        AudioFormat::Mp3 => encoders.preferred_mp3_encoder_name(),
        AudioFormat::Aac => preferred_audio_aac_encoder_name(encoders),
        AudioFormat::Opus => encoders.preferred_opus_encoder_name(),
        AudioFormat::Flac => encoders.supports_flac().then_some("flac"),
    }
}

fn round_to_nearest(value: u32, step: u32) -> u32 {
    if step == 0 {
        return value;
    }

    let lower = value / step * step;
    let upper = lower + step;
    if value - lower < upper.saturating_sub(value) {
        lower.max(step)
    } else {
        upper
    }
}
