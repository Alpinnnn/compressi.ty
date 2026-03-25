use crate::modules::compress_videos::models::{CodecChoice, EncoderBackend, VideoMetadata};

use super::planning::EncodePlan;

pub(super) fn estimate_size_bytes(duration_secs: f32, plan: &EncodePlan) -> u64 {
    let total_kbps = plan.video_bitrate_kbps + plan.audio_bitrate_kbps.unwrap_or(0);
    ((total_kbps as f64 * 1000.0 * duration_secs.max(1.0) as f64) / 8.0 * 1.02).round() as u64
}

pub(super) fn estimate_processing_time(video: &VideoMetadata, plan: &EncodePlan) -> f32 {
    let pixel_factor = (plan.output_width as f32 * plan.output_height as f32) / (1920.0 * 1080.0);
    let fps_factor = (plan.output_fps / 30.0).max(0.75);
    let complexity = (pixel_factor * fps_factor).max(0.35);
    let base_speed = match plan.encoder.backend {
        EncoderBackend::Software => match plan.encoder.codec {
            CodecChoice::H264 => {
                if plan.crf.is_some() {
                    1.45
                } else {
                    1.05
                }
            }
            CodecChoice::H265 => {
                if plan.pass_count == 2 {
                    0.62
                } else {
                    0.48
                }
            }
            CodecChoice::Av1 => 0.22,
        },
        EncoderBackend::Nvidia => match plan.encoder.codec {
            CodecChoice::H264 => 5.8,
            CodecChoice::H265 => 4.4,
            CodecChoice::Av1 => 3.0,
        },
        EncoderBackend::Amd => match plan.encoder.codec {
            CodecChoice::H264 => 4.6,
            CodecChoice::H265 => 3.5,
            CodecChoice::Av1 => 2.4,
        },
        EncoderBackend::IntelQuickSync => match plan.encoder.codec {
            CodecChoice::H264 => 4.8,
            CodecChoice::H265 => 3.7,
            CodecChoice::Av1 => 2.6,
        },
    };
    let speed_x = (base_speed / complexity).clamp(0.08, 10.0);
    (video.duration_secs * plan.pass_count as f32) / speed_x
}
