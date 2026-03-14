use std::{path::PathBuf, time::Duration};

use eframe::egui::{
    self, Align, Button, Color32, ColorImage, ComboBox, CornerRadius, Layout, RichText, ScrollArea,
    Sense, Slider, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, pos2, vec2,
};

use crate::{icons, modules::ModuleKind, theme::AppTheme, ui::components::panel};

use super::{
    compressor::{self, CompressionEvent, CompressionHandle},
    models::{
        CompressionPreset, CompressionResult, CompressionSettings, CompressionState, ConvertFormat,
        FileProgress, LoadedPhoto, PhotoAsset,
    },
};

// ─── State ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct CompressPhotosPage {
    files: Vec<PhotoListItem>,
    settings: CompressionSettings,
    active_batch: Option<CompressionHandle>,
    next_file_id: u64,
    last_output_dir: Option<PathBuf>,
    banner: Option<BannerMessage>,
}

struct PhotoListItem {
    asset: PhotoAsset,
    preview_texture: Option<TextureHandle>,
    state: CompressionState,
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

#[derive(Default)]
struct Summary {
    total: usize,
    active: usize,
    done: usize,
    failed: usize,
    orig_bytes: u64,
    out_bytes: u64,
}

impl Default for BannerTone {
    fn default() -> Self {
        Self::Info
    }
}

// ─── Spacing helpers ────────────────────────────────────────────────────────

fn flush(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
}
fn compact(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
}

// ─── Background polling ─────────────────────────────────────────────────────

impl CompressPhotosPage {
    pub fn poll_background(&mut self, ctx: &egui::Context) {
        let mut finished = None;

        if let Some(batch) = &self.active_batch {
            while let Ok(event) = batch.receiver.try_recv() {
                match event {
                    CompressionEvent::FileStarted { id } => {
                        if let Some(item) = self.files.iter_mut().find(|i| i.asset.id == id) {
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
                        if let Some(item) = self.files.iter_mut().find(|i| i.asset.id == id) {
                            item.state =
                                CompressionState::Compressing(FileProgress { progress, stage });
                        }
                    }
                    CompressionEvent::FileFinished { id, result } => {
                        if let Some(item) = self.files.iter_mut().find(|i| i.asset.id == id) {
                            item.state = CompressionState::Completed(result);
                        }
                    }
                    CompressionEvent::FileFailed { id, error } => {
                        if let Some(item) = self.files.iter_mut().find(|i| i.asset.id == id) {
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
                let n = self
                    .files
                    .iter()
                    .filter(|i| matches!(i.state, CompressionState::Completed(_)))
                    .count();
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Success,
                    text: format!("Done — {n} image(s) compressed."),
                });
            }
            self.active_batch = None;
        }

        if self.active_batch.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    // ─── Root layout (centered, max-width, flush) ───────────────────────

    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
    ) {
        self.handle_dropped_files(ctx);
        ui.set_width(ui.available_width());
        flush(ui);

        let avail = ui.available_width();
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

        ui.horizontal(|ui| {
            ui.add_space(page_margin);
            ui.allocate_ui_with_layout(
                vec2(content_w, ui.available_height()),
                Layout::top_down(Align::Min),
                |ui| {
                    flush(ui);

                    self.render_toolbar(ui, theme, active_module, ctx);
                    ui.add_space(16.0);

                    if let Some(msg) = &self.banner {
                        self.render_banner(ui, theme, msg);
                        ui.add_space(14.0);
                    }

                    let workspace_w = ui.available_width();
                    let workspace_h = ui.available_height().max(0.0);
                    let has_files = !self.files.is_empty();

                    if has_files && workspace_w >= 1080.0 {
                        ui.allocate_ui_with_layout(
                            vec2(workspace_w, workspace_h),
                            Layout::left_to_right(Align::Min),
                            |ui| {
                                flush(ui);
                                let gutter = 16.0;
                                let usable_w = (workspace_w - gutter * 2.0).max(0.0);
                                let queue_w = usable_w * 0.25;
                                let add_w = usable_w * 0.50;
                                let settings_w = usable_w * 0.25;

                                ui.allocate_ui_with_layout(
                                    vec2(queue_w, workspace_h),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        flush(ui);
                                        self.render_queue_column(ui, theme, workspace_h);
                                    },
                                );
                                ui.add_space(gutter);
                                ui.allocate_ui_with_layout(
                                    vec2(add_w, workspace_h),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        flush(ui);
                                        self.render_drop_zone(ui, ctx, theme, workspace_h);
                                    },
                                );
                                ui.add_space(gutter);
                                ui.allocate_ui_with_layout(
                                    vec2(settings_w, workspace_h),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        flush(ui);
                                        self.render_settings_column(ui, theme, workspace_h);
                                    },
                                );
                            },
                        );
                    } else if workspace_w >= 900.0 {
                        ui.allocate_ui_with_layout(
                            vec2(workspace_w, workspace_h),
                            Layout::left_to_right(Align::Min),
                            |ui| {
                                flush(ui);
                                let gutter = 16.0;
                                let left_w = (workspace_w - gutter) * 0.75;
                                let right_w = (workspace_w - gutter) * 0.25;

                                ui.allocate_ui_with_layout(
                                    vec2(left_w, workspace_h),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        flush(ui);
                                        let drop_h = if has_files {
                                            (workspace_h * 0.34).max(0.0)
                                        } else {
                                            workspace_h.max(0.0)
                                        };
                                        self.render_drop_zone(ui, ctx, theme, drop_h);
                                        if has_files {
                                            ui.add_space(12.0);
                                            self.render_queue_column(
                                                ui,
                                                theme,
                                                (workspace_h - drop_h - 12.0).max(0.0),
                                            );
                                        }
                                    },
                                );
                                ui.add_space(gutter);
                                ui.allocate_ui_with_layout(
                                    vec2(right_w, workspace_h),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        flush(ui);
                                        self.render_settings_column(ui, theme, workspace_h);
                                    },
                                );
                            },
                        );
                    } else {
                        self.render_stacked_workspace(ui, ctx, theme, workspace_h);
                    }
                },
            );
        });
    }

    fn render_queue_column(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        if self.files.is_empty() {
            return;
        }

        let summary_height = if self.summary().done > 0 { 100.0 } else { 84.0 };
        let queue_height = (height - summary_height - 12.0).max(0.0);

        self.render_summary_bar(ui, theme);
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), queue_height),
            Layout::top_down(Align::Min),
            |ui| self.render_queue(ui, theme, queue_height),
        );
    }

    fn render_settings_column(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        let actions_height = if self.active_batch.is_some() {
            132.0
        } else {
            112.0
        };
        let settings_height = (height - actions_height - 12.0).max(0.0);

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), settings_height),
            Layout::top_down(Align::Min),
            |ui| self.render_settings(ui, theme, settings_height),
        );
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), actions_height),
            Layout::top_down(Align::Min),
            |ui| self.render_actions(ui, theme, actions_height),
        );
    }

    fn render_stacked_workspace(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let has_files = !self.files.is_empty();
        let gap = 12.0;
        let drop_h = if has_files {
            height * 0.28
        } else {
            height * 0.42
        };
        let settings_h = height * 0.30;
        let queue_h = if has_files {
            (height - drop_h - settings_h - gap * 2.0).max(0.0)
        } else {
            0.0
        };

        self.render_drop_zone(ui, ctx, theme, drop_h.max(0.0));
        if has_files {
            ui.add_space(gap);
            self.render_queue_column(ui, theme, queue_h);
        }
        ui.add_space(gap);
        self.render_settings_column(ui, theme, settings_h.max(0.0));
    }

    // ─── Toolbar ────────────────────────────────────────────────────────

    fn render_toolbar(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        ctx: &egui::Context,
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
                        RichText::new("Compress Photos")
                            .size(20.0)
                            .strong()
                            .color(theme.colors.fg),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("{} Add Images", icons::ADD))
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
                            self.select_images(ctx);
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
                                if let Err(e) = open::that(dir) {
                                    self.banner = Some(BannerMessage {
                                        tone: BannerTone::Error,
                                        text: format!("Could not open output folder: {e}"),
                                    });
                                }
                            }
                        }
                    });
                });
            });
    }

    // ─── Banner ─────────────────────────────────────────────────────────

    fn render_banner(&self, ui: &mut Ui, theme: &AppTheme, msg: &BannerMessage) {
        let tint = match msg.tone {
            BannerTone::Info => theme.colors.accent,
            BannerTone::Success => theme.colors.positive,
            BannerTone::Error => theme.colors.negative,
        };

        panel::tinted(theme, tint)
            .inner_margin(egui::Margin::symmetric(20, 12))
            .show(ui, |ui| {
                ui.label(RichText::new(&msg.text).size(12.5).color(theme.colors.fg));
            });
    }

    // ─── Drop zone ──────────────────────────────────────────────────────

    fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let has_files = !self.files.is_empty();
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

                        let content_offset = if has_files { 86.0 } else { 110.0 };
                        ui.add_space(((ui.available_height() - content_offset) * 0.5).max(8.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if has_files {
                                    format!(
                                        "{} image(s) ready. Drop more images here anytime.",
                                        self.files.len()
                                    )
                                } else {
                                    "Drop images here to start your workspace".to_owned()
                                })
                                .size(if has_files { 13.0 } else { 16.0 })
                                .strong()
                                .color(theme.colors.fg),
                            );
                            ui.add_space(4.0);
                            ui.add_sized(
                                [ui.available_width().min(420.0), 0.0],
                                egui::Label::new(
                                    RichText::new(if has_files {
                                        "You can keep adding files before or during compression."
                                    } else {
                                        "Drag and drop JPG, PNG, WebP, or AVIF files, or browse from your device."
                                    })
                                    .size(12.0)
                                    .color(theme.colors.fg_dim),
                                )
                                .wrap(),
                            );
                            ui.add_space(10.0);

                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(format!(
                                            "{} {}",
                                            icons::IMAGES,
                                            if has_files {
                                                "Add More Images"
                                            } else {
                                                "Select Images"
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
                                self.select_images(ctx);
                            }
                        });
                    });
            },
        );
    }

    // ─── Summary bar ────────────────────────────────────────────────────

    fn render_summary_bar(&self, ui: &mut Ui, theme: &AppTheme) {
        if self.files.is_empty() {
            return;
        }

        let summary = self.summary();

        panel::card(theme)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                compact(ui);
                let mut stats = vec![
                    ("Files", summary.total.to_string()),
                    ("Active", summary.active.to_string()),
                    ("Done", summary.done.to_string()),
                ];
                if summary.failed > 0 {
                    stats.push(("Failed", summary.failed.to_string()));
                }

                ui.horizontal_top(|ui| {
                    flush(ui);
                    let gap = 8.0;
                    let cols = stats.len().max(1);
                    let cell_w = ((ui.available_width() - gap * (cols.saturating_sub(1) as f32))
                        / cols as f32)
                        .max(72.0);
                    let cell_h = 60.0;

                    for (idx, (label, value)) in stats.iter().enumerate() {
                        if idx > 0 {
                            ui.add_space(gap);
                        }
                        stat_cell_compact(ui, theme, label, value, cell_w, cell_h);
                    }
                });

                if summary.done > 0 {
                    let saved = summary.orig_bytes.saturating_sub(summary.out_bytes);
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{} → {} (−{})",
                            fmt_bytes(summary.orig_bytes),
                            fmt_bytes(summary.out_bytes),
                            fmt_bytes(saved),
                        ))
                        .size(11.0)
                        .color(theme.colors.positive),
                    );
                }
            });
    }

    // ─── Settings ───────────────────────────────────────────────────────

    fn render_settings(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                ui.label(
                    RichText::new("Settings")
                        .size(14.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                ui.add_space(8.0);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height((height - 34.0).max(0.0))
                    .show(ui, |ui| {
                        compact(ui);
                        ui.set_width(ui.available_width());

                        for preset in CompressionPreset::ALL {
                            let selected = self.settings.preset == preset;
                            if preset_row(ui, theme, preset, selected).clicked() {
                                self.settings.apply_preset(preset);
                            }
                        }

                        ui.checkbox(&mut self.settings.advanced_mode, "Advanced mode");

                        if self.settings.advanced_mode {
                            ui.add(
                                Slider::new(&mut self.settings.quality, 1..=100)
                                    .text("Quality")
                                    .show_value(true),
                            );
                            ui.add(
                                Slider::new(&mut self.settings.resize_percent, 25..=100)
                                    .text("Resize")
                                    .suffix("%")
                                    .show_value(true),
                            );
                            ui.checkbox(&mut self.settings.strip_metadata, "Strip metadata");

                            ComboBox::from_label("Format")
                                .selected_text(self.settings.format_choice.label())
                                .show_ui(ui, |ui| {
                                    for f in ConvertFormat::ALL {
                                        ui.selectable_value(
                                            &mut self.settings.format_choice,
                                            f,
                                            f.label(),
                                        );
                                    }
                                });
                        }
                    });
            });
    }

    // ─── Actions ────────────────────────────────────────────────────────

    fn render_actions(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        let accent = theme.colors.accent;
        let can_go = !self.files.is_empty() && self.active_batch.is_none();

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                if ui
                    .add_enabled(
                        can_go,
                        Button::new(
                            RichText::new(format!("{} Compress", icons::PLAY))
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
                    self.start_compression();
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
                            self.banner = Some(BannerMessage {
                                tone: BannerTone::Info,
                                text: "Cancel requested.".into(),
                            });
                        }
                    }

                    if ui
                        .add_enabled(
                            self.active_batch.is_none() && !self.files.is_empty(),
                            Button::new(RichText::new("Clear").size(12.0).color(theme.colors.fg))
                                .fill(theme.mix(theme.colors.surface, theme.colors.negative, 0.08))
                                .stroke(Stroke::new(
                                    1.0,
                                    theme.mix(theme.colors.border, theme.colors.negative, 0.2),
                                ))
                                .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        self.files.clear();
                        self.banner = None;
                    }
                });

                if self.active_batch.is_some() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(
                            RichText::new("Compressing…")
                                .size(11.0)
                                .color(theme.colors.fg_dim),
                        );
                    });
                }
            });
    }

    // ─── Queue ──────────────────────────────────────────────────────────

    fn render_queue(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        if self.files.is_empty() {
            return;
        }

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                ui.label(
                    RichText::new(format!("Queue — {}", self.files.len()))
                        .size(13.0)
                        .strong()
                        .color(theme.colors.fg),
                );

                let queue_height = (height - 28.0).max(0.0);
                ui.add_space(8.0);
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), queue_height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(queue_height)
                            .show(ui, |ui| {
                                flush(ui);
                                for item in &mut self.files {
                                    queue_row(ui, theme, item);
                                }
                            });
                    },
                );
            });
    }

    // ─── Feed ───────────────────────────────────────────────────────────

    // Feed removed — queue already shows all status info

    // ─── File I/O helpers ───────────────────────────────────────────────

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let paths = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        if !paths.is_empty() {
            self.add_paths(ctx, paths);
        }
    }

    fn select_images(&mut self, ctx: &egui::Context) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "avif"])
            .pick_files()
        {
            self.add_paths(ctx, paths);
        }
    }

    fn add_paths(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let mut added = 0usize;
        let mut rejected = Vec::new();

        for path in paths {
            if self.files.iter().any(|i| i.asset.path == path) {
                continue;
            }
            self.next_file_id += 1;
            match compressor::load_photo(path, self.next_file_id) {
                Ok(photo) => {
                    self.files.push(make_item(ctx, photo));
                    added += 1;
                }
                Err(e) => rejected.push(e),
            }
        }

        if !rejected.is_empty() {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Error,
                text: rejected.join("  "),
            });
        } else if added > 0 {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: format!("Added {added} image(s) to the queue."),
            });
        }
    }

    fn start_compression(&mut self) {
        match compressor::start_batch(
            self.files.iter().map(|i| i.asset.clone()).collect(),
            self.settings.clone(),
        ) {
            Ok(handle) => {
                self.last_output_dir = Some(handle.output_dir.clone());
                self.active_batch = Some(handle);
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Compression started — UI stays interactive.".into(),
                });
                for item in &mut self.files {
                    item.state = CompressionState::Ready;
                }
            }
            Err(e) => {
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Error,
                    text: e,
                });
            }
        }
    }

    fn summary(&self) -> Summary {
        let mut s = Summary {
            total: self.files.len(),
            ..Default::default()
        };
        for item in &self.files {
            match &item.state {
                CompressionState::Compressing(_) => s.active += 1,
                CompressionState::Completed(r) => {
                    s.done += 1;
                    s.orig_bytes += r.original_size;
                    s.out_bytes += r.compressed_size;
                }
                CompressionState::Failed(_) => s.failed += 1,
                _ => {}
            }
        }
        s
    }
}

