use std::time::Instant;

use crate::modules::compress_videos::models::VideoThumbnail;

/// Decoded preview frame and its playback position.
#[derive(Clone, Debug)]
pub(in crate::modules::compress_videos) struct PreviewFrame {
    pub image: VideoThumbnail,
    pub position_secs: f32,
}

/// Transient feedback shown after the user toggles playback in the inline player.
#[derive(Clone, Copy, Debug)]
pub(in crate::modules::compress_videos) struct PreviewClickFeedback {
    pub icon: PreviewClickFeedbackIcon,
    pub shown_at: Instant,
}

/// Playback glyph used by the transient inline player feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::modules::compress_videos) enum PreviewClickFeedbackIcon {
    Play,
    Pause,
}

/// Playback and loading state for the inline video preview player.
#[derive(Clone, Debug, Default)]
pub(in crate::modules::compress_videos) struct VideoPreviewState {
    pub item_id: Option<u64>,
    pub frame: Option<VideoThumbnail>,
    pub duration_secs: f32,
    pub current_position_secs: f32,
    pub scrub_position_secs: Option<f32>,
    pub preview_frame_rate: f32,
    pub is_loading: bool,
    pub load_error: Option<String>,
    pub is_playing: bool,
    pub resume_after_scrub: bool,
    pub click_feedback: Option<PreviewClickFeedback>,
}
