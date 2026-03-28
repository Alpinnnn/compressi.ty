use std::{
    io::{ErrorKind, Read},
    path::PathBuf,
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use crate::modules::compress_videos::{
    engine::VideoEngineController,
    models::{EngineInfo, PreviewFrame, VideoMetadata, VideoPreviewState, VideoThumbnail},
    processor::{self, PreviewStreamConfig},
};

use super::{
    CompressVideosPage,
    preview_runtime::{PreviewStreamEvent, PreviewStreamMode, RunningPreviewStream},
};

impl CompressVideosPage {
    pub(super) fn ensure_preview_for_selection(&mut self, engine: &VideoEngineController) {
        let Some(selected_id) = self.selected_id else {
            if self.preview_state.item_id.is_some() || self.running_preview_stream.is_some() {
                self.reset_preview_state();
            }
            return;
        };

        let Some(metadata) = self
            .queue
            .iter()
            .find(|item| item.id == selected_id)
            .and_then(|item| item.metadata.as_ref())
            .cloned()
        else {
            self.selected_id = None;
            self.reset_preview_state();
            return;
        };

        if self.preview_state.item_id == Some(selected_id) {
            if self.running_preview_stream.is_none()
                && self.preview_state.frame.is_none()
                && !self.preview_state.is_loading
                && self.preview_state.load_error.is_none()
                && let Some(engine_info) = engine.active_info().cloned()
            {
                self.start_preview_stream(
                    selected_id,
                    &metadata,
                    engine_info,
                    0.0,
                    PreviewStreamMode::SingleFrame,
                );
            }
            return;
        }

        self.stop_preview_stream();
        let config = processor::preview_stream_config(&metadata);
        self.preview_texture = None;
        self.preview_texture_dirty = false;
        self.preview_state = VideoPreviewState {
            item_id: Some(selected_id),
            duration_secs: metadata.duration_secs,
            preview_frame_rate: config.frame_rate,
            ..VideoPreviewState::default()
        };

        let Some(engine_info) = engine.active_info().cloned() else {
            return;
        };
        self.start_preview_stream(
            selected_id,
            &metadata,
            engine_info,
            0.0,
            PreviewStreamMode::Playback,
        );
    }

    pub(super) fn toggle_preview_playback(&mut self, engine: &VideoEngineController) {
        if self.preview_state.is_playing {
            self.stop_preview_stream();
            self.preview_state.is_playing = false;
            self.preview_state.is_loading = false;
            return;
        }

        let restart_at_zero = self.is_preview_at_end();
        let start_secs = if restart_at_zero {
            0.0
        } else {
            self.preview_state.current_position_secs
        };
        self.seek_preview(engine, start_secs, true);
    }

    pub(super) fn restart_preview(&mut self, engine: &VideoEngineController) {
        self.seek_preview(engine, 0.0, true);
    }

    pub(super) fn begin_preview_scrub(&mut self) {
        self.preview_state.resume_after_scrub = self.preview_state.is_playing;
        if self.preview_state.is_playing {
            self.stop_preview_stream();
            self.preview_state.is_playing = false;
            self.preview_state.is_loading = false;
        }
        self.preview_state.scrub_position_secs = Some(self.displayed_preview_position_secs());
    }

    pub(super) fn update_preview_scrub(&mut self, position_secs: f32) {
        self.preview_state.scrub_position_secs = Some(self.clamp_preview_position(position_secs));
    }

    pub(super) fn finish_preview_scrub(
        &mut self,
        engine: &VideoEngineController,
        position_secs: f32,
    ) {
        let autoplay = self.preview_state.resume_after_scrub;
        self.preview_state.resume_after_scrub = false;
        self.preview_state.scrub_position_secs = None;
        self.seek_preview(engine, position_secs, autoplay);
    }

    pub(super) fn seek_preview(
        &mut self,
        engine: &VideoEngineController,
        position_secs: f32,
        autoplay: bool,
    ) {
        let Some(item_id) = self.preview_state.item_id else {
            return;
        };
        let Some(metadata) = self
            .queue
            .iter()
            .find(|item| item.id == item_id)
            .and_then(|item| item.metadata.as_ref())
            .cloned()
        else {
            return;
        };
        let Some(engine_info) = engine.active_info().cloned() else {
            return;
        };

        let mode = if autoplay {
            PreviewStreamMode::Playback
        } else {
            PreviewStreamMode::SingleFrame
        };
        self.start_preview_stream(item_id, &metadata, engine_info, position_secs, mode);
    }

    pub(super) fn displayed_preview_position_secs(&self) -> f32 {
        self.preview_state
            .scrub_position_secs
            .unwrap_or(self.preview_state.current_position_secs)
            .clamp(0.0, self.preview_state.duration_secs.max(0.0))
    }

    pub(super) fn is_preview_at_end(&self) -> bool {
        if self.preview_state.duration_secs <= 0.0 {
            return false;
        }

        self.preview_state.current_position_secs
            >= (self.preview_state.duration_secs - self.preview_frame_step_secs()).max(0.0)
    }

    pub(super) fn reset_preview_state(&mut self) {
        self.stop_preview_stream();
        self.preview_state = VideoPreviewState::default();
        self.preview_texture = None;
        self.preview_texture_dirty = false;
    }

    pub(super) fn poll_preview_stream(&mut self) {
        let Some(running) = &self.running_preview_stream else {
            return;
        };

        let mode = running.mode;
        let stream_id = running.id;
        let mut events = Vec::new();
        let mut disconnected = false;
        loop {
            match running.receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        let mut should_clear_stream = disconnected;
        for event in events {
            if self.preview_state.item_id != Some(stream_id) {
                should_clear_stream = true;
                continue;
            }

            if let Some(error) = event.error {
                self.preview_state.is_loading = false;
                self.preview_state.is_playing = false;
                self.preview_state.load_error = Some(error);
                should_clear_stream = true;
                continue;
            }

            if let Some(frame) = event.frame {
                self.preview_state.frame = Some(frame.image);
                self.preview_texture_dirty = true;
                self.preview_state.current_position_secs =
                    self.clamp_preview_position(frame.position_secs);
                self.preview_state.is_loading = false;
                self.preview_state.load_error = None;
            }

            if event.finished {
                self.preview_state.is_loading = false;
                if matches!(mode, PreviewStreamMode::Playback) {
                    self.preview_state.is_playing = false;
                    self.preview_state.current_position_secs = self.preview_state.duration_secs;
                }
                should_clear_stream = true;
            }
        }

        if should_clear_stream {
            self.finish_preview_stream();
        }
    }

    fn start_preview_stream(
        &mut self,
        id: u64,
        metadata: &VideoMetadata,
        engine: EngineInfo,
        start_secs: f32,
        mode: PreviewStreamMode,
    ) {
        let is_new_item = self.preview_state.item_id != Some(id);
        self.stop_preview_stream();

        let config = processor::preview_stream_config(metadata);
        let start_secs = start_secs.clamp(0.0, metadata.duration_secs.max(0.0));
        if is_new_item {
            self.preview_texture = None;
            self.preview_texture_dirty = false;
        }
        self.preview_state = VideoPreviewState {
            item_id: Some(id),
            frame: self.preview_state.frame.clone(),
            duration_secs: metadata.duration_secs,
            current_position_secs: start_secs,
            scrub_position_secs: None,
            preview_frame_rate: config.frame_rate,
            is_loading: true,
            load_error: None,
            is_playing: matches!(mode, PreviewStreamMode::Playback),
            resume_after_scrub: false,
            click_feedback: self.preview_state.click_feedback,
        };

        let path = metadata.path.clone();
        let (tx, rx) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let shared_child = Arc::new(Mutex::new(None));
        let worker_cancel = Arc::clone(&cancel_flag);
        let worker_child = Arc::clone(&shared_child);
        let worker = thread::spawn(move || {
            run_preview_stream(
                engine,
                path,
                start_secs,
                config,
                mode,
                worker_cancel,
                worker_child,
                tx,
            );
        });

        self.running_preview_stream = Some(RunningPreviewStream {
            id,
            mode,
            receiver: rx,
            cancel_flag,
            shared_child,
            worker: Some(worker),
        });
    }

    fn stop_preview_stream(&mut self) {
        self.clear_preview_stream(true);
    }

    fn finish_preview_stream(&mut self) {
        self.clear_preview_stream(false);
    }

    fn clear_preview_stream(&mut self, cancel: bool) {
        let Some(mut running) = self.running_preview_stream.take() else {
            return;
        };

        if cancel {
            running.cancel_flag.store(true, Ordering::Relaxed);
            if let Ok(mut slot) = running.shared_child.lock()
                && let Some(child) = slot.as_mut()
            {
                let _ = child.kill();
            }
        }

        while running.receiver.try_recv().is_ok() {}

        if let Some(worker) = running.worker.take() {
            let _ = worker.join();
        }
    }

    fn clamp_preview_position(&self, position_secs: f32) -> f32 {
        position_secs.clamp(0.0, self.preview_state.duration_secs.max(0.0))
    }

    fn preview_frame_step_secs(&self) -> f32 {
        1.0 / self.preview_state.preview_frame_rate.max(1.0)
    }
}

fn run_preview_stream(
    engine: EngineInfo,
    path: PathBuf,
    start_secs: f32,
    config: PreviewStreamConfig,
    mode: PreviewStreamMode,
    cancel_flag: Arc<AtomicBool>,
    shared_child: Arc<Mutex<Option<Child>>>,
    sender: mpsc::Sender<PreviewStreamEvent>,
) {
    let single_frame = matches!(mode, PreviewStreamMode::SingleFrame);
    let mut command =
        processor::build_preview_stream_command(&engine, &path, start_secs, config, single_frame);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = sender.send(PreviewStreamEvent {
                frame: None,
                finished: false,
                error: Some(format!("Could not start preview stream: {error}")),
            });
            return;
        }
    };

    let Some(mut stdout) = child.stdout.take() else {
        let _ = sender.send(PreviewStreamEvent {
            frame: None,
            finished: false,
            error: Some("Could not read preview frames.".to_owned()),
        });
        return;
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = sender.send(PreviewStreamEvent {
            frame: None,
            finished: false,
            error: Some("Could not read preview stream diagnostics.".to_owned()),
        });
        return;
    };

    if let Ok(mut slot) = shared_child.lock() {
        *slot = Some(child);
    }

    let stderr_handle = thread::spawn(move || read_preview_stderr(stderr));
    let frame_bytes = (config.width as usize)
        .saturating_mul(config.height as usize)
        .saturating_mul(4);
    let playback_started_at = Instant::now();
    let mut frame_index = 0_u64;
    let mut failed = None;

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        let mut rgba = vec![0_u8; frame_bytes];
        match stdout.read_exact(&mut rgba) {
            Ok(()) => {
                let frame = PreviewFrame {
                    image: VideoThumbnail {
                        rgba,
                        width: config.width,
                        height: config.height,
                    },
                    position_secs: start_secs + frame_index as f32 / config.frame_rate.max(1.0),
                };
                let _ = sender.send(PreviewStreamEvent {
                    frame: Some(frame),
                    finished: false,
                    error: None,
                });
                frame_index += 1;

                if !single_frame {
                    let target_elapsed =
                        Duration::from_secs_f32(frame_index as f32 / config.frame_rate.max(1.0));
                    let elapsed = playback_started_at.elapsed();
                    if target_elapsed > elapsed {
                        thread::sleep(target_elapsed - elapsed);
                    }
                }
            }
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => {
                failed = Some(format!("Could not decode preview frame: {error}"));
                break;
            }
        }
    }

    let stderr_output = stderr_handle.join().unwrap_or_default();
    let status = wait_for_preview_child(&shared_child);

    if cancel_flag.load(Ordering::Relaxed) {
        return;
    }

    if let Some(error) = failed {
        let _ = sender.send(PreviewStreamEvent {
            frame: None,
            finished: false,
            error: Some(error),
        });
        return;
    }

    match status {
        Ok(status) if status.success() => {
            let _ = sender.send(PreviewStreamEvent {
                frame: None,
                finished: true,
                error: None,
            });
        }
        Ok(_) => {
            let detail = stderr_output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .last()
                .unwrap_or("Preview stream exited unexpectedly.");
            let _ = sender.send(PreviewStreamEvent {
                frame: None,
                finished: false,
                error: Some(detail.to_owned()),
            });
        }
        Err(error) => {
            let _ = sender.send(PreviewStreamEvent {
                frame: None,
                finished: false,
                error: Some(error),
            });
        }
    }
}

fn wait_for_preview_child(
    shared_child: &Arc<Mutex<Option<Child>>>,
) -> Result<std::process::ExitStatus, String> {
    let mut slot = shared_child
        .lock()
        .map_err(|_| "Could not finalize preview stream.".to_owned())?;
    let mut child = slot
        .take()
        .ok_or_else(|| "Could not finalize preview stream.".to_owned())?;
    child
        .wait()
        .map_err(|error| format!("Could not wait for preview stream: {error}"))
}

fn read_preview_stderr<R: Read>(mut reader: R) -> String {
    let mut buffer = String::new();
    let _ = reader.read_to_string(&mut buffer);
    buffer
}
