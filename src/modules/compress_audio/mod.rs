mod logic;
pub mod models;
mod ui;

use std::{collections::HashSet, path::PathBuf, sync::mpsc, time::Duration};

use crate::{
    modules::{
        compress_audio::{
            logic::{AudioBatchEvent, AudioBatchHandle, AudioBatchItem},
            models::{AudioAnalysis, AudioMetadata, AudioQueueItem},
        },
        compress_videos::engine::VideoEngineController,
    },
    runtime,
};

use self::{
    logic::{AudioPreviewPlayer, analyze_audio, probe_audio, start_audio_batch},
    models::{AudioCompressionSettings, AudioCompressionState, AudioProcessingProgress},
};

pub(crate) use self::logic::is_supported_audio_path;

/// Audio compression workspace state and queue orchestration.
pub struct CompressAudioPage {
    queue: Vec<AudioQueueItem>,
    next_id: u64,
    selected_id: Option<u64>,

    active_batch: Option<AudioBatchHandle>,
    pending_compression_ids: Vec<u64>,
    pending_probes: Vec<PendingProbe>,
    deferred_paths: Vec<PathBuf>,

    output_dir: Option<PathBuf>,
    output_dir_user_set: bool,
    last_output_dir: Option<PathBuf>,

    banner: Option<BannerMessage>,
    show_cancel_all_confirm: bool,
    preview_player: AudioPreviewPlayer,
    preview_scrub_position: Option<(u64, f32)>,
    track_info_open: bool,
    file_picker_rx: Option<crate::file_dialog::DialogReceiver<Vec<PathBuf>>>,
    output_folder_picker_rx: Option<crate::file_dialog::DialogReceiver<PathBuf>>,
}

struct PendingProbe {
    id: u64,
    receiver: mpsc::Receiver<Result<ProbeResult, String>>,
    encoders: crate::modules::compress_videos::models::EncoderAvailability,
}

struct ProbeResult {
    metadata: AudioMetadata,
    analysis: AudioAnalysis,
}

pub(super) struct BannerMessage {
    tone: BannerTone,
    text: String,
}

pub(super) enum BannerTone {
    Info,
    Success,
    Error,
}

impl Default for CompressAudioPage {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            next_id: 0,
            selected_id: None,
            active_batch: None,
            pending_compression_ids: Vec::new(),
            pending_probes: Vec::new(),
            deferred_paths: Vec::new(),
            output_dir: None,
            output_dir_user_set: false,
            last_output_dir: None,
            banner: None,
            show_cancel_all_confirm: false,
            preview_player: AudioPreviewPlayer::default(),
            preview_scrub_position: None,
            track_info_open: false,
            file_picker_rx: None,
            output_folder_picker_rx: None,
        }
    }
}

impl CompressAudioPage {
    /// Queues audio files supplied by external launch requests or shell integrations.
    pub fn queue_external_paths(
        &mut self,
        paths: Vec<PathBuf>,
        engine: &mut VideoEngineController,
    ) {
        self.add_paths(paths, engine);
    }

    /// Polls background analysis and compression workers, returning the preferred repaint cadence.
    pub fn poll_background(&mut self, engine: &mut VideoEngineController) -> Option<Duration> {
        self.flush_deferred_paths(engine);
        self.poll_probes();
        self.poll_batch();
        if !self.pending_compression_ids.is_empty()
            && engine.active_info().is_none()
            && !engine.is_busy()
        {
            engine.ensure_ready();
        }
        self.start_next_scheduled_audio(engine);

        let busy = !self.pending_probes.is_empty()
            || self.active_batch.is_some()
            || !self.pending_compression_ids.is_empty();
        if busy {
            Some(Duration::from_millis(50))
        } else {
            None
        }
    }

    /// Returns whether an audio compression batch is currently running or queued.
    pub fn is_compressing(&self) -> bool {
        self.active_batch.is_some() || !self.pending_compression_ids.is_empty()
    }

