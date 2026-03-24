use crate::modules::compress_videos::models::{
    CodecChoice, CompressionMode, EncoderAvailability, EncoderBackend, ResolutionChoice,
    ResolvedEncoder, VideoMetadata, VideoSettings,
};

#[derive(Clone)]
pub(in crate::modules::compress_videos::processor) struct EncodePlan {
    pub(in crate::modules::compress_videos::processor) encoder: ResolvedEncoder,
    pub(in crate::modules::compress_videos::processor) video_bitrate_kbps: u32,
    pub(in crate::modules::compress_videos::processor) audio_bitrate_kbps: Option<u32>,
    pub(in crate::modules::compress_videos::processor) crf: Option<u8>,
    /// CQ value for hardware encoder quality-based VBR (NVENC `-cq:v`).
    pub(in crate::modules::compress_videos::processor) hardware_cq: Option<u8>,
    pub(in crate::modules::compress_videos::processor) preset: Option<String>,
    pub(in crate::modules::compress_videos::processor) output_width: u32,
    pub(in crate::modules::compress_videos::processor) output_height: u32,
    pub(in crate::modules::compress_videos::processor) output_fps: f32,
    pub(in crate::modules::compress_videos::processor) pass_count: u8,
}

pub(in crate::modules::compress_videos::processor) fn build_plan(
    video: &VideoMetadata,
    settings: &VideoSettings,
    encoders: &EncoderAvailability,
    preview_mode: bool,
) -> EncodePlan {
    let codec = select_codec(settings, encoders);
    let encoder = encoders.resolved_encoder(codec);
    let resolution_choice = resolve_resolution_choice(video, settings);
    let (output_width, output_height) = resolve_dimensions(video, resolution_choice);
    let output_fps = resolve_fps(video, settings);

    match settings.mode {
        CompressionMode::ReduceSize => reduce_size_plan(
            video,
            settings,
            preview_mode,
            encoder,
            output_width,
            output_height,
            output_fps,
        ),
        CompressionMode::GoodQuality => quality_plan(
            video,
            settings,
            preview_mode,
            codec,
            encoder,
            output_width,
            output_height,
            output_fps,
        ),
        CompressionMode::CustomAdvanced => custom_plan(
            video,
            settings,
            preview_mode,
            encoder,
            output_width,
            output_height,
            output_fps,
        ),
    }
}

fn select_codec(settings: &VideoSettings, encoders: &EncoderAvailability) -> CodecChoice {
    match settings.mode {
        CompressionMode::ReduceSize => encoders.reduce_size_codec(),
        CompressionMode::GoodQuality => encoders.quality_codec(),
        CompressionMode::CustomAdvanced => {
            if encoders.supports(settings.custom_codec) {
                settings.custom_codec
            } else {
                encoders.fallback_codec()
            }
        }
    }
}

fn resolve_resolution_choice(video: &VideoMetadata, settings: &VideoSettings) -> ResolutionChoice {
    match settings.mode {
        CompressionMode::ReduceSize => reduce_size_resolution(video, settings.target_size_mb),
        CompressionMode::GoodQuality | CompressionMode::CustomAdvanced => settings.resolution,
    }
}

fn reduce_size_plan(
    video: &VideoMetadata,
    settings: &VideoSettings,
    preview_mode: bool,
    encoder: ResolvedEncoder,
    output_width: u32,
    output_height: u32,
    output_fps: f32,
) -> EncodePlan {
    let total_kbps = target_total_bitrate(settings.target_size_mb, video.duration_secs);
    let audio_bitrate_kbps = video.has_audio.then_some(aggressive_audio_bitrate(video));
    let video_bitrate_kbps = total_kbps
        .saturating_sub(audio_bitrate_kbps.unwrap_or(0))
        .clamp(220, 50_000);

    EncodePlan {
        encoder,
        video_bitrate_kbps,
        audio_bitrate_kbps,
        crf: None,
        hardware_cq: None,
        preset: encoder_preset(encoder, preview_mode, true),
        output_width,
        output_height,
        output_fps,
        pass_count: if preview_mode || encoder.is_hardware() {
            1
        } else {
            2
        },
    }
}

fn quality_plan(
    video: &VideoMetadata,
    settings: &VideoSettings,
    preview_mode: bool,
    codec: CodecChoice,
    encoder: ResolvedEncoder,
    output_width: u32,
    output_height: u32,
    output_fps: f32,
) -> EncodePlan {
    let audio_bitrate_kbps = video.has_audio.then_some(quality_audio_bitrate(video));
    let crf = quality_to_crf(settings.quality, codec);
    let video_bitrate_kbps =
        quality_estimated_bitrate(video, settings, codec, output_width, output_height);

    EncodePlan {
        encoder,
        video_bitrate_kbps,
        audio_bitrate_kbps,
        crf: if encoder.is_hardware() {
            None
        } else {
            Some(crf)
        },
        hardware_cq: if encoder.is_hardware() {
            Some(hardware_cq(settings.quality, codec))
        } else {
            None
        },
        preset: encoder_preset(encoder, preview_mode, false),
        output_width,
        output_height,
        output_fps,
        pass_count: 1,
    }
}

fn custom_plan(
    video: &VideoMetadata,
    settings: &VideoSettings,
    preview_mode: bool,
    encoder: ResolvedEncoder,
    output_width: u32,
    output_height: u32,
    output_fps: f32,
) -> EncodePlan {
    let audio_bitrate_kbps = if video.has_audio && settings.custom_audio_enabled {
        Some(settings.custom_audio_bitrate_kbps.clamp(64, 320))
    } else {
        None
    };

    EncodePlan {
        encoder,
        video_bitrate_kbps: settings.custom_bitrate_kbps.clamp(350, 80_000),
        audio_bitrate_kbps,
        crf: None,
        hardware_cq: None,
        preset: encoder_preset(encoder, preview_mode, false),
        output_width,
        output_height,
        output_fps,
        pass_count: 1,
    }
}

