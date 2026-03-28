pub mod logic;
pub mod models;
mod ui;

use std::{path::PathBuf, sync::mpsc, time::Duration};

use crate::{
    modules::{
        compress_audio::{
            logic::{AudioBatchEvent, AudioBatchHandle, AudioBatchItem},
            models::{AudioAnalysis, AudioCompressionSettings, AudioMetadata, AudioQueueItem},
        },
        compress_videos::engine::VideoEngineController,
    },
    runtime,
};

use self::{
    logic::{analyze_audio, is_supported_audio_path, probe_audio, start_audio_batch},
    models::{AudioCompressionState, AudioProcessingProgress},
};

/// Audio compression workspace state and queue orchestration.
pub struct CompressAudioPage {
    queue: Vec<AudioQueueItem>,
    next_id: u64,
    selected_id: Option<u64>,
    settings: AudioCompressionSettings,

    active_batch: Option<AudioBatchHandle>,
    pending_probes: Vec<PendingProbe>,
    deferred_paths: Vec<PathBuf>,

    output_dir: Option<PathBuf>,
    output_dir_user_set: bool,
    last_output_dir: Option<PathBuf>,

    banner: Option<BannerMessage>,
}

struct PendingProbe {
    id: u64,
    receiver: mpsc::Receiver<Result<ProbeResult, String>>,
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
            next_id: 1,
            selected_id: None,
            settings: AudioCompressionSettings::default(),
            active_batch: None,
            pending_probes: Vec::new(),
            deferred_paths: Vec::new(),
            output_dir: None,
            output_dir_user_set: false,
            last_output_dir: None,
            banner: None,
        }
    }
}

impl CompressAudioPage {
    pub fn queue_external_paths(
        &mut self,
        paths: Vec<PathBuf>,
        engine: &mut VideoEngineController,
    ) {
        self.add_paths(paths, engine);
    }

    pub fn poll_background(&mut self) -> Option<Duration> {
        let mut probe_updates = Vec::new();
        let mut completed_probes = Vec::new();
        for (index, pending_probe) in self.pending_probes.iter().enumerate() {
            match pending_probe.receiver.try_recv() {
                Ok(result) => {
                    probe_updates.push((pending_probe.id, result));
                    completed_probes.push(index);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    probe_updates.push((
                        pending_probe.id,
                        Err("Audio analysis stopped unexpectedly.".to_owned()),
                    ));
                    completed_probes.push(index);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        for (id, result) in probe_updates {
            match result {
                Ok(probe_result) => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.metadata = Some(probe_result.metadata);
                        item.analysis = Some(probe_result.analysis);
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

        for index in completed_probes.into_iter().rev() {
            self.pending_probes.swap_remove(index);
        }

        let mut batch_finished = None;
        let mut batch_output_dir = None;
        let mut batch_events = Vec::new();
        if let Some(active_batch) = &self.active_batch {
            batch_output_dir = Some(active_batch.output_dir.clone());
            while let Ok(event) = active_batch.receiver.try_recv() {
                batch_events.push(event);
            }
        }

        for event in batch_events {
            match event {
                AudioBatchEvent::ItemStarted { id } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Compressing(AudioProcessingProgress {
                            progress: 0.0,
                            stage: "Starting".to_owned(),
                            speed_x: 0.0,
                            eta_secs: None,
                        });
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
                }
                AudioBatchEvent::ItemSkipped { id, reason } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Skipped(reason);
                    }
                }
                AudioBatchEvent::ItemFailed { id, error } => {
                    if let Some(item) = self.find_item_mut(id) {
                        item.state = AudioCompressionState::Failed(error);
                    }
                }
                AudioBatchEvent::BatchFinished { cancelled } => {
                    batch_finished = Some(cancelled);
                }
            }
        }

        if let Some(cancelled) = batch_finished {
            if cancelled {
                for item in &mut self.queue {
                    if matches!(item.state, AudioCompressionState::Compressing(_)) {
                        item.state = AudioCompressionState::Cancelled;
                    }
                }
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Audio compression cancelled.".to_owned(),
                });
            } else {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Success,
                    text: "Audio compression finished.".to_owned(),
                });
            }

