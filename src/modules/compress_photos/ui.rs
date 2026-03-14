use std::{path::PathBuf, time::Duration};

use eframe::egui::{
    self, Align, Button, Color32, ColorImage, CornerRadius, Id, Layout, Rect, RichText,
    ScrollArea, Sense, Slider, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2, pos2,
    vec2,
};

use crate::{icons, modules::ModuleKind, theme::AppTheme, ui::components::panel};

use super::{
    compressor::{self, CompressionEvent, CompressionHandle},
    models::{
        CompressionPreset, CompressionSettings, CompressionState, ConvertFormat,
        FileProgress, LoadedPhoto, PhotoAsset,
    },
};

// ─── State ──────────────────────────────────────────────────────────────────

pub struct CompressPhotosPage {
    files: Vec<PhotoListItem>,
    settings: CompressionSettings,
    active_batch: Option<CompressionHandle>,
    next_file_id: u64,
    last_output_dir: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    banner: Option<BannerMessage>,
    selected_file_id: Option<u64>,
    preview_zoom: f32,
    preview_offset: Vec2,
    preview_output_texture: Option<(u64, TextureHandle)>,
    before_after_split: f32,
}

impl Default for CompressPhotosPage {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            settings: CompressionSettings::default(),
            active_batch: None,
            next_file_id: 0,
            last_output_dir: None,
            output_dir: None,
            banner: None,
            selected_file_id: None,
            preview_zoom: 1.0,
            preview_offset: Vec2::ZERO,
            preview_output_texture: None,
            before_after_split: 0.5,
        }
    }
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
        flush(ui);

        // Use the actual finite rect from CentralPanel for reliable height
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

        // Allocate a finite content rect with matching bottom padding
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

        self.render_toolbar(&mut content_ui, theme, active_module, ctx);
        content_ui.add_space(16.0);

        if let Some(msg) = &self.banner {
            self.render_banner(&mut content_ui, theme, msg);
            content_ui.add_space(14.0);
        }

        let workspace_w = content_ui.available_width();
        let workspace_h = content_ui.available_height().max(0.0);
        let has_files = !self.files.is_empty();

        if has_files && workspace_w >= 1080.0 {
            content_ui.allocate_ui_with_layout(
                vec2(workspace_w, workspace_h),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let usable_w = (workspace_w - gutter * 2.0).max(0.0);
                    let queue_w = usable_w * 0.25;
                    let add_w = usable_w * 0.50;

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
                            if self.selected_file_id.is_some() {
                                let half = (workspace_h - 12.0) * 0.5;
                                self.render_drop_zone(ui, ctx, theme, half);
                                ui.add_space(12.0);
                                self.render_preview(ui, ctx, theme, half);
                            } else {
                                self.render_drop_zone(ui, ctx, theme, workspace_h);
                            }
                        },
                    );
                    ui.add_space(gutter);
                    // Use remaining width for settings to prevent overflow
                    let settings_w = ui.available_width();
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
            content_ui.allocate_ui_with_layout(
                vec2(workspace_w, workspace_h),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let left_w = (workspace_w - gutter) * 0.75;

                    ui.allocate_ui_with_layout(
                        vec2(left_w, workspace_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            let has_preview = has_files && self.selected_file_id.is_some();
                            let drop_h = if has_files {
                                if has_preview {
                                    (workspace_h * 0.17).max(0.0)
                                } else {
                                    (workspace_h * 0.34).max(0.0)
                                }
                            } else {
                                workspace_h.max(0.0)
                            };
                            self.render_drop_zone(ui, ctx, theme, drop_h);
                            if has_preview {
                                ui.add_space(12.0);
                                let preview_h = (workspace_h * 0.17).max(0.0);
                                self.render_preview(ui, ctx, theme, preview_h);
                                ui.add_space(12.0);
                                self.render_queue_column(
                                    ui, theme,
                                    (workspace_h - drop_h - preview_h - 24.0).max(0.0),
                                );
                            } else if has_files {
                                ui.add_space(12.0);
                                self.render_queue_column(
                                    ui, theme,
                                    (workspace_h - drop_h - 12.0).max(0.0),
                                );
                            }
                        },
                    );
                    ui.add_space(gutter);
                    // Use remaining width for settings to prevent overflow
                    let right_w = ui.available_width();
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
            self.render_stacked_workspace(&mut content_ui, ctx, theme, workspace_h);
        }
    }

    fn render_queue_column(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        if self.files.is_empty() {
            return;
        }

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), height),
            Layout::top_down(Align::Min),
            |ui| self.render_queue(ui, theme, height),
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

                            if has_files {
                                // Two buttons: Add More + Change Output
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new(format!("{} Add More Images", icons::IMAGES))
                                                    .size(12.0)
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

                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new(format!("{} Change Output", icons::FOLDER))
                                                    .size(12.0)
                                                    .color(theme.colors.fg),
                                            )
                                            .fill(theme.colors.bg_raised)
                                            .stroke(Stroke::new(1.0, theme.colors.border))
                                            .corner_radius(CornerRadius::ZERO),
                                        )
                                        .clicked()
                                    {
                                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                            self.output_dir = Some(dir);
                                        }
                                    }
                                });

                                // Show current output path
                                if let Some(dir) = &self.output_dir {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(format!("Output: {}", dir.display()))
                                            .size(10.0)
                                            .color(theme.colors.fg_dim),
                                    );
                                } else {
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new("Output: Auto (compressity-output/photos/)")
                                            .size(10.0)
                                            .color(theme.colors.fg_muted),
                                    );
                                }
                            } else {
                                // No files yet: single Select Images button
                                if ui
                                    .add(
                                        Button::new(
                                            RichText::new(format!("{} Select Images", icons::IMAGES))
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
                            }
                        });
                    });
            },
        );
    }

    // ─── Image preview ──────────────────────────────────────────────────

    fn render_preview(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let sel_id = match self.selected_file_id {
            Some(id) => id,
            None => return,
        };
        let item = match self.files.iter().find(|f| f.asset.id == sel_id) {
            Some(item) => item,
            None => {
                self.selected_file_id = None;
                return;
            }
        };

        let is_done = matches!(item.state, CompressionState::Completed(_));

        panel::card(theme)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 24.0).max(0.0));

                // Header with filename and close button
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&item.asset.file_name)
                            .size(12.0).strong().color(theme.colors.fg),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(
                            Button::new(
                                RichText::new(format!("{}", icons::CLOSE))
                                    .family(icons::font_family())
                                    .size(14.0).color(theme.colors.fg_dim),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                        ).clicked() {
                            self.selected_file_id = None;
                        }
                        // Zoom controls
                        if ui.add(
                            Button::new(icons::rich(icons::ZOOM_OUT, 13.0, theme.colors.fg_dim))
                                .fill(theme.colors.bg_raised)
                                .stroke(Stroke::new(1.0, theme.colors.border))
                                .corner_radius(CornerRadius::ZERO),
                        ).clicked() {
                            self.preview_zoom = (self.preview_zoom - 0.25).max(0.25);
                        }
                        ui.label(
                            RichText::new(format!("{:.0}%", self.preview_zoom * 100.0))
                                .size(10.0).color(theme.colors.fg_dim),
                        );
                        if ui.add(
                            Button::new(icons::rich(icons::ZOOM_IN, 13.0, theme.colors.fg_dim))
                                .fill(theme.colors.bg_raised)
                                .stroke(Stroke::new(1.0, theme.colors.border))
                                .corner_radius(CornerRadius::ZERO),
                        ).clicked() {
                            self.preview_zoom = (self.preview_zoom + 0.25).min(5.0);
                        }
                    });
                });
                ui.add_space(6.0);

                // Image area
                let avail = vec2(ui.available_width(), (height - 64.0).max(40.0));
                let (img_rect, img_resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());

                // Background
                ui.painter().rect_filled(img_rect, CornerRadius::ZERO, theme.colors.bg_base);
                ui.painter().rect_stroke(
                    img_rect, CornerRadius::ZERO,
                    Stroke::new(1.0, theme.colors.border), StrokeKind::Middle,
                );

                // Handle zoom with scroll
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if img_resp.hovered() && scroll.abs() > 0.1 {
                    let old_zoom = self.preview_zoom;
                    self.preview_zoom = (self.preview_zoom + scroll * 0.002).clamp(0.25, 5.0);
                    let _ = old_zoom; // zoom around center
                }

                // Handle drag
                if img_resp.dragged() {
                    self.preview_offset += img_resp.drag_delta();
                }

                // Render the preview
                let has_output_tex = is_done
                    && self.preview_output_texture.as_ref().map(|(id, _)| *id) == Some(sel_id);

                if has_output_tex {
                    // ── Before/After slider ──
                    let orig_tex = item.preview_texture.as_ref();
                    let out_tex = self.preview_output_texture.as_ref().map(|(_, t)| t);

                    if let (Some(orig), Some(out)) = (orig_tex, out_tex) {
                        let clip = ui.painter().with_clip_rect(img_rect);
                        let orig_size = orig.size_vec2();
                        let scale = (img_rect.width() / orig_size.x)
                            .min(img_rect.height() / orig_size.y)
                            * self.preview_zoom;
                        let display_size = orig_size * scale;
                        let center = img_rect.center() + self.preview_offset;
                        let img_draw_rect = Rect::from_center_size(center, display_size);

                        let split_x = img_rect.left()
                            + img_rect.width() * self.before_after_split;

                        // Left side: original (clipped to left of split)
                        let left_clip = Rect::from_min_max(
                            img_rect.min,
                            pos2(split_x, img_rect.max.y),
                        );
                        ui.painter().with_clip_rect(left_clip).image(
                            orig.id(),
                            img_draw_rect,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );

                        // Right side: compressed (clipped to right of split)
                        let right_clip = Rect::from_min_max(
                            pos2(split_x, img_rect.min.y),
                            img_rect.max,
                        );
                        ui.painter().with_clip_rect(right_clip).image(
                            out.id(),
                            img_draw_rect,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );

                        // Divider line
                        clip.vline(
                            split_x,
                            img_rect.y_range(),
                            Stroke::new(2.0, theme.colors.accent),
                        );

                        // Divider handle
                        let handle_size = vec2(28.0, 20.0);
                        let handle_rect = Rect::from_center_size(
                            pos2(split_x, img_rect.center().y),
                            handle_size,
                        );
                        clip.rect_filled(handle_rect, CornerRadius::same(3), theme.colors.accent);
                        clip.text(
                            handle_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "◄►",
                            egui::FontId::proportional(10.0),
                            Color32::BLACK,
                        );

                        // Drag the divider
                        let divider_resp = ui.interact(
                            Rect::from_center_size(
                                pos2(split_x, img_rect.center().y),
                                vec2(20.0, img_rect.height()),
                            ),
                            Id::new("ba_slider").with(sel_id),
                            Sense::drag(),
                        );
                        if divider_resp.dragged() {
                            let dx = divider_resp.drag_delta().x;
                            self.before_after_split =
                                (self.before_after_split + dx / img_rect.width()).clamp(0.02, 0.98);
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                        } else if divider_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
                        }

                        // Labels
                        clip.text(
                            pos2(img_rect.left() + 8.0, img_rect.top() + 8.0),
                            egui::Align2::LEFT_TOP,
                            "Before",
                            egui::FontId::proportional(11.0),
                            theme.colors.fg_dim,
                        );
                        clip.text(
                            pos2(img_rect.right() - 8.0, img_rect.top() + 8.0),
                            egui::Align2::RIGHT_TOP,
                            "After",
                            egui::FontId::proportional(11.0),
                            theme.colors.positive,
                        );
                    }
                } else if let Some(tex) = &item.preview_texture {
                    // ── Normal preview (non-Done or no output tex) ──
                    let tex_size = tex.size_vec2();
                    let scale = (img_rect.width() / tex_size.x)
                        .min(img_rect.height() / tex_size.y)
                        * self.preview_zoom;
                    let display_size = tex_size * scale;
                    let center = img_rect.center() + self.preview_offset;

                    ui.painter().with_clip_rect(img_rect).image(
                        tex.id(),
                        Rect::from_center_size(center, display_size),
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    ui.painter().text(
                        img_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Preview not available",
                        egui::FontId::proportional(13.0),
                        theme.colors.fg_muted,
                    );
                }

                // Change cursor on hover
                if img_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                if img_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
            });
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
                    .id_salt("settings_scroll")
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

                            // Format selector styled to match the active preset row
                            format_selector(ui, theme, &mut self.settings.format_choice);
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

        let mut clicked_id: Option<u64> = None;
        let mut delete_id: Option<u64> = None;
        let is_compressing = self.active_batch.is_some();

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                let scroll_h = (height - 42.0).max(0.0);
                ScrollArea::vertical()
                    .id_salt("queue_scroll")
                    .auto_shrink([false, false])
                    .max_height(scroll_h)
                    .show(ui, |ui| {
                        flush(ui);
                        ui.set_width(ui.available_width());

                        let queue_items: Vec<usize> = self.files.iter().enumerate()
                            .filter(|(_, i)| matches!(i.state, CompressionState::Ready))
                            .map(|(idx, _)| idx).collect();
                        let progress_items: Vec<usize> = self.files.iter().enumerate()
                            .filter(|(_, i)| matches!(i.state, CompressionState::Compressing(_)))
                            .map(|(idx, _)| idx).collect();
                        let done_items: Vec<usize> = self.files.iter().enumerate()
                            .filter(|(_, i)| matches!(i.state,
                                CompressionState::Completed(_) | CompressionState::Failed(_) | CompressionState::Cancelled
                            ))
                            .map(|(idx, _)| idx).collect();

                        if !queue_items.is_empty() {
                            queue_section_header(ui, theme, "Queue", queue_items.len(), theme.colors.fg_muted);
                            for &idx in &queue_items {
                                let action = queue_row_interactive(ui, theme, &self.files[idx], true, !is_compressing);
                                if action.clicked { clicked_id = Some(self.files[idx].asset.id); }
                                if action.deleted { delete_id = Some(self.files[idx].asset.id); }
                            }
                            ui.add_space(8.0);
                        }

                        if !progress_items.is_empty() {
                            queue_section_header(ui, theme, "Progress", progress_items.len(), theme.colors.accent);
                            for &idx in &progress_items {
                                let action = queue_row_interactive(ui, theme, &self.files[idx], false, false);
                                if action.clicked { clicked_id = Some(self.files[idx].asset.id); }
                            }
                            ui.add_space(8.0);
                        }

                        if !done_items.is_empty() {
                            queue_section_header(ui, theme, "Done", done_items.len(), theme.colors.positive);
                            for &idx in &done_items {
                                let action = queue_row_interactive(ui, theme, &self.files[idx], false, false);
                                if action.clicked { clicked_id = Some(self.files[idx].asset.id); }
                            }
                        }
                    });
            });

        if let Some(id) = delete_id {
            self.files.retain(|f| f.asset.id != id);
            if self.selected_file_id == Some(id) {
                self.selected_file_id = None;
            }
        }
        if let Some(id) = clicked_id {
            if self.selected_file_id == Some(id) {
                self.selected_file_id = None;
            } else {
                self.selected_file_id = Some(id);
                self.preview_zoom = 1.0;
                self.preview_offset = Vec2::ZERO;
                self.before_after_split = 0.5;
                // Load output texture for Done items
                if let Some(item) = self.files.iter().find(|f| f.asset.id == id) {
                    if let CompressionState::Completed(r) = &item.state {
                        if let Ok(img) = image::open(&r.output_path) {
                            let thumb = img.thumbnail(512, 512).to_rgba8();
                            let size = [thumb.width() as usize, thumb.height() as usize];
                            let tex = ui.ctx().load_texture(
                                format!("output-preview-{id}"),
                                ColorImage::from_rgba_unmultiplied(size, thumb.as_raw()),
                                TextureOptions::LINEAR,
                            );
                            self.preview_output_texture = Some((id, tex));
                        } else {
                            self.preview_output_texture = None;
                        }
                    } else {
                        self.preview_output_texture = None;
                    }
                }
            }
        }
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
        // Only compress files that are Ready (not already Done/Failed/Cancelled)
        let ready_assets: Vec<PhotoAsset> = self.files.iter()
            .filter(|i| matches!(i.state, CompressionState::Ready))
            .map(|i| i.asset.clone())
            .collect();

        match compressor::start_batch(ready_assets, self.settings.clone(), self.output_dir.clone()) {
            Ok(handle) => {
                self.last_output_dir = Some(handle.output_dir.clone());
                self.active_batch = Some(handle);
                self.banner = Some(BannerMessage {
                    tone: BannerTone::Info,
                    text: "Compression started — UI stays interactive.".into(),
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

// ─── Queue section header ───────────────────────────────────────────────────

fn queue_section_header(ui: &mut Ui, theme: &AppTheme, title: &str, count: usize, tint: Color32) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{title} — {count}"))
                .size(12.0)
                .strong()
                .color(tint),
        );
    });
    // Separator line
    let w = ui.available_width();
    let (line_rect, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().rect_filled(
        line_rect,
        CornerRadius::ZERO,
        theme.mix(theme.colors.border, tint, 0.30),
    );
    ui.add_space(4.0);
}

// ─── Queue row action ───────────────────────────────────────────────────────

struct QueueRowAction {
    clicked: bool,
    deleted: bool,
}

fn queue_row_interactive(
    ui: &mut Ui,
    theme: &AppTheme,
    item: &PhotoListItem,
    show_delete: bool,
    can_delete: bool,
) -> QueueRowAction {
    let mut action = QueueRowAction { clicked: false, deleted: false };
    let id = Id::new("queue_row").with(item.asset.id);

    let frame_resp = panel::inset(theme)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                thumb(ui, theme, item.preview_texture.as_ref(), 42.0);
                ui.add_space(8.0);
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.set_width(ui.available_width() - if show_delete { 28.0 } else { 0.0 });
                    ui.label(
                        RichText::new(&item.asset.file_name)
                            .size(12.0).strong().color(theme.colors.fg),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · {}×{}",
                            item.asset.format.label(),
                            fmt_bytes(item.asset.original_size),
                            item.asset.dimensions.0,
                            item.asset.dimensions.1
                        ))
                        .size(10.0).color(theme.colors.fg_dim),
                    );
                    // State-specific content
                    match &item.state {
                        CompressionState::Ready => {
                            ui.label(
                                RichText::new("Waiting For Compress")
                                    .size(10.0).color(theme.colors.fg_muted),
                            );
                        }
                        CompressionState::Compressing(p) => {
                            ui.label(
                                RichText::new("Compressing")
                                    .size(10.0).color(theme.colors.accent),
                            );
                            ui.add_space(2.0);
                            let fraction = p.progress.clamp(0.0, 1.0);
                            let bar_h = 4.0;
                            let bar_w = ui.available_width().max(20.0);
                            let (bar_rect, _) = ui.allocate_exact_size(vec2(bar_w, bar_h), Sense::hover());
                            ui.painter().rect_filled(bar_rect, CornerRadius::same(2), theme.colors.bg_raised);
                            ui.painter().rect_stroke(bar_rect, CornerRadius::same(2), Stroke::new(0.5, theme.colors.border), StrokeKind::Middle);
                            if fraction > 0.0 {
                                let fill_rect = Rect::from_min_size(bar_rect.min, vec2(bar_rect.width() * fraction, bar_h));
                                ui.painter().rect_filled(fill_rect, CornerRadius::same(2), theme.colors.accent);
                            }
                        }
                        CompressionState::Completed(r) => {
                            ui.label(
                                RichText::new(format!(
                                    "Done, {} → {} ({:.1}%)",
                                    fmt_bytes(r.original_size),
                                    fmt_bytes(r.compressed_size),
                                    r.reduction_percent.abs()
                                ))
                                .size(10.0).color(theme.colors.positive),
                            );
                        }
                        CompressionState::Failed(err) => {
                            ui.label(
                                RichText::new(format!("Failed: {err}"))
                                    .size(10.0).color(theme.colors.negative),
                            );
                        }
                        CompressionState::Cancelled => {
                            ui.label(
                                RichText::new("Cancelled")
                                    .size(10.0).color(theme.colors.caution),
                            );
                        }
                    }
                });
            });
        });

    // Click to select
    let row_rect = frame_resp.response.rect;
    let row_resp = ui.interact(row_rect, id.with("click"), Sense::click());
    if row_resp.clicked() {
        action.clicked = true;
    }

    // Hover highlight
    if row_resp.hovered() {
        ui.painter().rect_stroke(
            row_rect, CornerRadius::ZERO,
            Stroke::new(1.0, theme.colors.border_focus), StrokeKind::Middle,
        );
    }

    // Trash button (only for Ready items, shown on hover)
    if show_delete && can_delete && row_resp.hovered() {
        let btn_size = vec2(24.0, 24.0);
        let btn_pos = pos2(
            row_rect.right() - btn_size.x - 12.0,
            row_rect.center().y - btn_size.y * 0.5,
        );
        let btn_rect = Rect::from_min_size(btn_pos, btn_size);
        let btn_resp = ui.interact(btn_rect, id.with("trash"), Sense::click());

        let t = ui.ctx().animate_bool(btn_resp.id, btn_resp.hovered());
        let fill = theme.mix(theme.colors.bg_raised, theme.colors.negative, 0.10 + t * 0.15);
        ui.painter().rect_filled(btn_rect, CornerRadius::ZERO, fill);
        ui.painter().rect_stroke(
            btn_rect, CornerRadius::ZERO,
            Stroke::new(1.0, theme.mix(theme.colors.border, theme.colors.negative, 0.3)),
            StrokeKind::Middle,
        );
        ui.painter().text(
            btn_rect.center(), egui::Align2::CENTER_CENTER,
            icons::TRASH, icons::font_id(13.0),
            theme.mix(theme.colors.negative, Color32::WHITE, 0.2 + t * 0.3),
        );

        if btn_resp.clicked() {
            action.deleted = true;
        }
    }

    action
}

// ─── Utility widgets ────────────────────────────────────────────────────────

fn format_selector(ui: &mut Ui, theme: &AppTheme, format_choice: &mut ConvertFormat) {
    let accent = theme.colors.accent;
    let selected_fill = theme.mix(theme.colors.surface, accent, 0.10);
    let selected_stroke = Stroke::new(
        1.0,
        theme.mix(theme.colors.border_focus, accent, 0.22),
    );

    ui.label(
        RichText::new("Format")
            .size(12.0)
            .color(theme.colors.fg_dim),
    );
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for f in ConvertFormat::ALL {
            let is_selected = *format_choice == f;
            let fill = if is_selected {
                selected_fill
            } else {
                theme.colors.bg_raised
            };
            let stroke = if is_selected {
                selected_stroke
            } else {
                Stroke::new(1.0, theme.colors.border)
            };
            let text_color = if is_selected {
                theme.colors.accent
            } else {
                theme.colors.fg_dim
            };

            if ui
                .add(
                    Button::new(
                        RichText::new(f.label())
                            .size(11.0)
                            .color(text_color),
                    )
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(CornerRadius::ZERO),
                )
                .clicked()
            {
                *format_choice = f;
            }
        }
    });
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
