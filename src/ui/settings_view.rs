mod document_engine_section;
mod engine_section;
mod output_section;
mod processing_section;

use eframe::egui::{
    self, Align, Button, CornerRadius, Layout, RichText, ScrollArea, Stroke, Ui, vec2,
};

use crate::{
    icons,
    modules::{
        ModuleKind, compress_documents::engine::DocumentEngineController,
        compress_videos::engine::VideoEngineController,
    },
    settings::AppSettings,
    theme::AppTheme,
    ui::components::{hint, panel},
};

use self::{
    document_engine_section::render_document_engine_settings,
    engine_section::render_engine_settings, output_section::render_output_settings,
    processing_section::render_processing_settings,
};

pub fn show(
    ui: &mut Ui,
    ctx: &egui::Context,
    theme: &AppTheme,
    app_settings: &mut AppSettings,
    active_module: &mut Option<ModuleKind>,
    video_engine: &mut VideoEngineController,
    document_engine: &mut DocumentEngineController,
) {
    let max_width = 860.0;
    let available_width = ui.available_width();
    let side_padding = ((available_width - max_width) * 0.5).max(0.0);

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(24.0);

            ui.horizontal(|ui| {
                ui.add_space(side_padding);
                ui.allocate_ui_with_layout(
                    vec2(max_width.min(available_width), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| render_header(ui, theme, active_module),
                );
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_padding);
                ui.allocate_ui_with_layout(
                    vec2(max_width.min(available_width), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        render_output_settings(ui, theme, app_settings);
                        ui.add_space(16.0);
                        render_processing_settings(ui, theme, app_settings);
                        ui.add_space(16.0);
                        render_engine_settings(ui, theme, video_engine);
                        ui.add_space(16.0);
                        render_document_engine_settings(ui, ctx, theme, document_engine);
                    },
                );
            });

            ui.add_space(24.0);
        });
}

fn render_header(ui: &mut Ui, theme: &AppTheme, active_module: &mut Option<ModuleKind>) {
    panel::card(theme)
        .inner_margin(egui::Margin::symmetric(20, 16))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
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

                ui.add_space(12.0);
                ui.vertical(|ui| {
                    hint::title(
                        ui,
                        theme,
                        "Settings",
                        22.0,
                        Some("Configure global defaults and the bundled video engine."),
                    );
                });
            });
        });
}
