use std::{collections::HashMap, path::PathBuf, sync::mpsc, thread, time::Duration};

use crate::{
    modules::compress_videos::{
        engine::VideoEngineController,
        models::{
            ProcessingProgress, VideoCompressionState, VideoPreviewState, VideoQueueItem,
            VideoThumbnail,
        },
        processor::{self, BatchEvent, BatchItem},
    },
    runtime,
};

use super::{BannerMessage, BannerTone, CompressVideosPage};

impl Default for CompressVideosPage {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            next_id: 0,
            selected_id: None,
            active_batch: None,
            pending_compression_ids: Vec::new(),
            pending_probes: Vec::new(),
            output_dir: None,
            output_dir_user_set: false,
            last_output_dir: None,
            banner: None,
            show_cancel_all_confirm: false,
            thumbnail_textures: HashMap::new(),
            preview_state: VideoPreviewState::default(),
            preview_texture: None,
            preview_texture_dirty: false,
            running_preview_stream: None,
            file_picker_rx: None,
            output_folder_picker_rx: None,
        }
    }
}

impl CompressVideosPage {
    /// Returns true while a video job is active or queued for automatic processing.
    pub fn is_compressing(&self) -> bool {
        self.active_batch.is_some() || !self.pending_compression_ids.is_empty()
    }

    /// Queues files that were opened externally through the OS shell.
    pub(crate) fn queue_external_paths(
        &mut self,
        paths: Vec<PathBuf>,
        engine: &VideoEngineController,
    ) {
        self.add_paths(paths, engine);
    }

    /// Cancels the active compression job and any queued follow-up jobs.
    pub fn cancel_compression(&mut self) {
        let pending_ids = std::mem::take(&mut self.pending_compression_ids);
        for id in pending_ids {
            if let Some(item) = self.queue.iter_mut().find(|item| item.id == id)
                && matches!(item.state, VideoCompressionState::Ready)
            {
                item.state = VideoCompressionState::Cancelled;
            }
        }

        if let Some(batch) = &self.active_batch {
            batch.cancel();
        }
    }

    /// Cancels only the currently active video without touching queued follow-up jobs.
    pub fn cancel_active_video(&self) {
        if let Some(batch) = &self.active_batch {
            batch.cancel();
        }
    }

    /// Polls background probes and jobs, then requests repaints while work continues.
    pub fn poll_background(
        &mut self,
        engine: &VideoEngineController,
        use_hardware_acceleration: bool,
    ) -> Option<Duration> {
        self.poll_probes();
        self.poll_batch();
        self.poll_preview_stream();
        self.start_next_scheduled_video(engine, use_hardware_acceleration);

        if self.running_preview_stream.is_some() {
            Some(Duration::from_millis(16))
        } else {
            let busy = !self.pending_probes.is_empty()
                || self.active_batch.is_some()
                || !self.pending_compression_ids.is_empty();
            if busy {
                Some(Duration::from_millis(50))
            } else {
                None
            }
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
                    let settings = crate::modules::compress_videos::models::VideoSettings::new(
                        &metadata, &encoders, range,
                    );
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
        let mut batch_item_ids = Vec::new();
        let mut clear_preview_selection = false;
        if let Some(batch) = &self.active_batch {
            batch_item_ids = batch.item_ids.clone();
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
                            clear_preview_selection = true;
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
                            clear_preview_selection = true;
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

        if clear_preview_selection {
            self.reset_preview_state();
        }

        if let Some(cancelled) = finished {
            if cancelled {
                for item in &mut self.queue {
                    if batch_item_ids.contains(&item.id)
                        && matches!(
                            item.state,
                            VideoCompressionState::Ready | VideoCompressionState::Compressing(_)
                        )
                    {
                        item.state = VideoCompressionState::Cancelled;
                    }
                }
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Compression cancelled for the current video.".into(),
                });
            } else if self.pending_compression_ids.is_empty() {
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
            self.pending_probes.push(super::PendingProbe {
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

                let _ = tx.send(super::ProbeResult {
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

    /// Schedules every ready video and starts the next job immediately when idle.
    pub(in crate::modules::compress_videos) fn start_batch_compression(
        &mut self,
        engine: &VideoEngineController,
        use_hardware_acceleration: bool,
    ) {
        let scheduled_count = self.schedule_ready_videos();
        if scheduled_count == 0 && self.active_batch.is_none() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "No videos ready to compress.".into(),
            });
            return;
        }

        self.start_next_scheduled_video(engine, use_hardware_acceleration);

        if scheduled_count > 0 {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: if self.active_batch.is_some() {
                    format!("{scheduled_count} video(s) added to the compression queue.")
                } else {
                    format!("{scheduled_count} video(s) scheduled for compression.")
                },
            });
        }
    }

    /// Schedules a single ready video for compression.
    pub(in crate::modules::compress_videos) fn start_single_compression(
        &mut self,
        id: u64,
        engine: &VideoEngineController,
        use_hardware_acceleration: bool,
    ) {
        if !self.schedule_video(id) {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Video is already queued for compression.".into(),
            });
            return;
        }

        self.start_next_scheduled_video(engine, use_hardware_acceleration);
        self.banner = Some(BannerMessage {
            tone: BannerTone::Info,
            text: "Video added to the compression queue.".into(),
        });
    }

