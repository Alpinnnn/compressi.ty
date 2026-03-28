use crate::modules::{
    compress_audio::models::AudioFormat, compress_videos::models::EncoderAvailability,
};

pub(super) fn format_available(format: AudioFormat, encoders: &EncoderAvailability) -> bool {
    match format {
        AudioFormat::Aac => encoders.supports_aac(),
        AudioFormat::Opus => encoders.supports_opus(),
        AudioFormat::Mp3 => encoders.supports_mp3(),
        AudioFormat::Flac => encoders.supports_flac(),
    }
}

pub(super) fn format_audio_channels(channels: u8) -> &'static str {
    match channels {
        0 => "Unknown",
        1 => "Mono",
        2 => "Stereo",
        _ => "Multi-channel",
    }
}

pub(super) fn format_audio_sample_rate(sample_rate_hz: u32) -> String {
    if sample_rate_hz >= 1000 {
        format!("{:.1} kHz", sample_rate_hz as f32 / 1000.0)
    } else {
        format!("{sample_rate_hz} Hz")
    }
}

pub(super) fn output_summary(
    output_format: AudioFormat,
    target_bitrate_kbps: Option<u32>,
) -> String {
    target_bitrate_kbps
        .map(|bitrate| format!("{} | {} kbps", output_format.label(), bitrate))
        .unwrap_or_else(|| output_format.label().to_owned())
}

pub(super) fn sample_rate_choices() -> [(Option<u32>, &'static str); 5] {
    [
        (None, "Original"),
        (Some(22_050), "22.05 kHz"),
        (Some(32_000), "32 kHz"),
        (Some(44_100), "44.1 kHz"),
        (Some(48_000), "48 kHz"),
    ]
}

pub(super) fn channel_choices() -> [(Option<u8>, &'static str); 3] {
    [(None, "Original"), (Some(1), "Mono"), (Some(2), "Stereo")]
}
