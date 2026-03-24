use std::{collections::HashMap, path::PathBuf, sync::mpsc, thread, time::Duration};

use crate::{
    modules::compress_videos::{
        engine::VideoEngineController,
        models::{
            ProcessingProgress, VideoCompressionState, VideoQueueItem, VideoSettings,
            VideoThumbnail,
        },
        processor::{self, BatchEvent, BatchItem},
    },
    runtime,
};

use super::{BannerMessage, BannerTone, CompressVideosPage, PendingProbe, ProbeResult};

impl Default for CompressVideosPage {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            next_id: 0,
            selected_id: None,
            active_batch: None,
            pending_probes: Vec::new(),
            output_dir: None,
            output_dir_user_set: false,
            last_output_dir: None,
            banner: None,
            thumbnail_textures: HashMap::new(),
        }
    }
}

impl CompressVideosPage {
    /// Returns true while a batch compression job is currently active.
    pub fn is_compressing(&self) -> bool {
        self.active_batch.is_some()
    }

    /// Queues files that were opened externally through the OS shell.
    pub(crate) fn queue_external_paths(
        &mut self,
        paths: Vec<PathBuf>,
        engine: &VideoEngineController,
    ) {
        self.add_paths(paths, engine);
    }

    /// Cancels the active compression batch if one is running.
    pub fn cancel_compression(&self) {
        if let Some(batch) = &self.active_batch {
            batch.cancel();
        }
    }

    /// Polls background probes and batch jobs, then returns a repaint interval while work is active.
    pub fn poll_background(&mut self) -> Option<Duration> {
        self.poll_probes();
        self.poll_batch();

        let busy = !self.pending_probes.is_empty() || self.active_batch.is_some();
        if busy {
            Some(Duration::from_millis(50))
        } else {
            None
        }
    }

    fn poll_probes(&mut self) {
        let mut completed = Vec::new();
        for (idx, probe) in self.pending_probes.iter().enumerate() {
            if let Ok(result) = probe.receiver.try_recv() {
                completed.push((idx, probe.id, probe.encoders.clone(), result));
            }
        }

        for (idx, id, encoders, probe_result) in completed.into_iter().rev() {
            self.pending_probes.remove(idx);
            match probe_result.metadata {
                Ok(metadata) => {
                    let range = processor::size_slider_range(&metadata);
                    let settings = VideoSettings::new(&metadata, &encoders, range);
                    if let Some(item) = self.queue.iter_mut().find(|item| item.id == id) {
                        item.file_name = metadata.file_name.clone();
                        item.metadata = Some(metadata);
                        item.settings = Some(settings);
                        item.state = VideoCompressionState::Ready;
                        item.thumbnail = probe_result.thumbnail;
                    }
                }
                Err(error) => {
                    if let Some(item) = self.queue.iter_mut().find(|item| item.id == id) {
                        item.state = VideoCompressionState::Failed(error);
                    }
                }
            }
        }
    }

