use std::{collections::HashMap, path::PathBuf, sync::mpsc, thread, time::Duration};

use eframe::egui::{
    self, Align, Button, Color32, ColorImage, CornerRadius, DragValue, Id, Layout, Rect, RichText,
    ScrollArea, Sense, Slider, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, pos2, vec2,
};

use crate::{
    icons,
    modules::{
        ModuleKind,
        compress_videos::{
            engine::VideoEngineController,
            models::{
                CodecChoice, CompressionMode, EncoderAvailability, EngineStatus,
                ProcessingProgress, ResolutionChoice, VideoCompressionState, VideoMetadata,
                VideoQueueItem, VideoSettings, VideoThumbnail,
            },
            processor::{self, BatchEvent, BatchHandle, BatchItem},
        },
    },
    runtime,
    settings::AppSettings,
    theme::AppTheme,
    ui::components::panel,
};

// â”€â”€â”€ State â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Video compression workspace â€” queue-based batch system.
pub struct CompressVideosPage {
    queue: Vec<VideoQueueItem>,
    next_id: u64,
    selected_id: Option<u64>,

    active_batch: Option<BatchHandle>,
    pending_probes: Vec<PendingProbe>,

    output_dir: Option<PathBuf>,
    output_dir_user_set: bool,
    last_output_dir: Option<PathBuf>,

    banner: Option<BannerMessage>,

    /// Cached GPU textures keyed by queue item id.
    thumbnail_textures: HashMap<u64, TextureHandle>,
}

struct PendingProbe {
    id: u64,
    encoders: EncoderAvailability,
    receiver: mpsc::Receiver<ProbeResult>,
}

/// Combined result of probing + thumbnail generation.
struct ProbeResult {
    metadata: Result<VideoMetadata, String>,
    thumbnail: Option<VideoThumbnail>,
}

struct BannerMessage {
    tone: BannerTone,
    text: String,
}

enum BannerTone {
    Info,
    Success,
    Error,
}

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

// â”€â”€â”€ Spacing helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn flush(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
}
fn compact(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
}

fn truncate_filename(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        return name.to_owned();
    }
    if let Some(dot_pos) = name.rfind('.') {
        let ext = &name[dot_pos..];
        let stem_budget = max_chars.saturating_sub(ext.len()).saturating_sub(1);
        if stem_budget >= 4 {
            return format!("{}â€¦{}", &name[..stem_budget], ext);
        }
    }
    format!("{}â€¦", &name[..max_chars.saturating_sub(1)])
}

// â”€â”€â”€ Public API + Polling â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn is_video_settings_editable(state: &VideoCompressionState) -> bool {
    !matches!(
        state,
        VideoCompressionState::Compressing(_) | VideoCompressionState::Completed(_)
    )
}
impl CompressVideosPage {
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

    pub fn cancel_compression(&self) {
        if let Some(batch) = &self.active_batch {
            batch.cancel();
        }
    }

