use std::{
    process::Child,
    sync::{Arc, Mutex, atomic::AtomicBool, mpsc},
    thread::JoinHandle,
};

use crate::modules::compress_videos::models::PreviewFrame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::modules::compress_videos) enum PreviewStreamMode {
    Playback,
    SingleFrame,
}

pub(in crate::modules::compress_videos) struct PreviewStreamEvent {
    pub frame: Option<PreviewFrame>,
    pub finished: bool,
    pub error: Option<String>,
}

pub(in crate::modules::compress_videos) struct RunningPreviewStream {
    pub id: u64,
    pub mode: PreviewStreamMode,
    pub receiver: mpsc::Receiver<PreviewStreamEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub shared_child: Arc<Mutex<Option<Child>>>,
    pub worker: Option<JoinHandle<()>>,
}