    /// Requests cancellation of the active audio compression batch and queued follow-up jobs.
    pub fn cancel_compression(&mut self) {
        let pending_ids = std::mem::take(&mut self.pending_compression_ids);
        for id in pending_ids {
            if let Some(item) = self.find_item_mut(id)
                && matches!(&item.state, AudioCompressionState::Ready)
            {
                item.state = AudioCompressionState::Cancelled;
            }
        }

        if let Some(active_batch) = &self.active_batch {
            active_batch.cancel();
        }
    }

    /// Cancels only the currently active audio job without clearing the queued follow-up jobs.
    pub fn cancel_active_audio(&self) {
        if let Some(active_batch) = &self.active_batch {
            active_batch.cancel();
        }
    }

    fn poll_probes(&mut self) {
        let mut completed = Vec::new();
        for (index, pending_probe) in self.pending_probes.iter().enumerate() {
            match pending_probe.receiver.try_recv() {
                Ok(result) => completed.push((
                    index,
                    pending_probe.id,
                    pending_probe.encoders.clone(),
                    result,
                )),
                Err(mpsc::TryRecvError::Disconnected) => completed.push((
                    index,
                    pending_probe.id,
                    pending_probe.encoders.clone(),
                    Err("Audio analysis stopped unexpectedly.".to_owned()),
                )),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        for (index, id, encoders, result) in completed.into_iter().rev() {
            self.pending_probes.remove(index);
            match result {
                Ok(probe_result) => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.metadata = Some(probe_result.metadata);
                        item.analysis = Some(probe_result.analysis);
                        item.settings = Some(AudioCompressionSettings::new(&encoders));
                        item.state = AudioCompressionState::Ready;
                    }
                }
                Err(error) => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Failed(error);
                    }
                }
            }
        }
    }

    fn poll_batch(&mut self) {
        let mut finished = None;
        let mut batch_item_ids = Vec::new();
        let mut clear_selected_id = false;
        let mut batch_events = Vec::new();
        if let Some(batch) = &self.active_batch {
            batch_item_ids = batch.item_ids.clone();
            while let Ok(event) = batch.receiver.try_recv() {
                batch_events.push(event);
            }
        }

        for event in batch_events {
            match event {
                AudioBatchEvent::ItemStarted { id } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Compressing(AudioProcessingProgress {
                            progress: 0.02,
                            stage: "Starting".to_owned(),
                            speed_x: 0.0,
                            eta_secs: None,
                        });
                    }
                    if self.selected_id == Some(id) {
                        clear_selected_id = true;
                    }
                }
                AudioBatchEvent::ItemProgress { id, progress } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Compressing(progress);
                    }
                }
                AudioBatchEvent::ItemFinished { id, result } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Completed(result);
                    }
                    if self.selected_id == Some(id) {
                        clear_selected_id = true;
                    }
                }
                AudioBatchEvent::ItemSkipped { id, reason } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Skipped(reason);
                    }
                    if self.selected_id == Some(id) {
                        clear_selected_id = true;
                    }
                }
                AudioBatchEvent::ItemFailed { id, error } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Failed(error);
                    }
                }
                AudioBatchEvent::BatchFinished { cancelled } => {
                    finished = Some(cancelled);
                }
            }
        }

        if clear_selected_id {
            self.selected_id = None;
            self.preview_player.stop();
            self.preview_scrub_position = None;
        }

        if let Some(cancelled) = finished {
            if cancelled {
                for item in &mut self.queue {
                    if batch_item_ids.contains(&item.id)
                        && matches!(
                            item.state,
                            AudioCompressionState::Ready | AudioCompressionState::Compressing(_)
                        )
                    {
                        item.state = AudioCompressionState::Cancelled;
                    }
                }
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Compression cancelled for the current audio.".to_owned(),
                });
            } else if self.pending_compression_ids.is_empty() {
                let completed_count = self
                    .queue
                    .iter()
                    .filter(|item| matches!(&item.state, AudioCompressionState::Completed(_)))
                    .count();
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Success,
                    text: format!("Done - {completed_count} audio file(s) compressed."),
                });
            }

            self.active_batch = None;
        }
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>, engine: &mut VideoEngineController) {
        let audio_paths = runtime::collect_matching_paths(paths, is_supported_audio_path);
        if audio_paths.is_empty() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Error,
                text: "No supported audio files were found.".to_owned(),
            });
            return;
        }

        let existing = self
            .queue
            .iter()
            .map(|item| item.source_path.clone())
            .collect::<HashSet<_>>();
        let fresh_paths = audio_paths
            .into_iter()
            .filter(|path| !existing.contains(path))
            .collect::<Vec<_>>();

        if fresh_paths.is_empty() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Those audio files are already in the queue.".to_owned(),
            });
            return;
        }

        if let Some(engine_info) = engine.active_info().cloned() {
            self.enqueue_paths(fresh_paths, engine_info);
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Added audio files to the queue.".to_owned(),
            });
        } else {
            self.deferred_paths.extend(fresh_paths);
            engine.ensure_ready();
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Preparing the FFmpeg engine so your audio files can be analyzed..."
                    .to_owned(),
            });
        }
    }

    fn flush_deferred_paths(&mut self, engine: &mut VideoEngineController) {
        if self.deferred_paths.is_empty() {
            return;
        }

        let Some(engine_info) = engine.active_info().cloned() else {
            if !engine.is_busy() {
                engine.ensure_ready();
            }
            return;
        };

        let paths = std::mem::take(&mut self.deferred_paths);
        self.enqueue_paths(paths, engine_info);
        self.banner = Some(BannerMessage {
            tone: BannerTone::Info,
            text: "Audio files analyzed and ready to review.".to_owned(),
        });
    }

    fn enqueue_paths(
        &mut self,
        paths: Vec<PathBuf>,
        engine_info: crate::modules::compress_videos::models::EngineInfo,
    ) {
        for path in paths {
            let id = self.next_id;
            self.next_id += 1;

            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("audio")
                .to_owned();

            self.queue.push(AudioQueueItem {
                id,
                source_path: path.clone(),
                file_name,
                metadata: None,
                analysis: None,
                settings: None,
                state: AudioCompressionState::Analyzing,
            });

            let (sender, receiver) = mpsc::channel();
            let probe_engine = engine_info.clone();
            let encoders = probe_engine.encoders.clone();
            std::thread::spawn(move || {
                let result = probe_audio(&probe_engine, path).map(|metadata| ProbeResult {
                    analysis: analyze_audio(&metadata, &probe_engine.encoders),
                    metadata,
                });
                let _ = sender.send(result);
            });

            self.pending_probes.push(PendingProbe {
                id,
                receiver,
                encoders,
            });
        }
    }

    pub(in crate::modules::compress_audio) fn start_batch_compression(
        &mut self,
        engine: &VideoEngineController,
    ) {
        let scheduled_count = self.schedule_ready_audios();
        if scheduled_count == 0 && self.active_batch.is_none() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "No audio files ready to compress.".to_owned(),
            });
            return;
        }

        self.start_next_scheduled_audio(engine);

        if scheduled_count > 0 {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: if self.active_batch.is_some() {
                    format!("{scheduled_count} audio file(s) added to the compression queue.")
                } else {
                    format!("{scheduled_count} audio file(s) scheduled for compression.")
                },
            });
        }
    }

    pub(in crate::modules::compress_audio) fn start_single_compression(
        &mut self,
        id: u64,
        engine: &VideoEngineController,
    ) {
        if !self.schedule_audio(id) {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Audio is already queued for compression.".to_owned(),
            });
            return;
        }

        self.start_next_scheduled_audio(engine);
        self.banner = Some(BannerMessage {
            tone: BannerTone::Info,
            text: "Audio added to the compression queue.".to_owned(),
        });
    }

    fn schedule_ready_audios(&mut self) -> usize {
        let ids = self
            .queue
            .iter()
            .filter(|item| matches!(&item.state, AudioCompressionState::Ready))
            .map(|item| item.id)
            .collect::<Vec<_>>();
        let mut scheduled = 0;
        for id in ids {
            if self.schedule_audio(id) {
                scheduled += 1;
            }
        }
        scheduled
    }

    fn schedule_audio(&mut self, id: u64) -> bool {
        let active_contains = self
            .active_batch
            .as_ref()
            .map(|batch| batch.item_ids.contains(&id))
            .unwrap_or(false);
        if active_contains || self.pending_compression_ids.contains(&id) {
            return false;
        }

        if self
            .queue
            .iter()
            .any(|item| item.id == id && matches!(&item.state, AudioCompressionState::Ready))
        {
            self.pending_compression_ids.push(id);
            true
        } else {
            false
        }
    }

    fn start_next_scheduled_audio(&mut self, engine: &VideoEngineController) {
        if self.active_batch.is_some() {
            return;
        }

        let Some(engine_info) = engine.active_info().cloned() else {
            return;
        };

        while let Some(id) = self.pending_compression_ids.first().copied() {
            let Some(item) = self
                .queue
                .iter()
                .find(|item| item.id == id && matches!(&item.state, AudioCompressionState::Ready))
                .cloned()
            else {
                self.pending_compression_ids.remove(0);
                continue;
            };

            let Some(metadata) = item.metadata else {
                self.pending_compression_ids.remove(0);
                continue;
            };
            let Some(settings) = item.settings else {
                self.pending_compression_ids.remove(0);
                continue;
            };

            let batch_item = AudioBatchItem {
                id,
                metadata,
                settings,
            };
            match start_audio_batch(
                engine_info.clone(),
                vec![batch_item],
                self.output_dir.clone(),
            ) {
                Ok(handle) => {
                    self.last_output_dir = Some(handle.output_dir.clone());
                    self.active_batch = Some(handle);
                    self.pending_compression_ids.remove(0);
                }
                Err(error) => {
                    self.pending_compression_ids.remove(0);
                    self.banner = Some(BannerMessage {
                        tone: BannerTone::Error,
                        text: error,
                    });
                }
            }
            return;
        }
    }

    pub(in crate::modules::compress_audio) fn request_cancel_all(&mut self) {
        self.show_cancel_all_confirm = true;
    }

    pub(in crate::modules::compress_audio) fn confirm_cancel_all(&mut self) {
        self.show_cancel_all_confirm = false;
        let had_active_job = self.active_batch.is_some();
        self.cancel_compression();
        if !had_active_job {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "All queued compression jobs were cancelled.".to_owned(),
            });
        }
    }

    pub(in crate::modules::compress_audio) fn dismiss_cancel_all(&mut self) {
        self.show_cancel_all_confirm = false;
    }

    pub(in crate::modules::compress_audio) fn has_pending_compression(&self) -> bool {
        self.active_batch.is_some() || !self.pending_compression_ids.is_empty()
    }

    pub(in crate::modules::compress_audio) fn is_audio_pending_compression(&self, id: u64) -> bool {
        self.pending_compression_ids.contains(&id)
            || self
                .active_batch
                .as_ref()
                .map(|batch| batch.item_ids.contains(&id))
                .unwrap_or(false)
    }

    pub(in crate::modules::compress_audio) fn clear_queue(&mut self) {
        self.queue.clear();
        self.pending_probes.clear();
        self.pending_compression_ids.clear();
        self.deferred_paths.clear();
        self.selected_id = None;
        self.preview_player.stop();
        self.preview_scrub_position = None;
        self.banner = None;
        self.show_cancel_all_confirm = false;
    }

    fn remove_item(&mut self, id: u64) {
        self.queue.retain(|item| item.id != id);
        self.pending_probes.retain(|probe| probe.id != id);
        self.pending_compression_ids
            .retain(|pending_id| *pending_id != id);
        if self.selected_id == Some(id) {
            self.selected_id = None;
            self.preview_player.stop();
            self.preview_scrub_position = None;
        }
    }

    fn find_item(&self, id: u64) -> Option<&AudioQueueItem> {
        self.queue.iter().find(|item| item.id == id)
    }

    fn find_item_mut(&mut self, id: u64) -> Option<&mut AudioQueueItem> {
        self.queue.iter_mut().find(|item| item.id == id)
    }
}