    pub fn poll_background(&mut self, ctx: &egui::Context) {
        self.poll_probes();
        self.poll_batch();

        let busy = !self.pending_probes.is_empty() || self.active_batch.is_some();
        if busy {
            ctx.request_repaint_after(Duration::from_millis(50));
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
                    if let Some(item) = self.queue.iter_mut().find(|i| i.id == id) {
                        item.file_name = metadata.file_name.clone();
                        item.metadata = Some(metadata);
                        item.settings = Some(settings);
                        item.state = VideoCompressionState::Ready;
                        item.thumbnail = probe_result.thumbnail;
                    }
                }
                Err(error) => {
                    if let Some(item) = self.queue.iter_mut().find(|i| i.id == id) {
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
                        if let Some(item) = self.queue.iter_mut().find(|i| i.id == id) {
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
                        if let Some(item) = self.queue.iter_mut().find(|i| i.id == id) {
                            item.state = VideoCompressionState::Compressing(progress);
                        }
                    }
                    BatchEvent::VideoFinished { id, result } => {
                        if let Some(item) = self.queue.iter_mut().find(|i| i.id == id) {
                            item.state = VideoCompressionState::Completed(result);
                        }
                        if self.selected_id == Some(id) {
                            self.selected_id = None;
                        }
                    }
                    BatchEvent::VideoFailed { id, error } => {
                        if let Some(item) = self.queue.iter_mut().find(|i| i.id == id) {
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
                        // keep as ready
                    } else if matches!(item.state, VideoCompressionState::Compressing(_)) {
                        item.state = VideoCompressionState::Cancelled;
                    }
                }
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Batch cancelled. Finished videos remain in the output folder.".into(),
                });
            } else {
                let n = self
                    .queue
                    .iter()
                    .filter(|i| matches!(i.state, VideoCompressionState::Completed(_)))
                    .count();
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Success,
                    text: format!("Done â€” {n} video(s) compressed."),
                });
            }
            self.active_batch = None;
        }
    }

    // â”€â”€â”€ File I/O â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn pick_videos(&mut self, engine: &VideoEngineController) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Videos", &["mp4", "mov", "mkv", "webm", "avi", "m4v"])
            .pick_files()
        {
            self.add_paths(paths, engine);
        }
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>, engine: &VideoEngineController) {
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
            .filter(|p| processor::is_supported_video_path(p))
            .filter(|p| {
                !self.queue.iter().any(|i| {
                    i.source_path == *p
                        && matches!(
                            i.state,
                            VideoCompressionState::Ready | VideoCompressionState::Probing
                        )
                })
            })
            .collect();

        if new_paths.is_empty() {
            return;
        }

        for path in new_paths {
            let id = self.next_id;
            self.next_id += 1;
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
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
                // Also generate a thumbnail if probe succeeded
                let thumbnail = if let Ok(ref meta) = metadata_result {
                    processor::generate_thumbnail(&engine_clone, &path, meta.duration_secs)
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
            text: format!("Added {} video(s) to queue.", self.queue.len()),
        });
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context, engine: &VideoEngineController) {
        let paths = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        if !paths.is_empty() {
            self.add_paths(paths, engine);
        }
    }

    fn start_batch_compression(&mut self, engine: &VideoEngineController) {
        let Some(engine) = engine.active_info().cloned() else {
            return;
        };

        let items: Vec<BatchItem> = self
            .queue
            .iter()
            .filter(|i| matches!(i.state, VideoCompressionState::Ready))
            .filter_map(|i| {
                let video = i.metadata.clone()?;
                let settings = i.settings.clone()?;
                Some(BatchItem {
                    id: i.id,
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
            Err(e) => {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Error,
                    text: e,
                });
            }
        }
    }

    // â”€â”€â”€ Root layout â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        app_settings: &AppSettings,
        engine: &mut VideoEngineController,
    ) {
        engine.ensure_ready();
        if !self.output_dir_user_set {
            self.output_dir = app_settings.preferred_video_output_folder();
        }
        self.handle_dropped_files(ctx, engine);
        flush(ui);

        let panel_rect = ui.max_rect();
        let avail = panel_rect.width();
        let page_margin = if avail >= 1280.0 {
            28.0
        } else if avail >= 960.0 {
            22.0
        } else if avail >= 720.0 {
            16.0
        } else {
            12.0
        };
        let content_w = (avail - page_margin * 2.0).max(0.0);
        let bottom_pad = page_margin;

        let content_rect = Rect::from_min_size(
            panel_rect.min + vec2(page_margin, 0.0),
            vec2(content_w, (panel_rect.height() - bottom_pad).max(0.0)),
        );

        let mut content_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        flush(&mut content_ui);

        self.render_toolbar(&mut content_ui, theme, active_module);
        content_ui.add_space(16.0);

        if let Some(msg) = &self.banner {
            render_banner(&mut content_ui, theme, msg);
            content_ui.add_space(14.0);
        }

        // Engine status bar
        if !matches!(engine.status(), EngineStatus::Ready(_)) {
            self.render_engine_status(&mut content_ui, theme, engine);
            content_ui.add_space(12.0);
        }

        let workspace_w = content_ui.available_width();
        let workspace_h = content_ui.available_height().max(0.0);
        let has_files = !self.queue.is_empty();

        if has_files && workspace_w >= 900.0 {
            // 3-column layout: Queue | Drop+Info | Settings
            content_ui.allocate_ui_with_layout(
                vec2(workspace_w, workspace_h),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let usable_w = (workspace_w - gutter * 2.0).max(0.0);
                    let queue_w = usable_w * 0.28;
                    let center_w = usable_w * 0.38;

                    ui.allocate_ui_with_layout(
                        vec2(queue_w, workspace_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_queue(ui, theme, workspace_h);
                        },
                    );
                    ui.add_space(gutter);
                    ui.allocate_ui_with_layout(
                        vec2(center_w, workspace_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_drop_zone(ui, ctx, theme, workspace_h * 0.45, engine);
                            ui.add_space(12.0);
                            self.render_actions(
                                ui,
                                theme,
                                (workspace_h * 0.55 - 12.0).max(0.0),
                                engine,
                            );
                        },
                    );
                    ui.add_space(gutter);
                    let settings_w = ui.available_width();
                    ui.allocate_ui_with_layout(
                        vec2(settings_w, workspace_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_settings_panel(ui, theme, workspace_h, engine);
                        },
                    );
                },
            );
        } else {
            // Stacked layout
            let drop_h = if has_files {
                workspace_h * 0.22
            } else {
                workspace_h * 0.45
            };
            self.render_drop_zone(&mut content_ui, ctx, theme, drop_h.max(0.0), engine);
            if has_files {
                content_ui.add_space(12.0);
                let remaining = (workspace_h - drop_h - 12.0).max(0.0);
                let queue_h = remaining * 0.35;
                let settings_h = remaining * 0.40;
                let actions_h = remaining * 0.25 - 24.0;
                self.render_queue(&mut content_ui, theme, queue_h);
                content_ui.add_space(12.0);
                self.render_settings_panel(&mut content_ui, theme, settings_h, engine);
                content_ui.add_space(12.0);
                self.render_actions(&mut content_ui, theme, actions_h.max(0.0), engine);
            }
        }
    }

    // â”€â”€â”€ Toolbar â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn render_toolbar(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
    ) {
        let output_dir = self.last_output_dir.clone();
        panel::card(theme)
            .inner_margin(egui::Margin::symmetric(20, 12))
            .show(ui, |ui| {
                compact(ui);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            Button::new(
                                RichText::new(format!("{} Back", icons::BACK))
                                    .size(13.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        *active_module = None;
                    }
                    ui.label(
                        RichText::new("Compress Videos")
                            .size(20.0)
                            .strong()
                            .color(theme.colors.fg),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("{} Change Output", icons::FOLDER))
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::BLACK),
                                )
                                .fill(theme.colors.accent)
                                .stroke(Stroke::NONE)
                                .corner_radius(CornerRadius::ZERO),
                            )
                            .clicked()
                        {
                            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                self.output_dir = Some(dir);
                                self.output_dir_user_set = true;
                            }
                        }
                        if let Some(dir) = &output_dir {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(format!("{} Open Output", icons::FOLDER))
                                            .size(13.0)
                                            .color(theme.colors.fg),
                                    )
                                    .fill(theme.colors.bg_raised)
                                    .stroke(Stroke::new(1.0, theme.colors.border))
                                    .corner_radius(CornerRadius::ZERO),
                                )
                                .clicked()
                            {
                                let _ = open::that(dir);
                            }
                        }
                    });
                });
            });
    }

    // â”€â”€â”€ Engine status â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn render_engine_status(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        match engine.status().clone() {
            EngineStatus::Checking => {
                panel::tinted(theme, theme.colors.accent).show(ui, |ui| {
                    ui.label(RichText::new("Preparing video toolsâ€¦").size(13.0).strong().color(theme.colors.fg));
                    ui.label(RichText::new("The bundled engine is being detected or a managed update is being prepared.").size(11.5).color(theme.colors.fg_dim));
                });
            }
            EngineStatus::Downloading { progress, stage } => {
                panel::tinted(theme, theme.colors.accent).show(ui, |ui| {
                    render_simple_bar(ui, theme, progress, &stage);
                });
            }
            EngineStatus::Ready(_) => {}
            EngineStatus::Failed(error) => {
                panel::tinted(theme, theme.colors.negative).show(ui, |ui| {
                    ui.label(
                        RichText::new("Video tools could not be prepared")
                            .size(13.0)
                            .strong()
                            .color(theme.colors.fg),
                    );
                    ui.label(RichText::new(&error).size(11.5).color(theme.colors.fg_dim));
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if secondary_button(ui, theme, "Retry Setup").clicked() {
                            engine.ensure_ready();
                        }
                        if secondary_button(ui, theme, "Refresh Engine").clicked() {
                            engine.refresh();
                        }
                    });
                });
            }
        }
    }

    // â”€â”€â”€ Drop zone â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
    ) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let has_files = !self.queue.is_empty();
        let accent = theme.colors.accent;
        let fill = if hovering {
            theme.mix(theme.colors.bg_raised, accent, 0.06)
        } else {
            theme.colors.surface
        };
        let stroke = Stroke::new(
            1.0,
            if hovering {
                theme.mix(theme.colors.border_focus, accent, 0.2)
            } else {
                theme.colors.border
            },
        );
        let ready = matches!(engine.status(), EngineStatus::Ready(_));

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), height.max(0.0)),
            Layout::top_down(Align::Min),
            |ui| {
                panel::card(theme)
                    .fill(fill)
                    .stroke(stroke)
                    .inner_margin(egui::Margin::same(18))
                    .show(ui, |ui| {
                        compact(ui);
                        ui.set_min_height((height - 36.0).max(0.0));
                        let content_offset = if has_files { 60.0 } else { 90.0 };
                        ui.add_space(((ui.available_height() - content_offset) * 0.5).max(8.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if has_files {
                                    let ready_count = self
                                        .queue
                                        .iter()
                                        .filter(|i| matches!(i.state, VideoCompressionState::Ready))
                                        .count();
                                    format!(
                                        "{} video(s) ready. Drop more videos or folders here.",
                                        ready_count
                                    )
                                } else {
                                    "Drop videos or folders here to start your workspace".to_owned()
                                })
                                .size(if has_files { 13.0 } else { 16.0 })
                                .strong()
                                .color(theme.colors.fg),
                            );
                            ui.add_space(8.0);
                            if ui
                                .add_enabled(
                                    ready,
                                    Button::new(
                                        RichText::new(format!(
                                            "{} {}",
                                            icons::VIDEO,
                                            if has_files {
                                                "Add More Videos"
                                            } else {
                                                "Select Videos"
                                            }
                                        ))
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::BLACK),
                                    )
                                    .fill(accent)
                                    .stroke(Stroke::NONE)
                                    .corner_radius(CornerRadius::ZERO),
                                )
                                .clicked()
                            {
                                self.pick_videos(engine);
                            }
                            if !ready {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Video tools are being preparedâ€¦")
                                        .size(11.0)
                                        .color(theme.colors.fg_dim),
                                );
                            }
                            // Probing status
                            if !self.pending_probes.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(format!(
                                        "Probing {} video(s)â€¦",
                                        self.pending_probes.len()
                                    ))
                                    .size(11.0)
                                    .color(theme.colors.fg_dim),
                                );
                            }
                        });
                    });
            },
        );
    }

    // â”€â”€â”€ Queue â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn render_queue(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        if self.queue.is_empty() {
            return;
        }

        // Lazily create GPU textures from raw thumbnail data
        for item in &self.queue {
            if let Some(thumb) = &item.thumbnail {
                if !self.thumbnail_textures.contains_key(&item.id) {
                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [thumb.width as usize, thumb.height as usize],
                        &thumb.rgba,
                    );
                    let tex = ui.ctx().load_texture(
                        format!("video-thumb-{}", item.id),
                        color_image,
                        TextureOptions::LINEAR,
                    );
                    self.thumbnail_textures.insert(item.id, tex);
                }
            }
        }

        let mut clicked_id: Option<u64> = None;
        let mut delete_id: Option<u64> = None;
        let mut locked_settings_click = false;
        let is_compressing = self.active_batch.is_some();

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));
                let scroll_h = (height - 42.0).max(0.0);
                ScrollArea::vertical()
                    .id_salt("video_queue_scroll")
                    .auto_shrink([false, false])
                    .max_height(scroll_h)
                    .show(ui, |ui| {
                        flush(ui);
                        ui.set_width(ui.available_width());

                        let categories: [(
                            &str,
                            Color32,
                            Box<dyn Fn(&VideoCompressionState) -> bool>,
                        ); 4] = [
                            (
                                "Probing",
                                theme.colors.fg_muted,
                                Box::new(|s| matches!(s, VideoCompressionState::Probing)),
                            ),
                            (
                                "Queue",
                                theme.colors.fg_muted,
                                Box::new(|s| matches!(s, VideoCompressionState::Ready)),
                            ),
                            (
                                "Progress",
                                theme.colors.accent,
                                Box::new(|s| matches!(s, VideoCompressionState::Compressing(_))),
                            ),
                            (
                                "Done",
                                theme.colors.positive,
                                Box::new(|s| {
                                    matches!(
                                        s,
                                        VideoCompressionState::Completed(_)
                                            | VideoCompressionState::Failed(_)
                                            | VideoCompressionState::Cancelled
                                    )
                                }),
                            ),
                        ];

                        for (title, tint, filter) in &categories {
                            let indices: Vec<usize> = self
                                .queue
                                .iter()
                                .enumerate()
                                .filter(|(_, i)| filter(&i.state))
                                .map(|(idx, _)| idx)
                                .collect();
                            if indices.is_empty() {
                                continue;
                            }

                            queue_section_header(ui, theme, title, indices.len(), *tint);
                            for &idx in &indices {
                                let item = &self.queue[idx];
                                let settings_editable = is_video_settings_editable(&item.state);
                                let selected =
                                    settings_editable && self.selected_id == Some(item.id);
                                let can_delete = !is_compressing
                                    && matches!(
                                        item.state,
                                        VideoCompressionState::Ready
                                            | VideoCompressionState::Failed(_)
                                            | VideoCompressionState::Cancelled
                                    );
                                let thumb_tex = self.thumbnail_textures.get(&item.id);
                                let action = video_queue_row(
                                    ui, theme, item, selected, can_delete, thumb_tex,
                                );
                                if action.clicked && settings_editable {
                                    clicked_id = Some(item.id);
                                } else if action.clicked {
                                    locked_settings_click = true;
                                }
                                if action.deleted {
                                    delete_id = Some(item.id);
                                }
                            }
                            ui.add_space(8.0);
                        }
                    });
            });

        if let Some(id) = delete_id {
            self.queue.retain(|i| i.id != id);
            self.thumbnail_textures.remove(&id);
            if self.selected_id == Some(id) {
                self.selected_id = None;
            }
        }
        if let Some(id) = clicked_id {
            self.selected_id = if self.selected_id == Some(id) {
                None
            } else {
                Some(id)
            };
        }
        if locked_settings_click {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Settings are only available for videos that are still editable.".into(),
            });
        }
    }

    // â”€â”€â”€ Settings panel â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn render_settings_panel(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
    ) {
        panel::card(theme).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            compact(ui);
            ui.set_min_height((height - 28.0).max(0.0));

            let sel_id = self.selected_id;
            let has_selection = sel_id.is_some() && self.queue.iter().any(|i| i.id == sel_id.unwrap_or(0) && i.settings.is_some());

            if !has_selection {
                ui.label(RichText::new("Settings").size(14.0).strong().color(theme.colors.fg));
                ui.add_space(8.0);
                ui.label(RichText::new("Select a video from the queue to configure its compression settings individually.")
                    .size(12.0).color(theme.colors.fg_dim));
                return;
            }

            let id = sel_id.unwrap_or(0);
            let item = self.queue.iter().find(|i| i.id == id).cloned();
            let Some(item) = item else { return; };
            if !is_video_settings_editable(&item.state) {
                self.selected_id = None;
                ui.label(RichText::new("Settings").size(14.0).strong().color(theme.colors.fg));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Settings are locked while a video is compressing or after it has finished.",
                    )
                    .size(12.0)
                    .color(theme.colors.fg_dim),
                );
                return;
            }
            let Some(metadata) = item.metadata.clone() else { return; };
            let Some(mut settings) = item.settings.clone() else { return; };
            let encoders = engine
                .active_info()
                .map(|info| info.encoders.clone())
                .unwrap_or_default();

            ui.label(RichText::new(format!("Settings â€” {}", truncate_filename(&item.file_name, 24))).size(14.0).strong().color(theme.colors.fg));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} | {} | {}Ã—{}", format_bytes(metadata.size_bytes), format_duration(metadata.duration_secs), metadata.width, metadata.height))
                .size(11.0).color(theme.colors.fg_dim));
            ui.add_space(8.0);

            ScrollArea::vertical().id_salt("video_settings_scroll").auto_shrink([false, false]).max_height((height - 100.0).max(0.0)).show(ui, |ui| {
                compact(ui);

                // Mode selector â€” card style (matching compress_photos design)
                for mode in CompressionMode::ALL {
                    if mode_card(ui, theme, mode, settings.mode == mode).clicked() {
                        settings.mode = mode;
                    }
                }
                ui.add_space(8.0);

                let slider_range = processor::size_slider_range(&metadata);
                match settings.mode {
                    CompressionMode::ReduceSize => {
                        ui.label(RichText::new("Target size (MB)").size(12.0).color(theme.colors.fg_dim));
                        ui.add(Slider::new(&mut settings.target_size_mb, slider_range.min_mb..=slider_range.max_mb).suffix(" MB").show_value(true));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Direct input:").size(11.0).color(theme.colors.fg_dim));
                            ui.add(DragValue::new(&mut settings.target_size_mb).range(slider_range.min_mb..=slider_range.max_mb).speed(1.0).suffix(" MB"));
                        });
                    }
                    CompressionMode::GoodQuality => {
                        ui.label(RichText::new("Quality").size(12.0).color(theme.colors.fg_dim));
                        ui.add(Slider::new(&mut settings.quality, 20..=95).show_value(true));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Resolution").size(12.0).color(theme.colors.fg_dim));
                        ui.horizontal_wrapped(|ui| {
                            for choice in ResolutionChoice::QUICK {
                                if choice_button(ui, theme, choice.label(), settings.resolution == choice).clicked() {
                                    settings.resolution = choice;
                                }
                            }
                        });
                    }
                    CompressionMode::CustomAdvanced => {
                        let source_video_kbps = metadata
                            .video_bitrate_kbps
                            .or(metadata.container_bitrate_kbps)
                            .unwrap_or(settings.custom_bitrate_kbps)
                            .clamp(350, 80_000);
                        let source_fps = metadata.fps.round().clamp(12.0, 120.0) as u32;

                        ui.label(
                            RichText::new("Fine-tune bitrate, codec, resolution, frame rate, and audio. Compatible GPU encoders are used automatically when available.")
                                .size(11.5)
                                .color(theme.colors.fg_dim),
                        );
                        ui.add_space(8.0);

                        ui.label(RichText::new("Codec").size(12.0).color(theme.colors.fg_dim));
                        ui.horizontal_wrapped(|ui| {
                            for codec in CodecChoice::ALL {
                                let enabled = encoders.supports(codec);
                                if advanced_codec_button(
                                    ui,
                                    theme,
                                    codec,
                                    settings.custom_codec == codec,
                                    enabled,
                                    &encoders,
                                )
                                .clicked()
                                {
                                    settings.custom_codec = codec;
                                }
                            }
                        });

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Video Bitrate")
                                .size(12.0)
                                .color(theme.colors.fg_dim),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Source video: {}. Lower values shrink file size faster.",
                                format_kbps(source_video_kbps)
                            ))
                            .size(10.5)
                            .color(theme.colors.fg_muted),
                        );
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            for (label, target) in advanced_bitrate_presets(&metadata, settings.custom_codec) {
                                if choice_button(
                                    ui,
                                    theme,
                                    label,
                                    roughly_matches_value(settings.custom_bitrate_kbps, target),
                                )
                                .clicked()
                                {
                                    settings.custom_bitrate_kbps = target;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        ui.add(
                            Slider::new(&mut settings.custom_bitrate_kbps, 350..=80_000)
                                .logarithmic(true)
                                .suffix(" kbps")
                                .show_value(true),
                        );
                        ui.add(
                            DragValue::new(&mut settings.custom_bitrate_kbps)
                                .range(350..=80_000)
                                .speed(50.0)
                                .suffix(" kbps"),
                        );

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Resolution")
                                .size(12.0)
                                .color(theme.colors.fg_dim),
                        );
                        ui.label(
                            RichText::new("Reduce resolution when size matters more than detail.")
                                .size(10.5)
                                .color(theme.colors.fg_muted),
                        );
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            for choice in ResolutionChoice::ADVANCED {
                                if choice_button(ui, theme, choice.label(), settings.resolution == choice)
                                    .clicked()
                                {
                                    settings.resolution = choice;
                                }
                            }
                        });

                        ui.add_space(8.0);
                        ui.label(RichText::new("Frame Rate").size(12.0).color(theme.colors.fg_dim));
                        ui.label(
                            RichText::new("Lower FPS can help screen recordings and talking-head videos compress smaller.")
                                .size(10.5)
                                .color(theme.colors.fg_muted),
                        );
                        ui.add_space(4.0);
                        let mut fps_choices = Vec::new();
                        for fps in [source_fps, 60, 30, 24] {
                            let clamped = fps.min(source_fps.max(12));
                            if !fps_choices.contains(&clamped) {
                                fps_choices.push(clamped);
                            }
                        }
                        ui.horizontal_wrapped(|ui| {
                            for fps in &fps_choices {
                                let label = if *fps == source_fps {
                                    format!("Source ({source_fps})")
                                } else {
                                    format!("{fps} FPS")
                                };
                                if choice_button(ui, theme, &label, settings.custom_fps == *fps)
                                    .clicked()
                                {
                                    settings.custom_fps = *fps;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        ui.add(
                            DragValue::new(&mut settings.custom_fps)
                                .range(12..=metadata.fps.round().max(12.0) as u32)
                                .speed(1.0)
                                .suffix(" fps"),
                        );

                        ui.add_space(8.0);
                        ui.label(RichText::new("Audio").size(12.0).color(theme.colors.fg_dim));
                        if metadata.has_audio {
                            ui.checkbox(&mut settings.custom_audio_enabled, "Keep audio track");
                            if settings.custom_audio_enabled {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    for kbps in [64_u32, 96, 128, 160, 192] {
                                        let label = format!("{kbps} kbps");
                                        if choice_button(
                                            ui,
                                            theme,
                                            &label,
                                            settings.custom_audio_bitrate_kbps == kbps,
                                        )
                                        .clicked()
                                        {
                                            settings.custom_audio_bitrate_kbps = kbps;
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                                ui.add(
                                    Slider::new(&mut settings.custom_audio_bitrate_kbps, 64..=320)
                                        .suffix(" kbps")
                                        .show_value(true),
                                );
                            }
                        } else {
                            ui.label(
                                RichText::new("This source video has no audio track.")
                                    .size(10.5)
                                    .color(theme.colors.fg_muted),
                            );
                        }
                    }
                }

                // Estimate â€” displays positive savings or size increase
                let estimate = processor::estimate_output(&metadata, &settings, &encoders);
                ui.add_space(8.0);
                let (estimate_text, estimate_color) = if estimate.savings_percent >= 0.0 {
                    (
                        format!("Est. {} ({:.0}% smaller)", format_bytes(estimate.estimated_size_bytes), estimate.savings_percent),
                        theme.colors.positive,
                    )
                } else {
                    (
                        format!("Est. {} ({:.0}% larger)", format_bytes(estimate.estimated_size_bytes), estimate.savings_percent.abs()),
                        theme.colors.caution,
                    )
                };
                ui.label(RichText::new(estimate_text).size(11.5).color(estimate_color));

                // Apply to all button
                ui.add_space(8.0);
                if ui.add(Button::new(RichText::new("Apply Settings to All Ready Videos").size(11.5).color(theme.colors.fg))
                    .fill(theme.colors.bg_raised).stroke(Stroke::new(1.0, theme.colors.border)).corner_radius(CornerRadius::ZERO)
                ).clicked() {
                    let new_settings = settings.clone();
                    for queue_item in &mut self.queue {
                        if matches!(queue_item.state, VideoCompressionState::Ready) {
                            if let Some(meta) = &queue_item.metadata {
                                let mut applied = new_settings.clone();
                                // Adjust target_size_mb per video
                                let range = processor::size_slider_range(meta);
                                applied.target_size_mb = applied.target_size_mb.clamp(range.min_mb, range.max_mb);
                                applied.custom_fps = applied.custom_fps.min(meta.fps.round().max(12.0) as u32);
                                queue_item.settings = Some(applied);
                            }
                        }
                    }
                    self.banner = Some(BannerMessage { tone: BannerTone::Info, text: "Settings applied to all ready videos.".into() });
                }
            });

            // Persist settings back
            if let Some(queue_item) = self.queue.iter_mut().find(|i| i.id == id) {
                queue_item.settings = Some(settings);
            }
        });
    }

    // â”€â”€â”€ Actions panel â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn render_actions(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
    ) {
        let accent = theme.colors.accent;
        let can_go = !self.queue.is_empty()
            && self.active_batch.is_none()
            && self
                .queue
                .iter()
                .any(|i| matches!(i.state, VideoCompressionState::Ready));

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                if ui
                    .add_enabled(
                        can_go,
                        Button::new(
                            RichText::new(format!("{} Compress All", icons::PLAY))
                                .size(13.0)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(accent)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::ZERO)
                        .min_size(vec2(ui.available_width(), 34.0)),
                    )
                    .clicked()
                {
                    self.start_batch_compression(engine);
                }

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.active_batch.is_some(),
                            Button::new(RichText::new("Cancel").size(12.0).color(theme.colors.fg))
                                .fill(theme.mix(theme.colors.surface, theme.colors.caution, 0.1))
                                .stroke(Stroke::new(
                                    1.0,
                                    theme.mix(theme.colors.border, theme.colors.caution, 0.24),
                                ))
                                .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        if let Some(batch) = &self.active_batch {
                            batch.cancel();
                        }
                    }
                    if ui
                        .add_enabled(
                            self.active_batch.is_none() && !self.queue.is_empty(),
                            Button::new(
                                RichText::new("Clear All").size(12.0).color(theme.colors.fg),
                            )
                            .fill(theme.mix(theme.colors.surface, theme.colors.negative, 0.08))
                            .stroke(Stroke::new(
                                1.0,
                                theme.mix(theme.colors.border, theme.colors.negative, 0.2),
                            ))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        self.queue.clear();
                        self.selected_id = None;
                        self.banner = None;
                    }
                });

                if self.active_batch.is_some() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(
                            RichText::new("Compressingâ€¦")
                                .size(11.0)
                                .color(theme.colors.fg_dim),
                        );
                    });
                }
            });
    }
}

