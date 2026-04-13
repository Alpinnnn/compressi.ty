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
    if !first_pass {
        command.arg("-movflags").arg("+faststart");
    }
    command
        .arg("-g")
        .arg(recommended_gop_size(plan.output_fps).to_string());

    apply_rate_control(&mut command, plan);

    if let Some(passlog) = passlog {
        command.arg("-passlogfile").arg(passlog);
        command.arg("-pass").arg(if first_pass { "1" } else { "2" });
    }

    apply_codec_specific_options(&mut command, plan, first_pass);
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
        let pixel_filter = match plan.encoder.backend {
            EncoderBackend::IntelQuickSync => "format=nv12",
            EncoderBackend::Software | EncoderBackend::Nvidia | EncoderBackend::Amd => {
                "format=yuv420p"
            }
        };
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
        None => match plan.encoder.backend {
            EncoderBackend::IntelQuickSync => {
                if let Some(hardware_cq) = plan.hardware_cq {
                    command.arg("-global_quality").arg(hardware_cq.to_string());
                } else {
                    apply_constrained_vbr(command, plan.video_bitrate_kbps);
                }
            }
            EncoderBackend::Nvidia | EncoderBackend::Amd => {
                if let Some(hardware_cq) = plan.hardware_cq {
                    command
                        .arg("-rc:v")
                        .arg("vbr")
                        .arg("-cq:v")
                        .arg(hardware_cq.to_string())
                        .arg("-b:v")
                        .arg(format!("{}k", plan.video_bitrate_kbps))
                        .arg("-maxrate")
                        .arg(format!(
                            "{}k",
                            constrained_peak_kbps(plan.video_bitrate_kbps)
                        ))
                        .arg("-bufsize")
                        .arg(format!(
                            "{}k",
                            constrained_peak_kbps(plan.video_bitrate_kbps)
                        ));
                } else {
                    command.arg("-rc:v").arg("vbr");
                    apply_constrained_vbr(command, plan.video_bitrate_kbps);
                }
            }
            EncoderBackend::Software => {
                if plan.pass_count == 2 {
                    command
                        .arg("-b:v")
                        .arg(format!("{}k", plan.video_bitrate_kbps));
                } else {
                    apply_constrained_vbr(command, plan.video_bitrate_kbps);
                }
            }
        },
    }
}

