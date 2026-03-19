use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, ScrollArea, Stroke, Ui, vec2,
};

use crate::{icons, modules::ModuleKind, settings::AppSettings, theme::AppTheme, ui::components::panel};

pub fn show(
    ui: &mut Ui,
    _ctx: &egui::Context,
    theme: &AppTheme,
    app_settings: &mut AppSettings,
    active_module: &mut Option<ModuleKind>,
) {
    let max_w = 860.0;
    let avail = ui.available_width();
    let side = ((avail - max_w) * 0.5).max(0.0);

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.add_space(24.0);

            // ── Toolbar ─────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.allocate_ui_with_layout(
                    vec2(max_w.min(avail), 0.0),
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

                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new("Settings")
                                                .size(22.0)
                                                .strong()
                                                .color(theme.colors.fg),
                                        );
                                        ui.label(
                                            RichText::new("Configure global application preferences.")
                                                .size(12.0)
                                                .color(theme.colors.fg_dim),
                                        );
                                    });
                                });
                            });
                    },
                );
            });

            ui.add_space(16.0);

            // ── Settings content ────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.allocate_ui_with_layout(
                    vec2(max_w.min(avail), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        render_output_settings(ui, theme, app_settings);
                    },
                );
            });

            ui.add_space(24.0);
        });
}

fn render_output_settings(ui: &mut Ui, theme: &AppTheme, settings: &mut AppSettings) {
    panel::card(theme)
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.label(
                RichText::new("Output")
                    .size(16.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Configure where compressed files are saved by default.")
                    .size(12.0)
                    .color(theme.colors.fg_dim),
            );
            ui.add_space(16.0);

            // Default Output Folder
            ui.label(
                RichText::new("Default Output Folder")
                    .size(13.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Sets the default destination for all compression modules. You can still override this per-session.")
                    .size(11.0)
                    .color(theme.colors.fg_dim),
            );
            ui.add_space(8.0);

            // Show current path
            panel::inset(theme)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let display_path = match &settings.default_output_folder {
                            Some(dir) => dir.display().to_string(),
                            None => "Not set — uses auto-generated folder (compressity-output/)".to_owned(),
                        };
                        ui.label(
                            RichText::new(format!("{} {}", icons::FOLDER, display_path))
                                .size(12.0)
                                .color(if settings.default_output_folder.is_some() {
                                    theme.colors.fg
                                } else {
                                    theme.colors.fg_muted
                                }),
                        );
                    });
                });
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui
                    .add(
                        Button::new(
                            RichText::new(format!("{} Choose Folder", icons::FOLDER))
                                .size(12.0)
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
                        settings.default_output_folder = Some(dir);
                    }
                }

                if settings.default_output_folder.is_some() {
                    if ui
                        .add(
                            Button::new(
                                RichText::new("Reset to Default")
                                    .size(12.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        settings.default_output_folder = None;
                    }
                }
            });
        });
}