// â”€â”€â”€ Free functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn render_banner(ui: &mut Ui, theme: &AppTheme, banner: &BannerMessage) {
    let tint = match banner.tone {
        BannerTone::Info => theme.colors.accent,
        BannerTone::Success => theme.colors.positive,
        BannerTone::Error => theme.colors.negative,
    };
    panel::tinted(theme, tint)
        .inner_margin(egui::Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.label(
                RichText::new(&banner.text)
                    .size(12.5)
                    .color(theme.colors.fg),
            );
        });
}

fn queue_section_header(ui: &mut Ui, theme: &AppTheme, title: &str, count: usize, tint: Color32) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{title} â€” {count}"))
                .size(12.0)
                .strong()
                .color(tint),
        );
    });
    let w = ui.available_width();
    let (line_rect, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().rect_filled(
        line_rect,
        CornerRadius::ZERO,
        theme.mix(theme.colors.border, tint, 0.30),
    );
    ui.add_space(4.0);
}

struct QueueRowAction {
    clicked: bool,
    deleted: bool,
}

fn video_queue_row(
    ui: &mut Ui,
    theme: &AppTheme,
    item: &VideoQueueItem,
    selected: bool,
    can_delete: bool,
    thumb_tex: Option<&TextureHandle>,
) -> QueueRowAction {
    let mut action = QueueRowAction {
        clicked: false,
        deleted: false,
    };
    let row_id = Id::new("vq_row").with(item.id);
    let row_width = ui.available_width();
    let row_height = 64.0f32;
    let (row_rect, _) = ui.allocate_exact_size(vec2(row_width, row_height), Sense::hover());
    let row_resp = ui.interact(row_rect, row_id.with("click"), Sense::click());

    let btn_size = vec2(24.0, 24.0);
    let btn_pos = pos2(
        row_rect.right() - btn_size.x - 8.0,
        row_rect.center().y - btn_size.y * 0.5,
    );
    let btn_rect = Rect::from_min_size(btn_pos, btn_size);
    let btn_resp = if can_delete {
        Some(ui.interact(btn_rect, row_id.with("trash"), Sense::click()))
    } else {
        None
    };
    let row_hovered = row_resp.hovered() || btn_resp.as_ref().map(|b| b.hovered()).unwrap_or(false);

    let fill = if selected {
        theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.10)
    } else {
        theme.colors.bg_raised
    };
    ui.painter().rect_filled(row_rect, CornerRadius::ZERO, fill);
    ui.painter().rect_stroke(
        row_rect,
        CornerRadius::ZERO,
        Stroke::new(
            1.0,
            if row_hovered || selected {
                theme.colors.border_focus
            } else {
                theme.colors.border
            },
        ),
        StrokeKind::Middle,
    );

    // Thumbnail on the left (matching compress_photos layout)
    let thumb_size = 42.0;
    let thumb_rect = Rect::from_min_size(
        pos2(row_rect.left() + 10.0, row_rect.top() + 10.0),
        vec2(thumb_size, thumb_size),
    );
    ui.painter()
        .rect_filled(thumb_rect, CornerRadius::ZERO, theme.colors.bg_base);
    ui.painter().rect_stroke(
        thumb_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme.colors.border),
        StrokeKind::Middle,
    );
    if let Some(tex) = thumb_tex {
        let img = thumb_rect.shrink(4.0);
        ui.painter().image(
            tex.id(),
            img,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            thumb_rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::VIDEO,
            icons::font_id(14.0),
            theme.colors.fg_muted,
        );
    }

    let text_x = row_rect.left() + 10.0 + thumb_size + 8.0;
    let text_right = if can_delete && row_hovered {
        btn_rect.left() - 4.0
    } else {
        row_rect.right() - 10.0
    };
    let text_w = (text_right - text_x).max(0.0);
    let mut y = row_rect.top() + 10.0;

    // File name
    let galley = ui.painter().layout_no_wrap(
        truncate_filename(&item.file_name, 28),
        egui::FontId::proportional(12.0),
        theme.colors.fg,
    );
    ui.painter()
        .galley(pos2(text_x, y), galley, theme.colors.fg);
    y += 16.0;

    // State-specific line
    match &item.state {
        VideoCompressionState::Probing => {
            let g = ui.painter().layout_no_wrap(
                "Probingâ€¦".to_owned(),
                egui::FontId::proportional(10.0),
                theme.colors.fg_muted,
            );
            ui.painter()
                .galley(pos2(text_x, y), g, theme.colors.fg_muted);
        }
        VideoCompressionState::Ready => {
            if let Some(meta) = &item.metadata {
                let info = format!(
                    "{} | {}",
                    format_bytes(meta.size_bytes),
                    format_duration(meta.duration_secs)
                );
                let g = ui.painter().layout(
                    info,
                    egui::FontId::proportional(10.0),
                    theme.colors.fg_dim,
                    text_w,
                );
                ui.painter().galley(pos2(text_x, y), g, theme.colors.fg_dim);
            } else {
                let g = ui.painter().layout_no_wrap(
                    "Ready".to_owned(),
                    egui::FontId::proportional(10.0),
                    theme.colors.fg_muted,
                );
                ui.painter()
                    .galley(pos2(text_x, y), g, theme.colors.fg_muted);
            }
        }
        VideoCompressionState::Compressing(p) => {
            let g = ui.painter().layout_no_wrap(
                format!("Compressing {:.0}%", p.progress * 100.0),
                egui::FontId::proportional(10.0),
                theme.colors.accent,
            );
            ui.painter().galley(pos2(text_x, y), g, theme.colors.accent);
            y += 12.0;
            let bar_w = text_w.max(20.0);
            let bar_rect = Rect::from_min_size(pos2(text_x, y), vec2(bar_w, 4.0));
            ui.painter()
                .rect_filled(bar_rect, CornerRadius::same(2), theme.colors.bg_base);
            if p.progress > 0.0 {
                let fill_rect = Rect::from_min_size(
                    bar_rect.min,
                    vec2(bar_rect.width() * p.progress.clamp(0.0, 1.0), 4.0),
                );
                ui.painter()
                    .rect_filled(fill_rect, CornerRadius::same(2), theme.colors.accent);
            }
        }
        VideoCompressionState::Completed(r) => {
            let text = format!(
                "Done, {} â†’ {} ({:.1}%)",
                format_bytes(r.original_size_bytes),
                format_bytes(r.output_size_bytes),
                r.reduction_percent.abs()
            );
            let g = ui.painter().layout(
                text,
                egui::FontId::proportional(10.0),
                theme.colors.positive,
                text_w,
            );
            ui.painter()
                .galley(pos2(text_x, y), g, theme.colors.positive);
        }
        VideoCompressionState::Failed(err) => {
            let g = ui.painter().layout(
                format!("Failed: {err}"),
                egui::FontId::proportional(10.0),
                theme.colors.negative,
                text_w,
            );
            ui.painter()
                .galley(pos2(text_x, y), g, theme.colors.negative);
        }
        VideoCompressionState::Cancelled => {
            let g = ui.painter().layout_no_wrap(
                "Cancelled".to_owned(),
                egui::FontId::proportional(10.0),
                theme.colors.caution,
            );
            ui.painter()
                .galley(pos2(text_x, y), g, theme.colors.caution);
        }
    }

    // Delete button
    if let Some(btn) = &btn_resp {
        if row_hovered {
            let t = ui.ctx().animate_bool(btn.id, btn.hovered());
            let btn_fill = theme.mix(
                theme.colors.bg_raised,
                theme.colors.negative,
                0.10 + t * 0.15,
            );
            ui.painter()
                .rect_filled(btn_rect, CornerRadius::ZERO, btn_fill);
            ui.painter().rect_stroke(
                btn_rect,
                CornerRadius::ZERO,
                Stroke::new(
                    1.0,
                    theme.mix(theme.colors.border, theme.colors.negative, 0.3),
                ),
                StrokeKind::Middle,
            );
            ui.painter().text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                icons::TRASH,
                icons::font_id(13.0),
                theme.mix(theme.colors.negative, Color32::WHITE, 0.2 + t * 0.3),
            );
            if btn.clicked() {
                action.deleted = true;
            }
        }
    }

    if row_resp.clicked() && !action.deleted {
        action.clicked = true;
    }
    action
}