            self.last_output_dir = batch_output_dir;
            self.active_batch = None;
        }

        if self.active_batch.is_some() || !self.pending_probes.is_empty() {
            Some(Duration::from_millis(50))
        } else {
            None
        }
    }

    pub fn is_compressing(&self) -> bool {
        self.active_batch.is_some()
    }

    pub fn cancel_compression(&mut self) {
        if let Some(active_batch) = &self.active_batch {
            active_batch.cancel();
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
            .collect::<std::collections::BTreeSet<_>>();
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
                state: AudioCompressionState::Analyzing,
            });

            let (sender, receiver) = mpsc::channel();
            let probe_engine = engine_info.clone();
            std::thread::spawn(move || {
                let result = probe_audio(&probe_engine, path).map(|metadata| ProbeResult {
                    analysis: analyze_audio(&metadata, &probe_engine.encoders),
                    metadata,
                });
                let _ = sender.send(result);
            });

            self.pending_probes.push(PendingProbe { id, receiver });
            if self.selected_id.is_none() {
                self.selected_id = Some(id);
            }
        }
    }

    fn start_compression(&mut self, engine: &mut VideoEngineController) {
        if self.active_batch.is_some() {
            return;
        }

        let Some(engine_info) = engine.active_info().cloned() else {
            engine.ensure_ready();
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Preparing the FFmpeg engine before compression starts...".to_owned(),
            });
            return;
        };

        let items = self
            .queue
            .iter()
            .filter_map(|item| match (&item.metadata, &item.state) {
                (Some(metadata), AudioCompressionState::Ready)
                | (Some(metadata), AudioCompressionState::Failed(_))
                | (Some(metadata), AudioCompressionState::Skipped(_))
                | (Some(metadata), AudioCompressionState::Completed(_))
                | (Some(metadata), AudioCompressionState::Cancelled) => Some(AudioBatchItem {
                    id: item.id,
                    metadata: metadata.clone(),
                    settings: self.settings.clone(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();

        if items.is_empty() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Error,
                text: "Add at least one analyzed audio file before starting compression."
                    .to_owned(),
            });
            return;
        }

        match start_audio_batch(engine_info, items, self.output_dir.clone()) {
            Ok(batch_handle) => {
                for item in &mut self.queue {
                    if batch_handle.item_ids.contains(&item.id) {
                        item.state = AudioCompressionState::Compressing(AudioProcessingProgress {
                            progress: 0.0,
                            stage: "Queued".to_owned(),
                            speed_x: 0.0,
                            eta_secs: None,
                        });
                    }
                }
                self.last_output_dir = Some(batch_handle.output_dir.clone());
                self.active_batch = Some(batch_handle);
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Audio compression started.".to_owned(),
                });
            }
            Err(error) => {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Error,
                    text: error,
                });
            }
        }
    }

    fn remove_item(&mut self, id: u64) {
        self.queue.retain(|item| item.id != id);
        self.pending_probes.retain(|probe| probe.id != id);
        if self.selected_id == Some(id) {
            self.selected_id = self.queue.first().map(|item| item.id);
        }
    }

    fn clear_finished(&mut self) {
        self.queue.retain(|item| {
            !matches!(
                item.state,
                AudioCompressionState::Completed(_)
                    | AudioCompressionState::Skipped(_)
                    | AudioCompressionState::Cancelled
            )
        });
        if self
            .selected_id
            .is_some_and(|id| self.find_item(id).is_none())
        {
            self.selected_id = self.queue.first().map(|item| item.id);
        }
    }

    fn find_item(&self, id: u64) -> Option<&AudioQueueItem> {
        self.queue.iter().find(|item| item.id == id)
    }

    fn find_item_mut(&mut self, id: u64) -> Option<&mut AudioQueueItem> {
        self.queue.iter_mut().find(|item| item.id == id)
    }
}