    fn poll_batch(&mut self) {
        let mut finished = None;
        if let Some(batch) = &self.active_batch {
            while let Ok(event) = batch.receiver.try_recv() {
                match event {
                    BatchEvent::VideoStarted { id } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.id == id) {
                            item.state = VideoCompressionState::Compressing(ProcessingProgress {
                                progress: 0.02,
                                stage: "Starting".to_owned(),
                                speed_x: 0.0,
                                eta_secs: None,
                            });
                        }
                        if self.selected_id == Some(id) {
                            self.selected_id = None;
                        }
                    }
                    BatchEvent::VideoProgress { id, progress } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.id == id) {
                            item.state = VideoCompressionState::Compressing(progress);
                        }
                    }
                    BatchEvent::VideoFinished { id, result } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.id == id) {
                            item.state = VideoCompressionState::Completed(result);
                        }
                        if self.selected_id == Some(id) {
                            self.selected_id = None;
                        }
                    }
                    BatchEvent::VideoFailed { id, error } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.id == id) {
                            item.state = VideoCompressionState::Failed(error);
                        }
                    }
                    BatchEvent::BatchFinished { cancelled } => {
                        finished = Some(cancelled);
                    }
                }
            }
        }

        if let Some(cancelled) = finished {
            if cancelled {
                for item in &mut self.queue {
                    if matches!(item.state, VideoCompressionState::Ready) {
                        continue;
                    }
                    if matches!(item.state, VideoCompressionState::Compressing(_)) {
                        item.state = VideoCompressionState::Cancelled;
                    }
                }
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Batch cancelled. Finished videos remain in the output folder.".into(),
                });
            } else {
                let completed_count = self
                    .queue
                    .iter()
                    .filter(|item| matches!(item.state, VideoCompressionState::Completed(_)))
                    .count();
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Success,
                    text: format!("Done - {completed_count} video(s) compressed."),
                });
            }
            self.active_batch = None;
        }
    }

    pub(in crate::modules::compress_videos) fn add_paths(
        &mut self,
        paths: Vec<PathBuf>,
        engine: &VideoEngineController,
    ) {
        let Some(engine) = engine.active_info().cloned() else {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Video tools are still being prepared. Please wait.".into(),
            });
            return;
        };

        let had_input = !paths.is_empty();
        let paths =
            runtime::collect_matching_paths(paths, |path| processor::is_supported_video_path(path));
        if paths.is_empty() {
            if had_input {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "No supported video files were found in the dropped items.".into(),
                });
            }
            return;
        }

        let new_paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|path| processor::is_supported_video_path(path))
            .filter(|path| {
                !self.queue.iter().any(|item| {
                    item.source_path == *path
                        && matches!(
                            item.state,
                            VideoCompressionState::Ready | VideoCompressionState::Probing
                        )
                })
            })
            .collect();

        if new_paths.is_empty() {
            return;
        }

        let added_count = new_paths.len();
        for path in new_paths {
            let id = self.next_id;
            self.next_id += 1;

            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("video")
                .to_owned();

            self.queue.push(VideoQueueItem {
                id,
                metadata: None,
                settings: None,
                state: VideoCompressionState::Probing,
                source_path: path.clone(),
                file_name: file_name.clone(),
                thumbnail: None,
            });

            let (tx, rx) = mpsc::channel();
            self.pending_probes.push(PendingProbe {
                id,
                encoders: engine.encoders.clone(),
                receiver: rx,
            });

            let engine_clone = engine.clone();
            thread::spawn(move || {
                let metadata_result = processor::probe_video(&engine_clone, path.clone());
                let thumbnail = if let Ok(ref metadata) = metadata_result {
                    processor::generate_thumbnail(&engine_clone, &path, metadata.duration_secs)
                        .ok()
                        .map(|(rgba, width, height)| VideoThumbnail {
                            rgba,
                            width,
                            height,
                        })
                } else {
                    None
                };

                let _ = tx.send(ProbeResult {
                    metadata: metadata_result,
                    thumbnail,
                });
            });
        }

        self.banner = Some(BannerMessage {
            tone: BannerTone::Info,
            text: format!("Added {added_count} video(s) to queue."),
        });
    }

    pub(in crate::modules::compress_videos) fn start_batch_compression(
        &mut self,
        engine: &VideoEngineController,
    ) {
        let Some(engine) = engine.active_info().cloned() else {
            return;
        };

        let items: Vec<BatchItem> = self
            .queue
            .iter()
            .filter(|item| matches!(item.state, VideoCompressionState::Ready))
            .filter_map(|item| {
                let video = item.metadata.clone()?;
                let settings = item.settings.clone()?;
                Some(BatchItem {
                    id: item.id,
                    video,
                    settings,
                })
            })
            .collect();

        if items.is_empty() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "No videos ready to compress.".into(),
            });
            return;
        }

        match processor::start_video_batch(engine, items, self.output_dir.clone()) {
            Ok(handle) => {
                self.last_output_dir = Some(handle.output_dir.clone());
                self.active_batch = Some(handle);
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Batch compression started.".into(),
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
}