// ─── Free functions ─────────────────────────────────────────────────────────

fn make_item(ctx: &egui::Context, photo: LoadedPhoto) -> PhotoListItem {
    let id = photo.asset.id;
    let tex = photo.preview.map(|p| {
        ctx.load_texture(
            format!("photo-preview-{id}"),
            ColorImage::from_rgba_unmultiplied(p.size, &p.rgba),
            TextureOptions::LINEAR,
        )
    });
    PhotoListItem {
        asset: photo.asset,
        preview_texture: tex,
        state: CompressionState::Ready,
    }
}

fn queue_row(ui: &mut Ui, theme: &AppTheme, item: &mut PhotoListItem) {
    panel::inset(theme)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                thumb(ui, theme, item.preview_texture.as_ref(), 42.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(&item.asset.file_name)
                            .size(12.0)
                            .strong()
                            .color(theme.colors.fg),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · {}×{}",
                            item.asset.format.label(),
                            fmt_bytes(item.asset.original_size),
                            item.asset.dimensions.0,
                            item.asset.dimensions.1
                        ))
                        .size(10.0)
                        .color(theme.colors.fg_dim),
                    );
                    state_badge(ui, theme, &item.state);
                });
            });
        });
}

// ─── Utility widgets ────────────────────────────────────────────────────────

