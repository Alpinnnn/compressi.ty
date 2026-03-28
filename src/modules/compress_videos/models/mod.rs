mod engine;
mod media;
mod preview;
mod workflow;

pub use self::{
    engine::{
        EncoderAvailability, EncoderBackend, EngineInfo, EngineSource, EngineStatus,
        ResolvedEncoder,
    },
    media::{
        CompressionEstimate, CompressionRecommendation, CompressionResult, ProcessingProgress,
        VideoCompressionState, VideoMetadata, VideoQueueItem, VideoThumbnail,
    },
    workflow::{CodecChoice, CompressionMode, ResolutionChoice, SizeSliderRange, VideoSettings},
};

pub(in crate::modules::compress_videos) use self::preview::{
    PreviewClickFeedback, PreviewClickFeedbackIcon, PreviewFrame, VideoPreviewState,
};
