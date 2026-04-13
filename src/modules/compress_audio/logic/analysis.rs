use crate::modules::{
    compress_audio::models::{
        AudioAnalysis, AudioAutoPreset, AudioCompressionPlan, AudioCompressionSettings,
        AudioContentKind, AudioEstimate, AudioFormat, AudioMetadata, AudioWorkflowMode,
    },
    compress_videos::models::EncoderAvailability,
};

#[path = "analysis/profiles.rs"]
mod profiles;

use self::profiles::{
    choose_auto_format, estimate_size_bytes, resolve_aac_vbr_mode, resolve_encoder,
    resolve_target_bitrate,
};

/// Builds the lightweight smart analysis summary shown above the settings.
pub fn analyze_audio(metadata: &AudioMetadata, encoders: &EncoderAvailability) -> AudioAnalysis {
    let content_kind = detect_content_kind(metadata);
    let auto_format = choose_auto_format(content_kind, AudioAutoPreset::Balanced, encoders);

    let detail = match content_kind {
        AudioContentKind::Voice => match auto_format {
            AudioFormat::Opus => {
                "Detected a voice-focused recording, so Smart Mode will prefer OPUS for smaller files with clear speech."
            }
            AudioFormat::Aac => {
                "Detected a voice-focused recording, so Smart Mode will keep speech clear with an AAC fallback."
            }
            AudioFormat::Mp3 => {
                "Detected a voice-focused recording. MP3 fallback is available for compatibility, but file size may not shrink as much."
            }
            AudioFormat::Flac => {
                "Detected a voice-focused recording, but only lossless output is available from the current FFmpeg build."
            }
        },
        AudioContentKind::Music => match auto_format {
            AudioFormat::Aac => {
                "Detected a music-heavy file, so Smart Mode will favor AAC for a strong quality-to-size balance."
            }
            AudioFormat::Opus => {
                "Detected a music-heavy file. Smart Mode can still use OPUS when it offers better savings on this device."
            }
            AudioFormat::Mp3 => {
                "Detected a music-heavy file. MP3 is available as the safest compatibility fallback."
            }
            AudioFormat::Flac => {
                "Detected a music-heavy file, but the current FFmpeg build only exposes lossless output."
            }
        },
        AudioContentKind::Mixed => match auto_format {
            AudioFormat::Aac => {
                "Detected a mixed file, so Smart Mode will stay conservative with AAC unless a smaller preset is chosen."
            }
            AudioFormat::Opus => {
                "Detected a mixed file, so Smart Mode will use OPUS when low-bitrate efficiency matters most."
            }
            AudioFormat::Mp3 => {
                "Detected a mixed file. MP3 fallback keeps output widely compatible across older devices."
            }
            AudioFormat::Flac => {
                "Detected a mixed file, but only lossless output is currently available from the FFmpeg runtime."
            }
        },
    };

    AudioAnalysis {
        content_kind,
        headline: format!("Detected {}", content_kind.label()),
        detail: detail.to_owned(),
    }
}

/// Computes the output estimate shown in the settings panel.
pub fn estimate_output(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    encoders: &EncoderAvailability,
) -> AudioEstimate {
    let plan = build_plan(metadata, settings, encoders);
    let savings_percent = if metadata.size_bytes == 0 {
        0.0
    } else {
        100.0 - (plan.estimated_size_bytes as f32 / metadata.size_bytes as f32 * 100.0)
    };

    AudioEstimate {
        original_size_bytes: metadata.size_bytes,
        estimated_size_bytes: plan.estimated_size_bytes,
        savings_percent,
        output_format: plan.output_format,
        target_bitrate_kbps: plan.target_bitrate_kbps,
        effective_sample_rate_hz: plan.sample_rate_hz,
        effective_channels: plan.channels,
        warnings: plan.warnings.clone(),
        recommendation: plan.recommendation.clone(),
        should_skip: plan.should_skip,
        skip_reason: plan.skip_reason.clone(),
    }
}

