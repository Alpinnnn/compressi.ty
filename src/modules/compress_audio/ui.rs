mod chrome;
mod controls;
mod details;
mod helpers;
mod queue;
mod settings_panel;
mod widgets;
mod workspace;

use eframe::egui::{self, Align, Layout, Rect, Ui, vec2};

use crate::{
    modules::{
        ModuleKind,
        compress_audio::models::AudioCompressionState,
        compress_videos::{engine::VideoEngineController, models::EngineStatus},
    },
    settings::AppSettings,
    theme::AppTheme,
};

use self::chrome::render_banner;
pub(super) use super::{BannerMessage, BannerTone, CompressAudioPage};

const MEDIA_PANEL_GAP: f32 = 12.0;
const DETAILS_COLLAPSED_HEIGHT: f32 = 168.0;
const DETAILS_EXPANDED_HEIGHT: f32 = 320.0;
const DROP_ZONE_MIN_HEIGHT: f32 = 132.0;

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

pub(super) fn is_audio_settings_editable(state: &AudioCompressionState) -> bool {
    matches!(state, AudioCompressionState::Ready)
}

impl CompressAudioPage {
    fn media_panel_heights(&self, available_height: f32) -> (f32, f32) {
        let preferred_details_height = if self.track_info_open {
            DETAILS_EXPANDED_HEIGHT
        } else {
            DETAILS_COLLAPSED_HEIGHT
        };
        let max_details_height =
            (available_height - MEDIA_PANEL_GAP - DROP_ZONE_MIN_HEIGHT).max(0.0);
        let details_height = preferred_details_height.min(max_details_height).max(0.0);
        let gap = if details_height > 0.0 {
            MEDIA_PANEL_GAP
        } else {
            0.0
        };
        let drop_height = (available_height - details_height - gap).max(0.0);

        (drop_height, details_height)
    }