fn choice_button(ui: &mut Ui, theme: &AppTheme, label: &str, selected: bool) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).size(11.5).color(theme.colors.fg))
            .fill(if selected {
                theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.18)
            } else {
                theme.colors.bg_raised
            })
            .stroke(Stroke::new(1.0, theme.colors.border))
            .corner_radius(CornerRadius::ZERO),
    )
}

fn advanced_codec_button(
    ui: &mut Ui,
    theme: &AppTheme,
    codec: CodecChoice,
    selected: bool,
    enabled: bool,
    encoders: &EncoderAvailability,
) -> egui::Response {
    let (headline, detail) = match codec {
        CodecChoice::H264 => (
            "H.264",
            "Best compatibility with phones, browsers, and social apps.",
        ),
        CodecChoice::H265 => (
            "HEVC / H.265",
            "Smaller files than H.264, but some older devices may struggle.",
        ),
        CodecChoice::Av1 => (
            "AV1",
            "Best compression efficiency, with the heaviest compatibility tradeoff.",
        ),
    };
    let backend = if codec == CodecChoice::H264 && encoders.h264_nvidia
        || codec == CodecChoice::H265 && encoders.h265_nvidia
        || codec == CodecChoice::Av1 && encoders.av1_nvidia
    {
        "Auto GPU: NVIDIA"
    } else if codec == CodecChoice::H264 && encoders.h264_amd
        || codec == CodecChoice::H265 && encoders.h265_amd
        || codec == CodecChoice::Av1 && encoders.av1_amd
    {
        "Auto GPU: AMD"
    } else if enabled {
        "CPU encode"
    } else {
        "Unavailable"
    };

    let fill = if selected {
        theme.mix(theme.colors.surface, theme.colors.accent, 0.10)
    } else {
        theme.colors.bg_raised
    };
    let stroke = if selected {
        Stroke::new(
            1.0,
            theme.mix(theme.colors.border_focus, theme.colors.accent, 0.22),
        )
    } else {
        Stroke::new(1.0, theme.colors.border)
    };

    let frame = panel::inset(theme)
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_size(vec2(188.0, 72.0));
            ui.label(
                RichText::new(headline)
                    .size(12.0)
                    .strong()
                    .color(if enabled {
                        theme.colors.fg
                    } else {
                        theme.colors.fg_muted
                    }),
            );
            ui.add_sized(
                [ui.available_width(), 0.0],
                egui::Label::new(RichText::new(detail).size(10.5).color(if enabled {
                    theme.colors.fg_dim
                } else {
                    theme.colors.fg_muted
                }))
                .wrap(),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(backend).size(10.0).color(if enabled {
                theme.colors.accent
            } else {
                theme.colors.fg_muted
            }));
        });

    ui.interact(
        frame.response.rect,
        ui.id().with(("advanced_codec", codec.label())),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    )
}

