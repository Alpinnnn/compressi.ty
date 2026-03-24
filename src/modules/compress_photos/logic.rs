use std::{path::PathBuf, sync::mpsc, time::Duration};

use crate::modules::compress_photos::{
    compressor::{self, CompressionEvent},
    models::{CompressionState, FileProgress, PhotoAsset},
};

use super::{BannerMessage, BannerTone, CompressPhotosPage, FileLoadResult};

impl CompressPhotosPage {
    /// Returns true while a photo compression batch is active.
    pub fn is_compressing(&self) -> bool {
        self.active_batch.is_some()
    }

    /// Queues paths received from OS-level launch integration.
    pub(crate) fn queue_external_paths(&mut self, paths: Vec<PathBuf>) {
        self.add_paths(paths);
    }

    /// Cancels the active compression batch if one is running.
    pub fn cancel_compression(&self) {
        if let Some(batch) = &self.active_batch {
            batch.cancel();
        }
    }

    /// Polls background loaders and compression jobs, then requests repaints while work continues.
    pub fn poll_background(&mut self) -> Option<Duration> {
        let mut finished = None;

        if let Some(batch) = &self.active_batch {
            while let Ok(event) = batch.receiver.try_recv() {
                match event {
                    CompressionEvent::FileStarted { id } => {
                        if let Some(item) = self.files.iter_mut().find(|file| file.asset.id == id) {
                            item.state = CompressionState::Compressing(FileProgress {
                                progress: 0.02,
                                stage: "Queued".to_owned(),
                            });
                        }
                    }
                    CompressionEvent::FileProgress {
                        id,
                        progress,
                        stage,
                    } => {
                        if let Some(item) = self.files.iter_mut().find(|file| file.asset.id == id) {
                            item.state =
                                CompressionState::Compressing(FileProgress { progress, stage });
                        }
                    }
                    CompressionEvent::FileFinished { id, result } => {
                        if let Some(item) = self.files.iter_mut().find(|file| file.asset.id == id) {
                            item.state = CompressionState::Completed(result);
                        }
                    }
                    CompressionEvent::FileFailed { id, error } => {
                        if let Some(item) = self.files.iter_mut().find(|file| file.asset.id == id) {
                            item.state = CompressionState::Failed(error);
                        }
                    }
                    CompressionEvent::BatchFinished { cancelled } => {
                        finished = Some(cancelled);
                    }
                }
            }
        }

        if let Some(cancelled) = finished {
            if cancelled {
                for item in &mut self.files {
                    compressor::mark_cancelled(&mut item.state);
                }
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Batch cancelled. Finished files remain in the output folder.".into(),
                });
            } else {
                let completed = self
                    .files
                    .iter()
                    .filter(|file| matches!(file.state, CompressionState::Completed(_)))
                    .count();
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Success,
                    text: format!("Done - {completed} image(s) compressed."),
                });
            }
            self.active_batch = None;
        }

        if let Some(rx) = &self.file_loader_rx {
            loop {
                match rx.try_recv() {
                    Ok(batch) => {
                        self.pending_add_count = 0;
                        let mut added = 0usize;
                        let mut errors = Vec::new();
                        for result in batch.results {
                            match result {
                                Ok(photo) => {
                                    self.pending_loaded_photos.push(photo);
                                    added += 1;
                                }
                                Err(error) => errors.push(error),
                            }
                        }
                        if !errors.is_empty() {
                            self.banner = Some(BannerMessage {
                                tone: BannerTone::Error,
                                text: errors.join("  "),
                            });
                        } else if added > 0 {
                            self.banner = Some(BannerMessage {
                                tone: BannerTone::Info,
                                text: format!("Added {added} image(s) to the queue."),
                            });
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.pending_add_count = 0;
                        self.file_loader_rx = None;
                        break;
                    }
                }
            }
        }

        if self.active_batch.is_some() || self.file_loader_rx.is_some() {
            Some(Duration::from_millis(50))
        } else {
            None
        }
    }

    pub(in crate::modules::compress_photos) fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let had_input = !paths.is_empty();
        let paths = crate::runtime::collect_matching_paths(paths, |path| {
            crate::modules::compress_photos::models::PhotoFormat::from_path(path).is_some()
        });
        if paths.is_empty() {
            if had_input {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "No supported image files were found in the dropped items.".into(),
                });
            }
            return;
        }

        let new_paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|path| {
                !self.files.iter().any(|item| {
                    item.asset.path == *path && matches!(item.state, CompressionState::Ready)
                })
            })
            .collect();

        if new_paths.is_empty() {
            return;
        }

        let (tx, rx) = mpsc::channel::<FileLoadResult>();
        self.file_loader_rx = Some(rx);
        self.pending_add_count = new_paths.len();

        let mut start_id = self.next_file_id;
        self.next_file_id += new_paths.len() as u64;

        std::thread::spawn(move || {
            let mut results = Vec::with_capacity(new_paths.len());
            for path in new_paths {
                start_id += 1;
                results.push(compressor::load_photo(path, start_id));
            }
            let _ = tx.send(FileLoadResult { results });
        });
    }

    pub(in crate::modules::compress_photos) fn start_compression(&mut self) {
        let ready_assets: Vec<PhotoAsset> = self
            .files
            .iter()
            .filter(|item| matches!(item.state, CompressionState::Ready))
            .map(|item| item.asset.clone())
            .collect();

        match compressor::start_batch(ready_assets, self.settings.clone(), self.output_dir.clone())
        {
            Ok(handle) => {
                self.last_output_dir = Some(handle.output_dir.clone());
                self.active_batch = Some(handle);
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Compression started.".into(),
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
