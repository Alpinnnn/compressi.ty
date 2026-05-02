use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, Ui, vec2,
};

use crate::{
    icons, modules::compress_documents::models::DocumentCompressionState, theme::AppTheme,
    ui::components::panel,
};

use super::{CompressDocumentsPage, compact, flush};

impl CompressDocumentsPage {
    pub(super) fn render_workspace(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let workspace_width = ui.available_width();
        if workspace_width >= 900.0 {
            ui.allocate_ui_with_layout(
                vec2(workspace_width, height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let usable_width = (workspace_width - gutter * 2.0).max(0.0);
                    let queue_width = usable_width * 0.28;
                    let center_width = usable_width * 0.38;
                    let actions_height = 108.0;
                    let drop_height = (height - actions_height - 12.0).max(0.0);

                    ui.allocate_ui_with_layout(
                        vec2(queue_width, height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_queue(ui, theme, height);
                        },
                    );
                    ui.add_space(gutter);
                    ui.allocate_ui_with_layout(
                        vec2(center_width, height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_drop_zone(ui, ctx, theme, drop_height);
                            ui.add_space(12.0);
                            self.render_action_panel(ui, theme, actions_height);
                        },
                    );
                    ui.add_space(gutter);
                    let settings_width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        vec2(settings_width, height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_settings_panel(ui, theme, height);
                        },
                    );
                },
            );
        } else {
            self.render_stacked_workspace(ui, ctx, theme, height);
        }
    }

    fn render_stacked_workspace(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let gap = 12.0;
        let drop_height = if self.queue.is_empty() {
            (height * 0.34).max(210.0)
        } else {
            (height * 0.24).max(170.0)
        };
        self.render_drop_zone(ui, ctx, theme, drop_height);
        ui.add_space(gap);
        self.render_action_panel(ui, theme, 108.0);
        ui.add_space(gap);
        self.render_queue(ui, theme, (height * 0.32).max(190.0));
        ui.add_space(gap);
        self.render_settings_panel(ui, theme, (height - drop_height - gap * 3.0).max(260.0));
    }

    pub(super) fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let hovering = ctx.input(|input| !input.raw.hovered_files.is_empty());
        let frame = if hovering {
            panel::tinted(theme, theme.colors.accent)
        } else {
            panel::card(theme)
        };

        frame.show(ui, |ui| {
            compact(ui);
            ui.set_min_height((height - 40.0).max(0.0));
            ui.vertical_centered(|ui| {
                ui.add_space((height * 0.15).max(8.0));
                ui.label(icons::rich(icons::DOCUMENT, 42.0, theme.colors.accent));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Drop documents")
                        .size(20.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                ui.label(
                    RichText::new("PDF, Word, PowerPoint, Excel, ODF, EPUB, XPS")
                        .size(12.0)
                        .color(theme.colors.fg_dim),
                );
                ui.add_space(14.0);
                if ui
                    .add(
                        Button::new(
                            RichText::new("Select Files")
                                .size(13.0)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(theme.colors.accent)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::ZERO)
                        .min_size(vec2(148.0, 34.0)),
                    )
                    .clicked()
                {
                    self.select_documents();
                }
            });
        });
    }

    pub(super) fn render_action_panel(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        let has_ready = self.has_compressible_documents();
        let has_finished = self.queue.iter().any(|item| {
            matches!(
                item.state,
                DocumentCompressionState::Completed(_)
                    | DocumentCompressionState::Failed(_)
                    | DocumentCompressionState::Cancelled
            )
        });

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                if ui
                    .add_enabled(
                        has_ready && self.active_batch.is_none(),
                        Button::new(
                            RichText::new(format!("{} Compress All", icons::PLAY))
                                .size(13.0)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(theme.colors.accent)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::ZERO)
                        .min_size(vec2(ui.available_width(), 34.0)),
                    )
                    .clicked()
                {
                    self.start_all_compression();
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.active_batch.is_some(),
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
                        self.cancel_compression();
                    }

                    if ui
                        .add_enabled(
                            has_finished && self.active_batch.is_none(),
                            Button::new(
                                RichText::new("Clear Finished")
                                    .size(12.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        self.clear_finished();
                    }
                });
            });
    }
}
