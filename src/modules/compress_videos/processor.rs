mod batch;
mod estimates;
mod events;
mod execution;
mod files;
mod parsing;
mod planning;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use crate::modules::compress_videos::models::{
    CompressionEstimate, CompressionMode, CompressionRecommendation, EncoderAvailability,
    EngineInfo, SizeSliderRange, VideoMetadata, VideoSettings,
};

use self::{
    estimates::{estimate_processing_time, estimate_size_bytes},
    execution::{background_command, run_capture},
    parsing::parse_ffprobe_output,
    planning::build_plan,
};

pub use self::{
    batch::{BatchEvent, BatchHandle, BatchItem, start_video_batch},
    files::generate_thumbnail,
};

const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "m4v"];

/// Returns true when the path looks like a supported video file.
pub fn is_supported_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .map(|ext| VIDEO_EXTENSIONS.iter().any(|known| *known == ext))
        .unwrap_or(false)
}

/// Reads metadata for the selected video through ffprobe.
pub fn probe_video(engine: &EngineInfo, path: PathBuf) -> Result<VideoMetadata, String> {
    let mut command = background_command(&engine.ffprobe_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg(
            "format=duration,size,bit_rate:stream=index,codec_type,codec_name,width,height,r_frame_rate,avg_frame_rate,bit_rate",
        )
        .arg("-of")
        .arg("flat=s=_")
        .arg(&path);

    let output = run_capture(command)?;
    parse_ffprobe_output(path, &output)
}

/// Computes the adaptive range used by the target size slider.
pub fn size_slider_range(video: &VideoMetadata) -> SizeSliderRange {
    let original_mb = video.original_size_mb().max(6);
    let min_mb =
        ((original_mb as f32 * 0.08).round() as u32).clamp(4, original_mb.saturating_sub(1).max(4));
    let max_mb =
        ((original_mb as f32 * 0.85).round() as u32).clamp(min_mb + 1, original_mb.max(min_mb + 1));
    let recommended_mb =
        ((original_mb as f32 * recommendation_ratio(video)).round() as u32).clamp(min_mb, max_mb);

    SizeSliderRange {
        min_mb,
        max_mb,
        recommended_mb,
    }
}

/// Builds the live estimate that powers the summary cards.
pub fn estimate_output(
    video: &VideoMetadata,
    settings: &VideoSettings,
    encoders: &EncoderAvailability,
) -> CompressionEstimate {
    let plan = build_plan(video, settings, encoders, false);
    let estimated_size_bytes = estimate_size_bytes(video.duration_secs, &plan);
    let savings_percent = if video.size_bytes == 0 {
        0.0
    } else {
        100.0 - (estimated_size_bytes as f32 / video.size_bytes as f32 * 100.0)
    };
    let recommendation = build_recommendation(video, size_slider_range(video));

    CompressionEstimate {
        original_size_bytes: video.size_bytes,
        estimated_size_bytes,
        estimated_time_secs: estimate_processing_time(video, &plan),
        savings_percent,
        target_width: plan.output_width,
        target_height: plan.output_height,
        pass_count: plan.pass_count,
        recommendation,
    }
}

fn build_recommendation(
    video: &VideoMetadata,
    range: SizeSliderRange,
) -> Option<CompressionRecommendation> {
    if video.original_size_mb() <= 8 {
        return None;
    }

    let target_size_mb = range.recommended_mb;
    let saving_percent = if video.original_size_mb() == 0 {
        0.0
    } else {
        100.0 - (target_size_mb as f32 / video.original_size_mb() as f32 * 100.0)
    };

    Some(CompressionRecommendation {
        headline: format!("Recommended: Reduce to about {target_size_mb} MB"),
        detail: format!(
            "Save about {:.0}% for easier sharing.",
            saving_percent.max(0.0)
        ),
        mode: CompressionMode::ReduceSize,
        target_size_mb,
    })
}

fn recommendation_ratio(video: &VideoMetadata) -> f32 {
    if video.duration_secs > 300.0 || video.height > 1440 {
        0.28
    } else if video.duration_secs > 120.0 {
        0.24
    } else {
        0.20
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_recommendation,
        parsing::{parse_ffprobe_output, parse_time_to_secs},
        size_slider_range,
    };
    use crate::modules::compress_videos::models::{
        CompressionMode, SizeSliderRange, VideoMetadata,
    };
    use std::path::PathBuf;

    #[test]
    fn parses_ffprobe_flat_output() {
        let parsed = parse_ffprobe_output(
            PathBuf::from("clip.mp4"),
            r#"
format_duration="14.500000"
format_size="52428800"
format_bit_rate="28949760"
streams_stream_0_codec_type="video"
streams_stream_0_codec_name="h264"
streams_stream_0_width=1920
streams_stream_0_height=1080
streams_stream_0_avg_frame_rate="30000/1001"
streams_stream_0_bit_rate="28000000"
streams_stream_1_codec_type="audio"
streams_stream_1_bit_rate="128000"
"#,
        )
        .unwrap();

        assert_eq!(parsed.width, 1920);
        assert_eq!(parsed.height, 1080);
        assert_eq!(parsed.video_codec, "h264");
        assert!(parsed.has_audio);
    }

    #[test]
    fn parses_progress_time() {
        let seconds = parse_time_to_secs("00:01:05.50").unwrap();
        assert!((seconds - 65.5).abs() < 0.01);
    }

    #[test]
    fn builds_reasonable_size_range() {
        let video = sample_video();
        let range = size_slider_range(&video);
        assert!(range.min_mb < range.max_mb);
        assert!(range.recommended_mb >= range.min_mb);
        assert!(range.recommended_mb <= range.max_mb);
    }

    #[test]
    fn builds_recommendation_for_large_video() {
        let video = sample_video();
        let recommendation = build_recommendation(
            &video,
            SizeSliderRange {
                min_mb: 6,
                max_mb: 120,
                recommended_mb: 20,
            },
        )
        .unwrap();

        assert_eq!(recommendation.mode, CompressionMode::ReduceSize);
        assert!(recommendation.headline.contains("20 MB"));
    }

    fn sample_video() -> VideoMetadata {
        VideoMetadata {
            path: PathBuf::from("clip.mp4"),
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
}