    fn schedule_ready_videos(&mut self) -> usize {
        let ids: Vec<u64> = self
            .queue
            .iter()
            .filter(|item| matches!(item.state, VideoCompressionState::Ready))
            .map(|item| item.id)
            .collect();
        let mut scheduled = 0;
        for id in ids {
            if self.schedule_video(id) {
                scheduled += 1;
            }
        }
        scheduled
    }

    fn schedule_video(&mut self, id: u64) -> bool {
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
            .any(|item| item.id == id && matches!(item.state, VideoCompressionState::Ready))
        {
            self.pending_compression_ids.push(id);
            true
        } else {
            false
        }
    }

    fn start_next_scheduled_video(
        &mut self,
        engine: &VideoEngineController,
        use_hardware_acceleration: bool,
    ) {
        if self.active_batch.is_some() {
            return;
        }

        let Some(mut engine_info) = engine.active_info().cloned() else {
            return;
        };
        engine_info.encoders = engine_info
            .encoders
            .with_hardware_acceleration(use_hardware_acceleration);

        while let Some(id) = self.pending_compression_ids.first().copied() {
            let Some(item) = self
                .queue
                .iter()
                .find(|item| item.id == id && matches!(item.state, VideoCompressionState::Ready))
            else {
                self.pending_compression_ids.remove(0);
                continue;
            };

            let Some(video) = item.metadata.clone() else {
                self.pending_compression_ids.remove(0);
                continue;
            };
            let Some(settings) = item.settings.clone() else {
                self.pending_compression_ids.remove(0);
                continue;
            };

            let batch_item = BatchItem {
                id,
                video,
                settings,
            };
            match processor::start_video_batch(
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

    /// Opens the confirmation dialog for bulk cancellation.
    pub(in crate::modules::compress_videos) fn request_cancel_all(&mut self) {
        self.show_cancel_all_confirm = true;
    }

    /// Confirms bulk cancellation for the active job and any queued jobs.
    pub(in crate::modules::compress_videos) fn confirm_cancel_all(&mut self) {
        self.show_cancel_all_confirm = false;
        let had_active_job = self.active_batch.is_some();
        self.cancel_compression();
        if !had_active_job {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "All queued compression jobs were cancelled.".into(),
            });
        }
    }

    /// Closes the bulk cancellation confirmation dialog without changing job state.
    pub(in crate::modules::compress_videos) fn dismiss_cancel_all(&mut self) {
        self.show_cancel_all_confirm = false;
    }

    /// Returns true when there is an active or queued compression job.
    pub(in crate::modules::compress_videos) fn has_pending_compression(&self) -> bool {
        self.active_batch.is_some() || !self.pending_compression_ids.is_empty()
    }

    /// Returns true when the given queue item is either active or already scheduled.
    pub(in crate::modules::compress_videos) fn is_video_pending_compression(
        &self,
        id: u64,
    ) -> bool {
        self.pending_compression_ids.contains(&id)
            || self
                .active_batch
                .as_ref()
                .map(|batch| batch.item_ids.contains(&id))
                .unwrap_or(false)
    }
}
