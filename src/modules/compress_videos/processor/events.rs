use crate::modules::compress_videos::models::ProcessingProgress;

/// Events emitted by a running FFmpeg encode pass.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum EncodeEvent {
    Progress(ProcessingProgress),
}
