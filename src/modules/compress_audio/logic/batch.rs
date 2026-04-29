use std::{
    fs,
    path::{Path, PathBuf},
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::Instant,
};

use crate::modules::{
    compress_audio::{
        logic::{
            analysis::build_plan,
            files::build_unique_output_path,
            process::{build_encode_command, run_encode_pass},
        },
        models::{
            AudioCompressionPlan, AudioCompressionResult, AudioCompressionSettings, AudioMetadata,
            AudioProcessingProgress,
        },
    },
    compress_videos::models::EngineInfo,
};

use super::files::resolve_output_dir;

/// Events emitted by the background batch worker.
#[derive(Clone, Debug)]
pub enum AudioBatchEvent {
    ItemStarted {
        id: u64,
    },
    ItemProgress {
        id: u64,
        progress: AudioProcessingProgress,
    },
    ItemFinished {
        id: u64,
        result: AudioCompressionResult,
    },
    ItemSkipped {
        id: u64,
        reason: String,
    },
    ItemFailed {
        id: u64,
        error: String,
    },
    BatchFinished {
        cancelled: bool,
    },
}

/// Handle for a running audio compression batch.
pub struct AudioBatchHandle {
    pub receiver: Receiver<AudioBatchEvent>,
    pub output_dir: PathBuf,
    pub item_ids: Vec<u64>,
    cancel_flag: Arc<AtomicBool>,
    active_child: Arc<Mutex<Option<Child>>>,
}

impl AudioBatchHandle {
    /// Signals that the running audio batch should be cancelled.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        kill_active_child(&self.active_child);
    }
}

impl Drop for AudioBatchHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// A single item to compress in the audio batch.
pub struct AudioBatchItem {
    pub id: u64,
    pub metadata: AudioMetadata,
    pub settings: AudioCompressionSettings,
}

/// Starts sequential audio compression work in a background thread.
pub fn start_audio_batch(
    engine: EngineInfo,
    items: Vec<AudioBatchItem>,
    base_output_dir: Option<PathBuf>,
) -> Result<AudioBatchHandle, String> {
    let output_dir = resolve_output_dir(base_output_dir)?;
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "Could not create output folder {}: {error}",
            output_dir.display()
        )
    })?;

    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let thread_cancel = Arc::clone(&cancel_flag);
    let active_child = Arc::new(Mutex::new(None::<Child>));
    let thread_child = Arc::clone(&active_child);
    let thread_output_dir = output_dir.clone();
    let item_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();

    thread::spawn(move || {
        for item in &items {
            if thread_cancel.load(Ordering::Relaxed) {
                let _ = sender.send(AudioBatchEvent::BatchFinished { cancelled: true });
                return;
            }

            let _ = sender.send(AudioBatchEvent::ItemStarted { id: item.id });
            match compress_single_audio(
                &engine,
                &item.metadata,
                &item.settings,
                &thread_output_dir,
                &thread_cancel,
                &thread_child,
                item.id,
                &sender,
            ) {
                Ok(CompressionDisposition::Completed(result)) => {
                    let _ = sender.send(AudioBatchEvent::ItemFinished {
                        id: item.id,
                        result,
                    });
                }
                Ok(CompressionDisposition::Skipped(reason)) => {
                    let _ = sender.send(AudioBatchEvent::ItemSkipped {
                        id: item.id,
                        reason,
                    });
                }
                Err(error) => {
                    if error == "cancelled" {
                        let _ = sender.send(AudioBatchEvent::BatchFinished { cancelled: true });
                        return;
                    }

                    let _ = sender.send(AudioBatchEvent::ItemFailed { id: item.id, error });
                }
            }
        }

        let _ = sender.send(AudioBatchEvent::BatchFinished { cancelled: false });
    });

    Ok(AudioBatchHandle {
        receiver,
        output_dir,
        item_ids,
        cancel_flag,
        active_child,
    })
}

enum CompressionDisposition {
    Completed(AudioCompressionResult),
    Skipped(String),
}

fn compress_single_audio(
    engine: &EngineInfo,
    metadata: &AudioMetadata,
    settings: &AudioCompressionSettings,
    output_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
    active_child: &Arc<Mutex<Option<Child>>>,
    id: u64,
    batch_sender: &mpsc::Sender<AudioBatchEvent>,
) -> Result<CompressionDisposition, String> {
    let plan = build_plan(metadata, settings, &engine.encoders);
    if plan.should_skip && !settings.convert_format_only {
        return Ok(CompressionDisposition::Skipped(
            plan.skip_reason
                .unwrap_or_else(|| "This file is already compact enough.".to_owned()),
        ));
    }

    let output_path = build_output_path(output_dir, metadata, &plan);
    let (job_sender, job_receiver) = mpsc::channel::<AudioProcessingProgress>();
    let started_at = Instant::now();
    let progress_cancel = Arc::clone(cancel_flag);
    let batch_tx = batch_sender.clone();
    let progress_thread = thread::spawn(move || {
        while let Ok(progress) = job_receiver.recv() {
            if progress_cancel.load(Ordering::Relaxed) {
                break;
            }
            let _ = batch_tx.send(AudioBatchEvent::ItemProgress { id, progress });
        }
    });

    let result = run_encode_pass(
        build_encode_command(&engine.ffmpeg_path, metadata, settings, &plan, &output_path),
        metadata.duration_secs,
        "Compressing audio",
        cancel_flag,
        active_child,
        &job_sender,
    );

    drop(job_sender);
    let _ = progress_thread.join();

    if matches!(result, Err(ref error) if error == "cancelled") {
        let _ = fs::remove_file(&output_path);
        return Err("cancelled".to_owned());
    }
    result?;

    let output_size_bytes = fs::metadata(&output_path)
        .map_err(|error| format!("Could not read compressed file: {error}"))?
        .len();
    let reduction_percent = if metadata.size_bytes == 0 {
        0.0
    } else {
        100.0 - (output_size_bytes as f32 / metadata.size_bytes as f32 * 100.0)
    };

    Ok(CompressionDisposition::Completed(AudioCompressionResult {
        output_path,
        original_size_bytes: metadata.size_bytes,
        output_size_bytes,
        reduction_percent,
        elapsed_secs: started_at.elapsed().as_secs_f32(),
    }))
}

fn kill_active_child(active_child: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut child_slot) = active_child.lock()
        && let Some(child) = child_slot.as_mut()
    {
        let _ = child.kill();
    }
}

fn build_output_path(
    output_dir: &Path,
    metadata: &AudioMetadata,
    plan: &AudioCompressionPlan,
) -> PathBuf {
    build_unique_output_path(
        output_dir,
        &metadata.path,
        "compressed",
        plan.output_format.extension(),
    )
}
