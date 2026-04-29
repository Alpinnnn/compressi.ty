use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    modules::compress_videos::models::{EncoderAvailability, EngineInfo, EngineSource},
    runtime,
};

use super::process_utils::{background_command, run_capture};

const HARDWARE_PROBE_FRAME_SIZE: &str = "256x256";

pub(super) fn discover_engine_in_dir(
    dir: &Path,
    source: EngineSource,
) -> Result<Option<EngineInfo>, String> {
    if !runtime::engine_binaries_exist(dir) {
        return Ok(None);
    }

    let ffmpeg = dir.join(runtime::ffmpeg_binary_name());
    let ffprobe = dir.join(runtime::ffprobe_binary_name());
    let info = inspect_engine(ffmpeg, ffprobe, source)?;
    Ok(Some(info))
}

pub(super) fn discover_system_engine() -> Result<EngineInfo, String> {
    inspect_engine(
        PathBuf::from(runtime::ffmpeg_binary_name()),
        PathBuf::from(runtime::ffprobe_binary_name()),
        EngineSource::SystemPath,
    )
}

fn inspect_engine(
    ffmpeg_path: PathBuf,
    ffprobe_path: PathBuf,
    source: EngineSource,
) -> Result<EngineInfo, String> {
    let mut version_command = background_command(&ffmpeg_path);
    version_command.arg("-version");
    let version_output = run_capture(version_command).map_err(|error| {
        format!(
            "Could not read FFmpeg version from {}: {error}",
            ffmpeg_path.display()
        )
    })?;

    let mut encoders_command = background_command(&ffmpeg_path);
    encoders_command.arg("-hide_banner").arg("-encoders");
    let encoders_output = run_capture(encoders_command)
        .map_err(|error| format!("Could not inspect FFmpeg encoders: {error}"))?;

    let h264_software = encoders_output.contains(" libx264 ");
    let h265_software = encoders_output.contains(" libx265 ");
    let av1_software = encoders_output.contains(" libsvtav1 ");
    let aac = encoder_list_contains(&encoders_output, "aac");
    let libfdk_aac = encoder_list_contains(&encoders_output, "libfdk_aac");
    let flac = encoder_list_contains(&encoders_output, "flac");
    let libopus = encoder_list_contains(&encoders_output, "libopus");
    let opus = encoder_list_contains(&encoders_output, "opus");
    let libmp3lame = encoder_list_contains(&encoders_output, "libmp3lame");
    let libshine = encoder_list_contains(&encoders_output, "libshine");

    let h264_nvidia = encoder_list_contains(&encoders_output, "h264_nvenc")
        && probe_hardware_encoder(&ffmpeg_path, "h264_nvenc");
    let h265_nvidia = encoder_list_contains(&encoders_output, "hevc_nvenc")
        && probe_hardware_encoder(&ffmpeg_path, "hevc_nvenc");
    let av1_nvidia = encoder_list_contains(&encoders_output, "av1_nvenc")
        && probe_hardware_encoder(&ffmpeg_path, "av1_nvenc");

    let h264_amd = encoder_list_contains(&encoders_output, "h264_amf")
        && probe_hardware_encoder(&ffmpeg_path, "h264_amf");
    let h265_amd = encoder_list_contains(&encoders_output, "hevc_amf")
        && probe_hardware_encoder(&ffmpeg_path, "hevc_amf");
    let av1_amd = encoder_list_contains(&encoders_output, "av1_amf")
        && probe_hardware_encoder(&ffmpeg_path, "av1_amf");
    let h264_intel_qsv = encoder_list_contains(&encoders_output, "h264_qsv")
        && probe_hardware_encoder(&ffmpeg_path, "h264_qsv");
    let h265_intel_qsv = encoder_list_contains(&encoders_output, "hevc_qsv")
        && probe_hardware_encoder(&ffmpeg_path, "hevc_qsv");
    let av1_intel_qsv = encoder_list_contains(&encoders_output, "av1_qsv")
        && probe_hardware_encoder(&ffmpeg_path, "av1_qsv");

    Ok(EngineInfo {
        version: version_output
            .lines()
            .next()
            .map(str::trim)
            .unwrap_or("FFmpeg")
            .to_owned(),
        ffmpeg_path,
        ffprobe_path,
        encoders: EncoderAvailability {
            h264: h264_software || h264_nvidia || h264_amd || h264_intel_qsv,
            h265: h265_software || h265_nvidia || h265_amd || h265_intel_qsv,
            av1: av1_software || av1_nvidia || av1_amd || av1_intel_qsv,
            aac,
            libfdk_aac,
            flac,
            libopus,
            opus,
            libmp3lame,
            libshine,
            h264_nvidia,
            h265_nvidia,
            av1_nvidia,
            h264_amd,
            h265_amd,
            av1_amd,
            h264_intel_qsv,
            h265_intel_qsv,
            av1_intel_qsv,
        },
        source,
    })
}

fn encoder_list_contains(encoders_output: &str, encoder_name: &str) -> bool {
    encoders_output
        .lines()
        .any(|line| line.split_whitespace().any(|token| token == encoder_name))
}

fn probe_hardware_encoder(ffmpeg_path: &Path, encoder_name: &str) -> bool {
    if probe_hw_null(ffmpeg_path, encoder_name) {
        return true;
    }

    probe_hw_tempfile(ffmpeg_path, encoder_name)
}

fn probe_hw_null(ffmpeg_path: &Path, encoder_name: &str) -> bool {
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let probe_pixel_format = hardware_probe_pixel_format(encoder_name);

    let mut command = background_command(ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!("color=c=black:s={HARDWARE_PROBE_FRAME_SIZE}:d=0.1"))
        .arg("-frames:v")
        .arg("1")
        .arg("-an")
        .arg("-c:v")
        .arg(encoder_name)
        .arg("-pix_fmt")
        .arg(probe_pixel_format)
        .arg("-f")
        .arg("null")
        .arg(null_device);

    crate::process_lifecycle::output(&mut command)
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn probe_hw_tempfile(ffmpeg_path: &Path, encoder_name: &str) -> bool {
    let temp_dir = std::env::temp_dir().join("compressi.ty").join("gpu-probe");
    if fs::create_dir_all(&temp_dir).is_err() {
        return false;
    }

    let temp_file = temp_dir.join(format!("{encoder_name}_probe.mp4"));
    let probe_pixel_format = hardware_probe_pixel_format(encoder_name);

    let mut command = background_command(ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-y")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!("color=c=black:s={HARDWARE_PROBE_FRAME_SIZE}:d=0.1"))
        .arg("-frames:v")
        .arg("1")
        .arg("-an")
        .arg("-c:v")
        .arg(encoder_name)
        .arg("-pix_fmt")
        .arg(probe_pixel_format)
        .arg(&temp_file);

    let result = crate::process_lifecycle::output(&mut command)
        .map(|output| output.status.success())
        .unwrap_or(false);

    let _ = fs::remove_file(&temp_file);
    result
}

fn hardware_probe_pixel_format(encoder_name: &str) -> &'static str {
    // QSV is most reliable with NV12 input frames for 8-bit encode probes.
    if encoder_name.ends_with("_qsv") {
        "nv12"
    } else {
        "yuv420p"
    }
}