    fn pick_audio_files(&mut self, engine: &mut VideoEngineController) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                "Audio",
                &[
                    "aac", "aif", "aiff", "flac", "m4a", "m4b", "mka", "mp2", "mp3", "oga", "ogg",
                    "opus", "wav", "wma",
                ],
            )
            .pick_files()
        {
            self.add_paths(paths, engine);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context, engine: &mut VideoEngineController) {
        let paths = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });

        if !paths.is_empty() {
            self.add_paths(paths, engine);
        }
    }

    /// Renders the full audio compression workspace.
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
            self.output_dir = app_settings.preferred_audio_output_folder();
        }
        self.handle_dropped_files(ctx, engine);
        self.flush_deferred_paths(engine);
        flush(ui);

        let panel_rect = ui.max_rect();
        let available_width = panel_rect.width();
        let page_margin = if available_width >= 1280.0 {
            28.0
        } else if available_width >= 960.0 {
            22.0
        } else if available_width >= 720.0 {
            16.0
        } else {
            12.0
        };
        let content_width = (available_width - page_margin * 2.0).max(0.0);
        let bottom_padding = page_margin;

        let content_rect = Rect::from_min_size(
            panel_rect.min + vec2(page_margin, 0.0),
            vec2(
                content_width,
                (panel_rect.height() - bottom_padding).max(0.0),
            ),
        );

        let mut content_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        flush(&mut content_ui);

        self.render_toolbar(&mut content_ui, theme, active_module);
        content_ui.add_space(16.0);

        if let Some(message) = &self.banner {
            render_banner(&mut content_ui, theme, message);
            content_ui.add_space(14.0);
        }

        if !matches!(engine.status(), EngineStatus::Ready(_)) {
            self.render_engine_status(&mut content_ui, theme, engine);
            content_ui.add_space(12.0);
        }

        let workspace_width = content_ui.available_width();
        let workspace_height = content_ui.available_height().max(0.0);
        let has_files = !self.queue.is_empty();

        if has_files && workspace_width >= 900.0 {
            content_ui.allocate_ui_with_layout(
                vec2(workspace_width, workspace_height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let usable_width = (workspace_width - gutter * 2.0).max(0.0);
                    let queue_width = usable_width * 0.28;
                    let center_width = usable_width * 0.38;
                    let queue_actions_height = 108.0;
                    let queue_height = (workspace_height - queue_actions_height - 12.0).max(0.0);
                    let (drop_height, details_height) = self.media_panel_heights(workspace_height);

                    ui.allocate_ui_with_layout(
                        vec2(queue_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_queue(ui, theme, queue_height, engine);
                            ui.add_space(12.0);
                            self.render_actions(ui, theme, queue_actions_height, engine);
                        },
                    );
                    ui.add_space(gutter);
                    ui.allocate_ui_with_layout(
                        vec2(center_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_drop_zone(ui, ctx, theme, drop_height, engine);
                            if details_height > 0.0 {
                                ui.add_space(MEDIA_PANEL_GAP);
                                self.render_details_panel(ui, theme, details_height, engine);
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
                            self.render_settings_panel(ui, theme, workspace_height, engine);
                        },
                    );
                },
            );
        } else {
            let drop_height = if has_files {
                let media_height = (workspace_height * 0.54)
                    .max(DROP_ZONE_MIN_HEIGHT + DETAILS_COLLAPSED_HEIGHT + MEDIA_PANEL_GAP)
                    .min(workspace_height);
                let (drop_height, _) = self.media_panel_heights(media_height);
                drop_height
            } else {
                workspace_height
            };
            self.render_drop_zone(&mut content_ui, ctx, theme, drop_height.max(0.0), engine);
            if has_files {
                content_ui.add_space(MEDIA_PANEL_GAP);
                let media_height = (workspace_height * 0.54)
                    .max(DROP_ZONE_MIN_HEIGHT + DETAILS_COLLAPSED_HEIGHT + MEDIA_PANEL_GAP)
                    .min(workspace_height);
                let (_, details_height) = self.media_panel_heights(media_height);
                self.render_details_panel(&mut content_ui, theme, details_height, engine);
                content_ui.add_space(MEDIA_PANEL_GAP);
                let remaining_height =
                    (workspace_height - drop_height - details_height - MEDIA_PANEL_GAP * 2.0)
                        .max(0.0);
                let actions_height = ((remaining_height * 0.20).clamp(60.0, 96.0))
                    .min((remaining_height - MEDIA_PANEL_GAP).max(0.0));
                let queue_and_settings_height =
                    (remaining_height - actions_height - MEDIA_PANEL_GAP).max(0.0);
                let queue_height = queue_and_settings_height * 0.38;
                let settings_height = queue_and_settings_height - queue_height;
                self.render_queue(&mut content_ui, theme, queue_height, engine);
                content_ui.add_space(MEDIA_PANEL_GAP);
                self.render_actions(&mut content_ui, theme, actions_height.max(0.0), engine);
                content_ui.add_space(MEDIA_PANEL_GAP);
                self.render_settings_panel(
                    &mut content_ui,
                    theme,
                    settings_height.max(0.0),
                    engine,
                );
            }
        }

        self.render_cancel_all_confirm(ctx, theme);
    }

    fn render_cancel_all_confirm(&mut self, ctx: &egui::Context, theme: &AppTheme) {
        if !self.show_cancel_all_confirm {
            return;
        }

        egui::Area::new(egui::Id::new("audio_cancel_all_overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_TOP, vec2(0.0, 0.0))
            .interactable(false)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                let overlay_fill = theme.colors.bg_base.linear_multiply(0.82);
                ui.painter()
                    .rect_filled(screen, egui::CornerRadius::ZERO, overlay_fill);
            });

        egui::Window::new("Cancel all compression")
            .id(egui::Id::new("audio_cancel_all_window"))
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.surface)
                    .stroke(egui::Stroke::new(1.0, theme.colors.border))
                    .corner_radius(egui::CornerRadius::ZERO)
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(
                    egui::RichText::new("Cancel All")
                        .size(16.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Are you sure?")
                        .size(12.5)
                        .color(theme.colors.fg_dim),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Yes, Cancel All")
                                    .size(12.0)
                                    .strong()
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.negative)
                            .stroke(egui::Stroke::new(1.0, theme.colors.negative))
                            .corner_radius(egui::CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        self.confirm_cancel_all();
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Keep Running")
                                    .size(12.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.bg_raised)
                            .stroke(egui::Stroke::new(1.0, theme.colors.border))
                            .corner_radius(egui::CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        self.dismiss_cancel_all();
                    }
                });
            });
    }
}