fn apply_codec_specific_options(command: &mut Command, plan: &EncodePlan, first_pass: bool) {
    match plan.encoder.codec {
        CodecChoice::H264 => {}
        CodecChoice::H265 => {
            if !first_pass {
                command.arg("-tag:v").arg("hvc1");
            }
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
        command.arg("-f").arg("null");
    } else if let Some(audio_bitrate_kbps) = plan.audio_bitrate_kbps {
        command
            .arg("-c:a")
            .arg(plan.audio_encoder_name)
            .arg("-b:a")
            .arg(format!("{}k", audio_bitrate_kbps));
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

fn recommended_gop_size(output_fps: f32) -> u32 {
    ((output_fps.max(12.0) * 2.0).round() as u32).clamp(24, 240)
}

fn constrained_peak_kbps(target_kbps: u32) -> u32 {
    target_kbps
        .saturating_mul(2)
        .max(target_kbps.saturating_add(1))
}

fn apply_constrained_vbr(command: &mut Command, target_kbps: u32) {
    let peak_kbps = constrained_peak_kbps(target_kbps);
    command
        .arg("-b:v")
        .arg(format!("{}k", target_kbps))
        .arg("-maxrate")
        .arg(format!("{}k", peak_kbps))
        .arg("-bufsize")
        .arg(format!("{}k", peak_kbps));
}

#[cfg(test)]
mod tests {
    use super::build_encode_command;
    use crate::modules::compress_videos::models::{
        CodecChoice, EncoderBackend, ResolvedEncoder, VideoMetadata,
    };
    use crate::modules::compress_videos::processor::planning::strategy::EncodePlan;
    use std::{path::Path, process::Command};

    #[test]
    fn first_pass_uses_null_muxer_without_faststart_or_vbv_caps() {
        let video = sample_video();
        let plan = sample_plan();

        let command = build_encode_command(
            Path::new("ffmpeg"),
            &video,
            &EncodePlan {
                encoder: ResolvedEncoder {
                    codec: CodecChoice::H265,
                    backend: EncoderBackend::Software,
                },
                pass_count: 2,
                ..plan
            },
            0.0,
            video.duration_secs,
            Path::new("NUL"),
            Some(Path::new("passlog")),
            true,
            false,
        );

        let args = command_args(&command);
        assert!(contains_arg_pair(&args, "-f", "null"));
        assert!(contains_arg_pair(&args, "-b:v", "2400k"));
        assert!(!contains_flag(&args, "-movflags"));
        assert!(!contains_flag(&args, "-maxrate"));
        assert!(!contains_arg_pair(&args, "-tag:v", "hvc1"));
    }

    #[test]
    fn final_pass_uses_selected_audio_encoder_without_forcing_stereo() {
        let video = sample_video();
        let command = build_encode_command(
            Path::new("ffmpeg"),
            &video,
            &EncodePlan {
                audio_encoder_name: "libfdk_aac",
                ..sample_plan()
            },
            0.0,
            video.duration_secs,
            Path::new("out.mp4"),
            None,
            false,
            false,
        );

        let args = command_args(&command);
        assert!(contains_arg_pair(&args, "-c:a", "libfdk_aac"));
        assert!(contains_arg_pair(&args, "-b:a", "128k"));
        assert!(!contains_flag(&args, "-ac"));
    }

    #[test]
    fn one_pass_bitrate_mode_uses_constrained_vbr_and_dynamic_gop() {
        let video = sample_video();
        let command = build_encode_command(
            Path::new("ffmpeg"),
            &video,
            &EncodePlan {
                output_fps: 30.0,
                ..sample_plan()
            },
            0.0,
            video.duration_secs,
            Path::new("out.mp4"),
            None,
            false,
            false,
        );

        let args = command_args(&command);
        assert!(contains_arg_pair(&args, "-b:v", "2400k"));
        assert!(contains_arg_pair(&args, "-maxrate", "4800k"));
        assert!(contains_arg_pair(&args, "-bufsize", "4800k"));
        assert!(contains_arg_pair(&args, "-g", "60"));
    }

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn contains_flag(args: &[String], flag: &str) -> bool {
        args.iter().any(|arg| arg == flag)
    }

    fn contains_arg_pair(args: &[String], key: &str, value: &str) -> bool {
        args.windows(2)
            .any(|window| window[0] == key && window[1] == value)
    }

    fn sample_video() -> VideoMetadata {
        VideoMetadata {
            path: Path::new("clip.mp4").to_path_buf(),
            file_name: "clip.mp4".to_owned(),
            size_bytes: 80 * 1_048_576,
            duration_secs: 42.0,
            width: 1920,
            height: 1080,
            fps: 30.0,
            container_bitrate_kbps: Some(12_000),
            video_bitrate_kbps: Some(11_400),
            audio_bitrate_kbps: Some(128),
            video_codec: "h264".to_owned(),
            has_audio: true,
        }
    }

    fn sample_plan() -> EncodePlan {
        EncodePlan {
            encoder: ResolvedEncoder {
                codec: CodecChoice::H264,
                backend: EncoderBackend::Software,
            },
            video_bitrate_kbps: 2_400,
            audio_bitrate_kbps: Some(128),
            audio_encoder_name: "aac",
            crf: None,
            hardware_cq: None,
            preset: Some("medium".to_owned()),
            output_width: 1920,
            output_height: 1080,
            output_fps: 30.0,
            pass_count: 1,
        }
    }
}
