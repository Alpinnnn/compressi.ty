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

use crate::modules::compress_videos::models::{
    CompressionResult, EngineInfo, ProcessingProgress, VideoMetadata, VideoSettings,
};

use super::{
    events::EncodeEvent,
    execution::{ProgressWeight, run_encode_pass},
    files::{
        build_output_name, build_unique_output_path, cleanup_passlog, null_output_path,
        resolve_output_dir,
    },
    planning::{build_encode_command, build_plan},
};

/// Events emitted during batch video compression.
#[derive(Clone, Debug)]
pub enum BatchEvent {
    VideoStarted {
        id: u64,
    },
    VideoProgress {
        id: u64,
        progress: ProcessingProgress,
    },
    VideoFinished {
        id: u64,
        result: CompressionResult,
    },
    VideoFailed {
        id: u64,
        error: String,
    },
    BatchFinished {
        cancelled: bool,
    },
}

/// Handle for a running batch video compression job.
pub struct BatchHandle {
    pub receiver: Receiver<BatchEvent>,
    pub output_dir: PathBuf,
    pub item_ids: Vec<u64>,
    cancel_flag: Arc<AtomicBool>,
    active_child: Arc<Mutex<Option<Child>>>,
}

impl BatchHandle {
    /// Signals that the batch should be cancelled.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        kill_active_child(&self.active_child);
    }
}

impl Drop for BatchHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// A single item to compress in a batch.
pub struct BatchItem {
    pub id: u64,
    pub video: VideoMetadata,
    pub settings: VideoSettings,
}

/// Starts sequential compression of multiple videos in a background thread.
pub fn start_video_batch(
    engine: EngineInfo,
    items: Vec<BatchItem>,
    base_output_dir: Option<PathBuf>,
) -> Result<BatchHandle, String> {
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
    let item_ids = items.iter().map(|item| item.id).collect();

    thread::spawn(move || {
        for item in &items {
            if thread_cancel.load(Ordering::Relaxed) {
                let _ = sender.send(BatchEvent::BatchFinished { cancelled: true });
                return;
            }

            let _ = sender.send(BatchEvent::VideoStarted { id: item.id });
            let result = compress_single_video(
                &engine,
                &item.video,
                &item.settings,
                &thread_output_dir,
                &thread_cancel,
                &thread_child,
                item.id,
                &sender,
            );

            match result {
                Ok(compression_result) => {
                    let _ = sender.send(BatchEvent::VideoFinished {
                        id: item.id,
                        result: compression_result,
                    });
                }
                Err(error) => {
                    if error == "cancelled" {
                        let _ = sender.send(BatchEvent::BatchFinished { cancelled: true });
                        return;
                    }
                    let _ = sender.send(BatchEvent::VideoFailed { id: item.id, error });
                }
            }
        }

        let _ = sender.send(BatchEvent::BatchFinished { cancelled: false });
    });

    Ok(BatchHandle {
        receiver,
        output_dir,
        item_ids,
        cancel_flag,
        active_child,
    })
}

fn compress_single_video(
    engine: &EngineInfo,
    video: &VideoMetadata,
    settings: &VideoSettings,
    output_dir: &Path,
    cancel_flag: &Arc<AtomicBool>,
    active_child: &Arc<Mutex<Option<Child>>>,
    id: u64,
    batch_sender: &mpsc::Sender<BatchEvent>,
) -> Result<CompressionResult, String> {
    let output_path = build_unique_output_path(output_dir, &video.path, "compressed", "mp4");
    let plan = build_plan(video, settings, &engine.encoders, false);
    let passlog = output_dir.join(build_output_name(&video.path, "twopass", "log"));

    let (job_sender, job_receiver) = mpsc::channel::<EncodeEvent>();
    let started_at = Instant::now();

    let batch_tx = batch_sender.clone();
    let progress_cancel = Arc::clone(cancel_flag);
    let progress_thread = thread::spawn(move || {
        while let Ok(event) = job_receiver.recv() {
            if progress_cancel.load(Ordering::Relaxed) {
                break;
            }
            let EncodeEvent::Progress(progress) = event;
            let _ = batch_tx.send(BatchEvent::VideoProgress { id, progress });
        }
    });

    if plan.pass_count == 2 {
        let first_result = run_encode_pass(
            build_encode_command(
                &engine.ffmpeg_path,
                video,
                &plan,
                0.0,
                video.duration_secs,
                null_output_path(),
                Some(&passlog),
                true,
                false,
            ),
            video.duration_secs,
            ProgressWeight {
                start: 0.0,
                span: 0.48,
            },
            "Analyzing video",
            cancel_flag,
            active_child,
            &job_sender,
        );

        if matches!(first_result, Err(ref error) if error == "cancelled") {
            cleanup_passlog(&passlog);
            drop(job_sender);
            let _ = progress_thread.join();
            return Err("cancelled".to_owned());
        }

        if let Err(error) = first_result {
            cleanup_passlog(&passlog);
            drop(job_sender);
            let _ = progress_thread.join();
            return Err(error);
        }
    }

    let second_result = run_encode_pass(
        build_encode_command(
            &engine.ffmpeg_path,
            video,
            &plan,
            0.0,
            video.duration_secs,
            &output_path,
            (plan.pass_count == 2).then_some(&passlog),
            false,
            false,
        ),
        video.duration_secs,
        if plan.pass_count == 2 {
            ProgressWeight {
                start: 0.48,
                span: 0.52,
            }
        } else {
            ProgressWeight {
                start: 0.0,
                span: 1.0,
            }
        },
        "Compressing video",
        cancel_flag,
        active_child,
        &job_sender,
    );

    cleanup_passlog(&passlog);
    drop(job_sender);
    let _ = progress_thread.join();

    if matches!(second_result, Err(ref error) if error == "cancelled") {
        return Err("cancelled".to_owned());
    }

    second_result?;

    let output_size_bytes = fs::metadata(&output_path)
        .map_err(|error| format!("Could not read compressed file: {error}"))?
        .len();
    let reduction_percent = if video.size_bytes == 0 {
        0.0
    } else {
        100.0 - (output_size_bytes as f32 / video.size_bytes as f32 * 100.0)
    };

    Ok(CompressionResult {
        output_path,
        original_size_bytes: video.size_bytes,
        output_size_bytes,
        reduction_percent,
        elapsed_secs: started_at.elapsed().as_secs_f32(),
    })
}

fn kill_active_child(active_child: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut child_slot) = active_child.lock()
        && let Some(child) = child_slot.as_mut()
    {
        let _ = child.kill();
    }
}