fn target_total_bitrate(target_size_mb: u32, duration_secs: f32) -> u32 {
    let bytes = target_size_mb.max(1) as f64 * 1_048_576.0;
    (((bytes * 8.0) / duration_secs.max(1.0) as f64) / 1000.0 * 0.96)
        .round()
        .max(280.0) as u32
}

fn aggressive_audio_bitrate(video: &VideoMetadata) -> u32 {
    video.audio_bitrate_kbps.unwrap_or(128).clamp(64, 96)
}

fn quality_audio_bitrate(video: &VideoMetadata) -> u32 {
    video.audio_bitrate_kbps.unwrap_or(128).clamp(96, 160)
}

fn quality_to_crf(quality: u8, codec: CodecChoice) -> u8 {
    let quality = quality as f32 / 100.0;
    match codec {
        CodecChoice::H264 => (31.0 - quality * 13.0).round() as u8,
        CodecChoice::H265 => (34.0 - quality * 12.0).round() as u8,
        CodecChoice::Av1 => (40.0 - quality * 14.0).round() as u8,
    }
}

fn hardware_cq(quality: u8, codec: CodecChoice) -> u8 {
    let quality = quality as f32 / 100.0;
    let cq = match codec {
        CodecChoice::H264 => 32.0 - quality * 14.0,
        CodecChoice::H265 => 34.0 - quality * 14.0,
        CodecChoice::Av1 => 38.0 - quality * 16.0,
    };
    cq.round().clamp(0.0, 51.0) as u8
}

fn quality_estimated_bitrate(
    video: &VideoMetadata,
    settings: &VideoSettings,
    codec: CodecChoice,
    output_width: u32,
    output_height: u32,
) -> u32 {
    let source_kbps = video
        .video_bitrate_kbps
        .or(video.container_bitrate_kbps)
        .unwrap_or_else(|| fallback_source_bitrate(video))
        .max(500);
    let quality_factor = 0.30 + (settings.quality as f32 / 100.0) * 0.52;
    let scale_factor = (output_width as f32 * output_height as f32)
        / (video.width as f32 * video.height as f32).max(1.0);
    let codec_factor = match codec {
        CodecChoice::H264 => 1.0,
        CodecChoice::H265 => 0.82,
        CodecChoice::Av1 => 0.72,
    };

    (source_kbps as f32 * quality_factor * scale_factor.powf(0.85) * codec_factor)
        .round()
        .clamp(400.0, source_kbps as f32 * 0.97) as u32
}

fn fallback_source_bitrate(video: &VideoMetadata) -> u32 {
    ((video.size_bytes as f64 * 8.0) / video.duration_secs.max(1.0) as f64 / 1000.0).round() as u32
}

fn reduce_size_resolution(video: &VideoMetadata, target_size_mb: u32) -> ResolutionChoice {
    if target_size_mb <= 12 {
        ResolutionChoice::Sd480
    } else if target_size_mb <= 28 {
        ResolutionChoice::Hd720
    } else if video.height > 1080 {
        ResolutionChoice::Hd1080
    } else {
        ResolutionChoice::Original
    }
}

fn resolve_dimensions(video: &VideoMetadata, choice: ResolutionChoice) -> (u32, u32) {
    let max_height = match choice {
        ResolutionChoice::Auto => Some(auto_height(video)),
        ResolutionChoice::Original => None,
        _ => choice.max_height(),
    };

    let Some(max_height) = max_height else {
        return (make_even(video.width), make_even(video.height));
    };

    if video.height <= max_height {
        return (make_even(video.width), make_even(video.height));
    }

    let ratio = max_height as f32 / video.height as f32;
    let width = make_even((video.width as f32 * ratio).round() as u32).max(2);
    let height = make_even(max_height).max(2);

    (width, height)
}

fn auto_height(video: &VideoMetadata) -> u32 {
    if video.height > 1080 {
        1080
    } else {
        video.height
    }
}

fn resolve_fps(video: &VideoMetadata, settings: &VideoSettings) -> f32 {
    match settings.mode {
        CompressionMode::CustomAdvanced => settings
            .custom_fps
            .max(12)
            .min(video.fps.round().max(12.0) as u32)
            as f32,
        _ => video.fps,
    }
}

fn encoder_preset(
    encoder: ResolvedEncoder,
    preview_mode: bool,
    aggressive: bool,
) -> Option<String> {
    if encoder.is_hardware() {
        return match encoder.backend {
            EncoderBackend::Nvidia => Some(
                if preview_mode {
                    "p1"
                } else if aggressive {
                    "p5"
                } else {
                    "p4"
                }
                .to_owned(),
            ),
            EncoderBackend::Amd => Some(
                if preview_mode {
                    "speed"
                } else if aggressive {
                    "quality"
                } else {
                    "balanced"
                }
                .to_owned(),
            ),
            EncoderBackend::Software => None,
        };
    }

    Some(match encoder.codec {
        CodecChoice::H264 => {
            if preview_mode {
                "veryfast".to_owned()
            } else if aggressive {
                "slow".to_owned()
            } else {
                "medium".to_owned()
            }
        }
        CodecChoice::H265 => {
            if preview_mode {
                "faster".to_owned()
            } else {
                "medium".to_owned()
            }
        }
        CodecChoice::Av1 => {
            if preview_mode {
                "8".to_owned()
            } else {
                "6".to_owned()
            }
        }
    })
}

fn make_even(value: u32) -> u32 {
    if value % 2 == 0 { value } else { value - 1 }
}
