use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, Ui, vec2,
};

use crate::{
    icons,
    modules::compress_videos::{
        engine::VideoEngineController,
        models::{EngineStatus, VideoCompressionState},
    },
    theme::AppTheme,
    ui::components::panel,
};

use super::{CompressVideosPage, compact};

impl CompressVideosPage {
    pub(super) fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
    ) {
        let hovering = ctx.input(|input| !input.raw.hovered_files.is_empty());
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
                                        .filter(|item| {
                                            matches!(item.state, VideoCompressionState::Ready)
                                        })
                                        .count();
                                    format!("{ready_count} video(s) ready")
                                } else {
                                    "Drop videos or folders here".to_owned()
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
                                    RichText::new("Video tools are being prepared...")
                                        .size(11.0)
                                        .color(theme.colors.fg_dim),
                                );
                            }
                            if !self.pending_probes.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(format!(
                                        "Probing {} video(s)...",
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

    pub(super) fn render_actions(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
        use_hardware_acceleration: bool,
    ) {
        let accent = theme.colors.accent;
        let has_ready_videos = self
            .queue
            .iter()
            .any(|item| matches!(item.state, VideoCompressionState::Ready));

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                if ui
                    .add_enabled(
                        has_ready_videos,
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
                    self.start_batch_compression(engine, use_hardware_acceleration);
                }

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.has_pending_compression(),
                            Button::new(
                                RichText::new("Cancel All")
                                    .size(12.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.mix(theme.colors.surface, theme.colors.caution, 0.1))
                            .stroke(Stroke::new(
                                1.0,
                                theme.mix(theme.colors.border, theme.colors.caution, 0.24),
                            ))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        self.request_cancel_all();
                    }

                    if ui
                        .add_enabled(
                            !self.has_pending_compression() && !self.queue.is_empty(),
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
                        self.reset_preview_state();
                    }
                });

                if self.has_pending_compression() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(
                            RichText::new("Compressing...")
                                .size(11.0)
                                .color(theme.colors.fg_dim),
                        );
                    });
                }
            });
    }
}