fn advanced_bitrate_presets(video: &VideoMetadata, codec: CodecChoice) -> [(&'static str, u32); 4] {
    let source = video
        .video_bitrate_kbps
        .or(video.container_bitrate_kbps)
        .unwrap_or(3_500)
        .clamp(350, 80_000) as f32;
    let codec_factor = match codec {
        CodecChoice::H264 => 1.0,
        CodecChoice::H265 => 0.82,
        CodecChoice::Av1 => 0.72,
    };
    let base = (source * codec_factor).round().clamp(350.0, 80_000.0) as u32;

    [
        ("Near Source", base),
        (
            "Balanced",
            ((base as f32) * 0.72).round().clamp(350.0, 80_000.0) as u32,
        ),
        (
            "Smaller",
            ((base as f32) * 0.50).round().clamp(350.0, 80_000.0) as u32,
        ),
        (
            "Tiny",
            ((base as f32) * 0.35).round().clamp(350.0, 80_000.0) as u32,
        ),
    ]
}

fn roughly_matches_value(current: u32, target: u32) -> bool {
    current.abs_diff(target) <= current.max(target) / 20 + 60
}

fn format_kbps(kbps: u32) -> String {
    if kbps >= 1000 {
        format!("{:.1} Mbps", kbps as f32 / 1000.0)
    } else {
        format!("{kbps} kbps")
    }
}

fn secondary_button(ui: &mut Ui, theme: &AppTheme, label: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).size(11.5).color(theme.colors.fg))
            .fill(theme.colors.bg_raised)
            .stroke(Stroke::new(1.0, theme.colors.border))
            .corner_radius(CornerRadius::ZERO),
    )
}