fn state_badge(ui: &mut Ui, theme: &AppTheme, state: &CompressionState) {
    match state {
        CompressionState::Ready => pill(ui, theme, "Ready", theme.colors.fg_muted),
        CompressionState::Compressing(p) => pill(ui, theme, &p.stage, theme.colors.accent),
        CompressionState::Completed(r) => done_badge(ui, theme, r),
        CompressionState::Failed(_) => pill(ui, theme, "Failed", theme.colors.negative),
        CompressionState::Cancelled => pill(ui, theme, "Cancelled", theme.colors.caution),
    }
}

fn pill(ui: &mut Ui, theme: &AppTheme, text: &str, tint: Color32) {
    panel::chip_accent(theme, tint).show(ui, |ui| {
        ui.label(
            RichText::new(text)
                .size(11.0)
                .color(theme.mix(tint, Color32::WHITE, 0.18)),
        );
    });
}

fn done_badge(ui: &mut Ui, theme: &AppTheme, r: &CompressionResult) {
    pill(
        ui,
        theme,
        &format!("Done · {:+.1}%", r.reduction_percent),
        theme.colors.positive,
    );
}

fn stat_cell_compact(
    ui: &mut Ui,
    theme: &AppTheme,
    label: &str,
    value: &str,
    width: f32,
    height: f32,
) {
    ui.allocate_ui_with_layout(vec2(width, height), Layout::top_down(Align::Min), |ui| {
        panel::inset(theme)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_min_size(vec2(width, height));
                ui.label(
                    RichText::new(value)
                        .size(16.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                ui.label(
                    RichText::new(label.to_ascii_uppercase())
                        .size(9.0)
                        .color(theme.colors.fg_muted),
                );
            });
    });
}

fn preset_row(
    ui: &mut Ui,
    theme: &AppTheme,
    preset: CompressionPreset,
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
            ui.set_min_height(68.0);
            ui.label(
                RichText::new(preset.title())
                    .size(12.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_sized(
                [ui.available_width(), 0.0],
                egui::Label::new(
                    RichText::new(preset.description())
                        .size(11.0)
                        .color(theme.colors.fg_dim),
                )
                .wrap(),
            );
        });

    ui.interact(
        frame.response.rect,
        ui.id().with(preset.title()),
        Sense::click(),
    )
}

fn thumb(ui: &mut Ui, theme: &AppTheme, tex: Option<&TextureHandle>, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme.colors.bg_raised);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme.colors.border),
        StrokeKind::Middle,
    );
    if let Some(tex) = tex {
        let img = rect.shrink(4.0);
        ui.painter().image(
            tex.id(),
            img,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::IMAGE,
            icons::font_id(14.0),
            theme.colors.fg_muted,
        );
    }
}

fn fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{} {}", bytes, UNITS[u])
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}