/// Resolves the format, bitrate, and safety warnings for the current file and settings.
pub fn build_plan(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    encoders: &EncoderAvailability,
) -> AudioCompressionPlan {
    let content_kind = detect_content_kind(metadata);
    let mut warnings = Vec::new();
    let mut recommendation = None;

    let requested_format = match settings.mode {
        AudioWorkflowMode::Auto => choose_auto_format(content_kind, settings.auto_preset, encoders),
        AudioWorkflowMode::Manual => settings.manual_format,
    };

    let (output_format, encoder_name) = resolve_encoder(requested_format, content_kind, encoders);
    if output_format != requested_format {
        warnings.push(format!(
            "{} is not available in the current FFmpeg build. Using {} instead.",
            requested_format.label(),
            output_format.label()
        ));
    }

    let target_bitrate_kbps = resolve_target_bitrate(
        metadata,
        settings,
        output_format,
        content_kind,
        encoder_name,
    );
    let sample_rate_hz = resolve_sample_rate(metadata, settings, content_kind, output_format);
    let channels = resolve_channels(metadata, settings, content_kind, output_format);
    let aac_vbr_mode = resolve_aac_vbr_mode(
        settings,
        output_format,
        encoder_name,
        target_bitrate_kbps,
        channels.unwrap_or(metadata.channels),
    );
    let mp3_use_abr = output_format == AudioFormat::Mp3 && encoder_name == "libmp3lame";

    if metadata.is_lossy() && !output_format.is_lossless() && !settings.convert_format_only {
        warnings.push(
            "This will recompress a lossy source. The file can get smaller, but some detail may be lost."
                .to_owned(),
        );
    }

    if metadata.size_bytes <= 256 * 1024 || metadata.duration_secs <= 10.0 {
        warnings
            .push("This file is already small, so compression savings may be minimal.".to_owned());
    }

    if settings.convert_format_only {
        warnings.push(
            "Convert format only keeps quality first, so the output can stay close to the original size or grow slightly."
                .to_owned(),
        );
    }

    if let Some(target_bitrate_kbps) = target_bitrate_kbps {
        let effective_channels = channels.unwrap_or(metadata.channels).max(1) as u32;
        let per_channel_bitrate = target_bitrate_kbps / effective_channels.max(1);
        if is_bitrate_too_aggressive(content_kind, per_channel_bitrate) {
            recommendation = Some(match content_kind {
                AudioContentKind::Voice => {
                    "Speech may sound thin with this target. Try Balanced or High Quality for a safer result."
                        .to_owned()
                }
                AudioContentKind::Music | AudioContentKind::Mixed => {
                    "This bitrate is aggressive for music. Try a higher bitrate or the High Quality preset for cleaner output."
                        .to_owned()
                }
            });
        }
    }

    let estimated_size_bytes = estimate_size_bytes(
        metadata,
        output_format,
        target_bitrate_kbps,
        settings.convert_format_only,
    );

    let mut should_skip = false;
    let mut skip_reason = None;
    if !settings.convert_format_only && !output_format.is_lossless() {
        let would_not_help = estimated_size_bytes >= metadata.size_bytes.saturating_mul(96) / 100;
        let source_bitrate = source_bitrate_kbps(metadata);
        let target_close_to_source = target_bitrate_kbps
            .zip(source_bitrate)
            .map(|(target, source)| target >= source.saturating_sub(8))
            .unwrap_or(false);

        if would_not_help || target_close_to_source {
            should_skip = true;
            skip_reason = Some(
                "The current settings are unlikely to shrink this file in a meaningful way."
                    .to_owned(),
            );
            warnings.push(
                "The file is already compact for the chosen mode. Consider Small Size or a different format if you need stronger savings."
                    .to_owned(),
            );
        }
    }

    AudioCompressionPlan {
        output_format,
        encoder_name,
        target_bitrate_kbps,
        aac_vbr_mode,
        mp3_use_abr,
        sample_rate_hz,
        channels,
        content_kind,
        warnings,
        recommendation,
        estimated_size_bytes,
        should_skip,
        skip_reason,
    }
}

pub(super) fn detect_content_kind(metadata: &AudioMetadata) -> AudioContentKind {
    let bitrate_kbps = source_bitrate_kbps(metadata).unwrap_or(128);
    if metadata.channels <= 1 && (metadata.sample_rate_hz <= 32_000 || bitrate_kbps <= 96) {
        AudioContentKind::Voice
    } else if metadata.channels >= 2 && metadata.sample_rate_hz >= 44_100 && bitrate_kbps >= 96 {
        AudioContentKind::Music
    } else {
        AudioContentKind::Mixed
    }
}

pub(super) fn source_bitrate_kbps(metadata: &AudioMetadata) -> Option<u32> {
    metadata.audio_bitrate_kbps.or_else(|| {
        if metadata.duration_secs > 0.0 {
            Some(
                ((metadata.size_bytes as f32 * 8.0) / metadata.duration_secs / 1000.0).round()
                    as u32,
            )
        } else {
            None
        }
    })
}

fn resolve_sample_rate(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    content_kind: AudioContentKind,
    output_format: AudioFormat,
) -> Option<u32> {
    match settings.mode {
        AudioWorkflowMode::Manual => settings.manual_sample_rate_hz,
        AudioWorkflowMode::Auto if output_format == AudioFormat::Opus => None,
        AudioWorkflowMode::Auto => match (content_kind, settings.auto_preset) {
            (AudioContentKind::Voice, AudioAutoPreset::SmallSize) => Some(24_000),
            (AudioContentKind::Voice, AudioAutoPreset::Balanced) => Some(32_000),
            (_, AudioAutoPreset::SmallSize) if metadata.sample_rate_hz > 44_100 => Some(44_100),
            _ => None,
        },
    }
}