fn render_simple_bar(ui: &mut Ui, theme: &AppTheme, progress: f32, label: &str) {
    if !label.is_empty() {
        ui.label(RichText::new(label).size(11.5).color(theme.colors.fg_dim));
        ui.add_space(4.0);
    }
    let width = ui.available_width().max(180.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 10.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), theme.colors.bg_base);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, theme.colors.border),
        StrokeKind::Middle,
    );
    let fill_rect = Rect::from_min_size(
        rect.min,
        vec2(rect.width() * progress.clamp(0.0, 1.0), rect.height()),
    );
    ui.painter()
        .rect_filled(fill_rect, CornerRadius::same(2), theme.colors.accent);
}

/// Card-style mode selector matching the compress_photos `preset_row` design.
fn mode_card(
    ui: &mut Ui,
    theme: &AppTheme,
    mode: CompressionMode,
    selected: bool,
) -> egui::Response {
    let accent = theme.colors.accent;
    let fill = if selected {
        theme.mix(theme.colors.surface, accent, 0.10)
    } else {
        theme.colors.bg_raised
    };
    let stroke = if selected {
        Stroke::new(1.0, theme.mix(theme.colors.border_focus, accent, 0.22))
    } else {
        Stroke::new(1.0, theme.colors.border)
    };

    let frame = panel::inset(theme)
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(52.0);
            ui.label(
                RichText::new(mode.title())
                    .size(12.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_sized(
                [ui.available_width(), 0.0],
                egui::Label::new(
                    RichText::new(mode.description())
                        .size(11.0)
                        .color(theme.colors.fg_dim),
                )
                .wrap(),
            );
        });

    ui.interact(
        frame.response.rect,
        ui.id().with(mode.title()),
        Sense::click(),
    )
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_index = 0usize;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

fn format_duration(seconds: f32) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}
