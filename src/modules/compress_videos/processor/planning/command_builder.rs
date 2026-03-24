use std::{path::Path, process::Command};

use crate::modules::compress_videos::models::{CodecChoice, EncoderBackend, VideoMetadata};

use super::{
    super::execution::{background_command, format_time_arg},
    strategy::EncodePlan,
};

pub(in crate::modules::compress_videos::processor) fn build_encode_command(
    ffmpeg_binary: &Path,
    video: &VideoMetadata,
    plan: &EncodePlan,
    start_secs: f32,
    clip_duration_secs: f32,
    output_path: &Path,
    passlog: Option<&Path>,
    first_pass: bool,
    preview_mode: bool,
) -> Command {
    let mut command = background_command(ffmpeg_binary);
    command
        .arg("-hide_banner")
        .arg("-y")
        .arg("-nostdin")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-stats_period")
        .arg("0.25");

    if plan.encoder.is_hardware() {
        command.arg("-hwaccel").arg("auto");
    }

    if start_secs > 0.0 {
        command.arg("-ss").arg(format_time_arg(start_secs));
    }

    command.arg("-i").arg(&video.path);

    if clip_duration_secs > 0.0 {
        command.arg("-t").arg(format_time_arg(clip_duration_secs));
    }

    command.arg("-map").arg("0:v:0");
    if video.has_audio && !first_pass {
        command.arg("-map").arg("0:a?");
    }

    command.arg("-sn").arg("-dn");

    apply_video_filters(&mut command, video, plan);

    command.arg("-c:v").arg(plan.encoder.ffmpeg_name());
    if let Some(preset) = &plan.preset {
        command.arg("-preset").arg(preset);
    }
    if !plan.encoder.is_hardware() {
        command.arg("-pix_fmt").arg("yuv420p");
    }
    command.arg("-movflags").arg("+faststart");
    command.arg("-g").arg("240");

    apply_rate_control(&mut command, plan);

    if let Some(passlog) = passlog {
        command.arg("-passlogfile").arg(passlog);
        command.arg("-pass").arg(if first_pass { "1" } else { "2" });
    }

    apply_codec_specific_options(&mut command, plan);
    apply_audio_options(&mut command, plan, first_pass);

    if preview_mode {
        command.arg("-map_metadata").arg("-1");
    }

    command.arg(output_path);
    command
}

fn apply_video_filters(command: &mut Command, video: &VideoMetadata, plan: &EncodePlan) {
    let filter = build_filter_chain(video, plan);

    if plan.encoder.is_hardware() {
        let pixel_filter = "format=yuv420p";
        if filter.is_empty() {
            command.arg("-vf").arg(pixel_filter);
        } else {
            command.arg("-vf").arg(format!("{filter},{pixel_filter}"));
        }
    } else if !filter.is_empty() {
        command.arg("-vf").arg(filter);
    }
}

fn apply_rate_control(command: &mut Command, plan: &EncodePlan) {
    match plan.crf {
        Some(crf) => {
            command.arg("-crf").arg(crf.to_string());
        }
        None => {
            if let Some(hardware_cq) = plan.hardware_cq.filter(|_| plan.encoder.is_hardware()) {
                command
                    .arg("-rc:v")
                    .arg("vbr")
                    .arg("-cq:v")
                    .arg(hardware_cq.to_string())
                    .arg("-b:v")
                    .arg(format!("{}k", plan.video_bitrate_kbps))
                    .arg("-maxrate")
                    .arg(format!("{}k", plan.video_bitrate_kbps.saturating_mul(2)));
            } else {
                command
                    .arg("-b:v")
                    .arg(format!("{}k", plan.video_bitrate_kbps))
                    .arg("-maxrate")
                    .arg(format!("{}k", plan.video_bitrate_kbps))
                    .arg("-bufsize")
                    .arg(format!("{}k", plan.video_bitrate_kbps.saturating_mul(2)));
            }
        }
    }
}

fn apply_codec_specific_options(command: &mut Command, plan: &EncodePlan) {
    match plan.encoder.codec {
        CodecChoice::H264 => {}
        CodecChoice::H265 => {
            command.arg("-tag:v").arg("hvc1");
        }
        CodecChoice::Av1 => {
            if matches!(plan.encoder.backend, EncoderBackend::Software) {
                command.arg("-svtav1-params").arg("tune=0");
            }
        }
    }
}

fn apply_audio_options(command: &mut Command, plan: &EncodePlan, first_pass: bool) {
    if first_pass {
        command.arg("-an");
        command.arg("-f").arg("mp4");
    } else if let Some(audio_bitrate_kbps) = plan.audio_bitrate_kbps {
        command
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg(format!("{}k", audio_bitrate_kbps))
            .arg("-ac")
            .arg("2");
    } else {
        command.arg("-an");
    }
}

fn build_filter_chain(video: &VideoMetadata, plan: &EncodePlan) -> String {
    let mut filters = Vec::new();

    if plan.output_width != video.width || plan.output_height != video.height {
        filters.push(format!(
            "scale={}:{}",
            plan.output_width, plan.output_height
        ));
    }

    if plan.output_fps + 0.25 < video.fps {
        filters.push(format!("fps={:.2}", plan.output_fps));
    }

    filters.join(",")
}
