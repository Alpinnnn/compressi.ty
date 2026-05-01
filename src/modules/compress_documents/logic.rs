use std::{path::PathBuf, sync::mpsc, time::Duration};

use crate::{modules::compress_documents::models::DocumentCompressionState, runtime};

use super::{
    BannerMessage, BannerTone, CompressDocumentsPage, FileLoadResult,
    models::{DocumentAsset, DocumentQueueItem},
    processor::{self, DocumentBatchEvent},
};

impl CompressDocumentsPage {
    /// Returns true while a document compression batch or file loader is active.
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
                    DocumentBatchEvent::FileStarted { id } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.asset.id == id) {
                            item.state = DocumentCompressionState::Compressing(
                                super::models::DocumentProgress {
                                    progress: 0.02,
                                    stage: "Queued".to_owned(),
                                },
                            );
                        }
                    }
                    DocumentBatchEvent::FileProgress {
                        id,
                        progress,
                        stage,
                    } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.asset.id == id) {
                            item.state = DocumentCompressionState::Compressing(
                                super::models::DocumentProgress { progress, stage },
                            );
                        }
                    }
                    DocumentBatchEvent::FileFinished { id, result } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.asset.id == id) {
                            item.state = DocumentCompressionState::Completed(result);
                        }
                    }
                    DocumentBatchEvent::FileFailed { id, error } => {
                        if let Some(item) = self.queue.iter_mut().find(|item| item.asset.id == id) {
                            item.state = DocumentCompressionState::Failed(error);
                        }
                    }
                    DocumentBatchEvent::BatchFinished { cancelled } => {
                        finished = Some(cancelled);
                    }
                }
            }
        }

        if let Some(cancelled) = finished {
            if cancelled {
                for item in &mut self.queue {
                    processor::mark_cancelled(&mut item.state);
                }
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Document compression cancelled.".to_owned(),
                });
            } else {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Success,
                    text: "Document compression finished.".to_owned(),
                });
            }
            self.active_batch = None;
        }

        if let Some(rx) = &self.file_loader_rx {
            match rx.try_recv() {
                Ok(batch) => {
                    self.file_loader_rx = None;
                    self.pending_add_count = 0;
                    self.insert_loaded_documents(batch);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.file_loader_rx = None;
                    self.pending_add_count = 0;
                    self.banner = Some(BannerMessage {
                        tone: BannerTone::Error,
                        text: "Document loader stopped before it could finish.".to_owned(),
                    });
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if self.active_batch.is_some() || self.file_loader_rx.is_some() {
            Some(Duration::from_millis(50))
        } else {
            None
        }
    }

    pub(in crate::modules::compress_documents) fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let had_input = !paths.is_empty();
        let paths = runtime::collect_matching_paths(paths, processor::is_supported_path);
        if paths.is_empty() {
            if had_input {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "No supported document files were found.".to_owned(),
                });
            }
            return;
        }

        let new_paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|path| !self.queue.iter().any(|item| item.asset.path == *path))
            .collect();

        if new_paths.is_empty() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Those documents are already in the queue.".to_owned(),
            });
            return;
        }

        let (tx, rx) = mpsc::channel::<FileLoadResult>();
        self.file_loader_rx = Some(rx);
        self.pending_add_count = new_paths.len();
        let start_id = self.next_id;
        self.next_id += new_paths.len() as u64;

        std::thread::spawn(move || {
            let results = new_paths
                .into_iter()
                .enumerate()
                .map(|(index, path)| processor::load_document(path, start_id + index as u64 + 1))
                .collect();
            let _ = tx.send(FileLoadResult { results });
        });
    }

    pub(in crate::modules::compress_documents) fn start_all_compression(&mut self) {
        let documents = self
            .queue
            .iter()
            .filter(|item| can_start(&item.state))
            .map(|item| item.asset.clone())
            .collect::<Vec<_>>();
        self.start_compression_for(documents);
    }

    pub(in crate::modules::compress_documents) fn start_single_compression(&mut self, id: u64) {
        let documents = self
            .queue
            .iter()
            .filter(|item| item.asset.id == id && can_start(&item.state))
            .map(|item| item.asset.clone())
            .collect::<Vec<_>>();
        self.start_compression_for(documents);
    }

    pub(in crate::modules::compress_documents) fn remove_document(&mut self, id: u64) {
        self.queue.retain(|item| item.asset.id != id);
        if self.selected_id == Some(id) {
            self.selected_id = self.queue.first().map(|item| item.asset.id);
        }
    }

    pub(in crate::modules::compress_documents) fn clear_finished(&mut self) {
        self.queue.retain(|item| {
            !matches!(
                item.state,
                DocumentCompressionState::Completed(_)
                    | DocumentCompressionState::Failed(_)
                    | DocumentCompressionState::Cancelled
            )
        });
        if let Some(selected_id) = self.selected_id
            && !self.queue.iter().any(|item| item.asset.id == selected_id)
        {
            self.selected_id = self.queue.first().map(|item| item.asset.id);
        }
    }

    pub(in crate::modules::compress_documents) fn has_compressible_documents(&self) -> bool {
        self.queue.iter().any(|item| can_start(&item.state))
    }

    fn insert_loaded_documents(&mut self, batch: FileLoadResult) {
        let mut added = 0usize;
        let mut errors = Vec::new();

        for result in batch.results {
            match result {
                Ok(document) => {
                    let id = document.asset.id;
                    self.queue.push(DocumentQueueItem {
                        asset: document.asset,
                        state: DocumentCompressionState::Ready,
                    });
                    self.selected_id.get_or_insert(id);
                    added += 1;
                }
                Err(error) => errors.push(error),
            }
        }

        self.banner = if errors.is_empty() {
            Some(BannerMessage {
                tone: BannerTone::Success,
                text: format!("{added} document(s) added."),
            })
        } else {
            Some(BannerMessage {
                tone: BannerTone::Error,
                text: format!("{} document(s) added, {} skipped.", added, errors.len()),
            })
        };
    }

    fn start_compression_for(&mut self, documents: Vec<DocumentAsset>) {
        if self.active_batch.is_some() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "A document batch is already running.".to_owned(),
            });
            return;
        }

        match processor::start_batch(documents, self.settings.clone(), self.output_dir.clone()) {
            Ok(handle) => {
                self.last_output_dir = Some(handle.output_dir.clone());
                self.active_batch = Some(handle);
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Document compression started.".to_owned(),
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

fn can_start(state: &DocumentCompressionState) -> bool {
    matches!(
        state,
        DocumentCompressionState::Ready
            | DocumentCompressionState::Failed(_)
            | DocumentCompressionState::Cancelled
    )
}
