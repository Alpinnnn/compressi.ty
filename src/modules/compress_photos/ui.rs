mod chrome;
mod controls;
mod preview;
mod preview_loading;
mod queue;
mod widgets;
mod workspace;

use eframe::egui::{self, Align, Layout, Rect, Ui, vec2};

use crate::{modules::ModuleKind, settings::AppSettings, theme::AppTheme};

pub(super) use super::{BannerMessage, BannerTone, CompressPhotosPage, PhotoListItem};

pub(super) fn flush(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
}

pub(super) fn compact(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
}

pub(super) fn truncate_filename(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        return name.to_owned();
    }
    if let Some(dot_pos) = name.rfind('.') {
        let ext = &name[dot_pos..];
        let stem_budget = max_chars.saturating_sub(ext.len()).saturating_sub(1);
        if stem_budget >= 4 {
            return format!("{}...{}", &name[..stem_budget], ext);
        }
    }
    format!("{}...", &name[..max_chars.saturating_sub(1)])
}

impl CompressPhotosPage {
    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let paths = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if !paths.is_empty() {
            self.add_paths(paths);
        }
    }

    pub(super) fn select_images(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "avif"])
            .pick_files()
        {
            self.add_paths(paths);
        }
    }

    /// Renders the full photo compression workspace.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        app_settings: &AppSettings,
    ) {
        if !self.output_dir_user_set {
            self.output_dir = app_settings.preferred_photo_output_folder();
        }
        self.handle_dropped_files(ctx);
        self.apply_pending_loaded_photos(ctx);
        self.poll_preview_loader(ctx);
        flush(ui);

        let panel_rect = ui.max_rect();
        let avail_width = panel_rect.width();
        let page_margin = if avail_width >= 1280.0 {
            28.0
        } else if avail_width >= 960.0 {
            22.0
        } else if avail_width >= 720.0 {
            16.0
        } else {
            12.0
        };
        let content_width = (avail_width - page_margin * 2.0).max(0.0);
        let content_rect = Rect::from_min_size(
            panel_rect.min + vec2(page_margin, 0.0),
            vec2(content_width, (panel_rect.height() - page_margin).max(0.0)),
        );

        let mut content_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        flush(&mut content_ui);

        self.render_toolbar(&mut content_ui, theme, active_module);
        content_ui.add_space(16.0);

        if self.file_loader_rx.is_some() && self.pending_add_count > 0 {
            let msg = BannerMessage {
                tone: BannerTone::Info,
                text: format!("Adding {} image(s)...", self.pending_add_count),
            };
            self.render_banner(&mut content_ui, theme, &msg);
            content_ui.add_space(14.0);
        } else if let Some(msg) = &self.banner {
            self.render_banner(&mut content_ui, theme, msg);
            content_ui.add_space(14.0);
        }

        let workspace_width = content_ui.available_width();
        let workspace_height = content_ui.available_height().max(0.0);
        let has_files = !self.files.is_empty();

        if has_files && workspace_width >= 1080.0 {
            content_ui.allocate_ui_with_layout(
                vec2(workspace_width, workspace_height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let usable_width = (workspace_width - gutter * 2.0).max(0.0);
                    let queue_width = usable_width * 0.25;
                    let center_width = usable_width * 0.50;

                    ui.allocate_ui_with_layout(
                        vec2(queue_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_queue_column(ui, theme, workspace_height);
                        },
                    );
                    ui.add_space(gutter);
                    ui.allocate_ui_with_layout(
                        vec2(center_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            if self.selected_file_id.is_some() {
                                let half = (workspace_height - 12.0) * 0.5;
                                self.render_drop_zone(ui, ctx, theme, half);
                                ui.add_space(12.0);
                                self.render_preview(ui, ctx, theme, half);
                            } else {
                                self.render_drop_zone(ui, ctx, theme, workspace_height);
                            }
                        },
                    );
                    ui.add_space(gutter);
                    let settings_width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        vec2(settings_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_settings_column(ui, theme, workspace_height);
                        },
                    );
                },
            );
        } else if workspace_width >= 900.0 {
            content_ui.allocate_ui_with_layout(
                vec2(workspace_width, workspace_height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let left_width = (workspace_width - gutter) * 0.75;

                    ui.allocate_ui_with_layout(
                        vec2(left_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            let has_preview = has_files && self.selected_file_id.is_some();
                            let drop_height = if has_files {
                                if has_preview {
                                    (workspace_height * 0.17).max(0.0)
                                } else {
                                    (workspace_height * 0.34).max(0.0)
                                }
                            } else {
                                workspace_height.max(0.0)
                            };
                            self.render_drop_zone(ui, ctx, theme, drop_height);
                            if has_preview {
                                ui.add_space(12.0);
                                let preview_height = (workspace_height * 0.17).max(0.0);
                                self.render_preview(ui, ctx, theme, preview_height);
                                ui.add_space(12.0);
                                self.render_queue_column(
                                    ui,
                                    theme,
                                    (workspace_height - drop_height - preview_height - 24.0)
                                        .max(0.0),
                                );
                            } else if has_files {
                                ui.add_space(12.0);
                                self.render_queue_column(
                                    ui,
                                    theme,
                                    (workspace_height - drop_height - 12.0).max(0.0),
                                );
                            }
                        },
                    );
                    ui.add_space(gutter);
                    let right_width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        vec2(right_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_settings_column(ui, theme, workspace_height);
                        },
                    );
                },
            );
        } else {
            self.render_stacked_workspace(&mut content_ui, ctx, theme, workspace_height);
        }
    }
}
