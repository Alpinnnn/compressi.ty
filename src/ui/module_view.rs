use eframe::egui::{
    self, Align, Button, CornerRadius, Layout, RichText, ScrollArea, Stroke, Ui, vec2,
};

use crate::{
    icons,
    modules::ModuleKind,
    theme::AppTheme,
    ui::components::{hint, panel},
};

pub fn show(
    ui: &mut Ui,
    _ctx: &egui::Context,
    theme: &AppTheme,
    module: ModuleKind,
    active_module: &mut Option<ModuleKind>,
) {
    let spec = module.spec();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            let max_width = 860.0;
            let available_width = ui.available_width();
            let side_padding = ((available_width - max_width) * 0.5).max(0.0);

            ui.add_space(24.0);

            ui.horizontal(|ui| {
                ui.add_space(side_padding);
                ui.allocate_ui_with_layout(
                    vec2(max_width.min(available_width), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
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
                                    hint::title(ui, theme, spec.title, 22.0, Some(spec.detail));
                                });
                            });
                    },
                );
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side_padding);
                ui.allocate_ui_with_layout(
                    vec2(max_width.min(available_width), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| render_placeholder(ui, theme, spec.accent),
                );
            });

            ui.add_space(24.0);
        });
}

fn render_placeholder(ui: &mut Ui, theme: &AppTheme, accent: egui::Color32) {
    panel::tinted(theme, accent).show(ui, |ui| {
        ui.label(
            RichText::new("Coming soon")
                .size(16.0)
                .strong()
                .color(theme.colors.fg),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("This workspace is not available yet.")
                .size(12.5)
                .color(theme.colors.fg_dim),
        );
    });
}