fn resolve_channels(
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    content_kind: AudioContentKind,
    output_format: AudioFormat,
) -> Option<u8> {
    match settings.mode {
        AudioWorkflowMode::Manual => settings.manual_channels,
        AudioWorkflowMode::Auto if output_format == AudioFormat::Opus => None,
        AudioWorkflowMode::Auto => match content_kind {
            AudioContentKind::Voice if metadata.channels > 1 => Some(1),
            _ => None,
        },
    }
}
fn is_bitrate_too_aggressive(content_kind: AudioContentKind, per_channel_bitrate: u32) -> bool {
    match content_kind {
        AudioContentKind::Voice => per_channel_bitrate < 24,
        AudioContentKind::Music => per_channel_bitrate < 48,
        AudioContentKind::Mixed => per_channel_bitrate < 40,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_plan, choose_auto_format, detect_content_kind, estimate_size_bytes, profiles,
    };
    use crate::modules::{
        compress_audio::models::{
            AudioAutoPreset, AudioCompressionSettings, AudioContentKind, AudioFormat,
            AudioMetadata, AudioWorkflowMode,
        },
        compress_videos::models::EncoderAvailability,
    };
    use std::path::PathBuf;

    #[test]
    fn detects_voice_from_mono_low_rate_audio() {
        let metadata = AudioMetadata {
            path: PathBuf::from("voice.wav"),
            file_name: "voice.wav".to_owned(),
            size_bytes: 4_000_000,
            duration_secs: 180.0,
            audio_bitrate_kbps: Some(64),
            sample_rate_hz: 24_000,
            channels: 1,
            codec_name: "pcm_s16le".to_owned(),
            container_name: "wav".to_owned(),
            is_lossless: true,
        };

        assert_eq!(detect_content_kind(&metadata), AudioContentKind::Voice);
    }

    #[test]
    fn prefers_aac_for_music_balanced_when_available() {
        let encoders = EncoderAvailability {
            aac: true,
            libopus: true,
            ..Default::default()
        };

        let format = choose_auto_format(
            AudioContentKind::Music,
            AudioAutoPreset::Balanced,
            &encoders,
        );

        assert_eq!(format, AudioFormat::Aac);
    }

    #[test]
    fn estimates_lossy_outputs_smaller_than_large_pcm_inputs() {
        let metadata = AudioMetadata {
            path: PathBuf::from("track.wav"),
            file_name: "track.wav".to_owned(),
            size_bytes: 40 * 1_048_576,
            duration_secs: 240.0,
            audio_bitrate_kbps: Some(1_411),
            sample_rate_hz: 44_100,
            channels: 2,
            codec_name: "pcm_s16le".to_owned(),
            container_name: "wav".to_owned(),
            is_lossless: true,
        };

        let estimate = estimate_size_bytes(&metadata, AudioFormat::Aac, Some(128), false);

        assert!(estimate < metadata.size_bytes);
    }

    #[test]
    fn prefers_libfdk_for_aac_when_available() {
        let encoders = EncoderAvailability {
            aac: true,
            libfdk_aac: true,
            ..Default::default()
        };

        assert_eq!(
            profiles::preferred_audio_aac_encoder_name(&encoders),
            Some("libfdk_aac")
        );
    }

    #[test]
    fn auto_aac_plan_uses_fdk_vbr_when_available() {
        let metadata = sample_music_metadata();
        let encoders = EncoderAvailability {
            aac: true,
            libfdk_aac: true,
            ..Default::default()
        };

        let plan = build_plan(&metadata, &AudioCompressionSettings::default(), &encoders);

        assert_eq!(plan.output_format, AudioFormat::Aac);
        assert_eq!(plan.encoder_name, "libfdk_aac");
        assert_eq!(plan.aac_vbr_mode, Some(4));
    }

    #[test]
    fn auto_opus_voice_keeps_original_layout_for_encoder_optimization() {
        let metadata = AudioMetadata {
            path: PathBuf::from("voice.wav"),
            file_name: "voice.wav".to_owned(),
            size_bytes: 6_000_000,
            duration_secs: 240.0,
            audio_bitrate_kbps: Some(192),
            sample_rate_hz: 48_000,
            channels: 2,
            codec_name: "pcm_s16le".to_owned(),
            container_name: "wav".to_owned(),
            is_lossless: true,
        };
        let encoders = EncoderAvailability {
            libopus: true,
            ..Default::default()
        };
        let settings = AudioCompressionSettings {
            mode: AudioWorkflowMode::Auto,
            auto_preset: AudioAutoPreset::SmallSize,
            ..Default::default()
        };

        let plan = build_plan(&metadata, &settings, &encoders);

        assert_eq!(plan.output_format, AudioFormat::Opus);
        assert_eq!(plan.sample_rate_hz, None);
        assert_eq!(plan.channels, None);
    }

    fn sample_music_metadata() -> AudioMetadata {
        AudioMetadata {
            path: PathBuf::from("track.wav"),
            file_name: "track.wav".to_owned(),
            size_bytes: 40 * 1_048_576,
            duration_secs: 240.0,
            audio_bitrate_kbps: Some(1_411),
            sample_rate_hz: 44_100,
            channels: 2,
            codec_name: "pcm_s16le".to_owned(),
            container_name: "wav".to_owned(),
            is_lossless: true,
        }
    }
}
