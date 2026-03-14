use eframe::egui::{
    self, Align, Button, CornerRadius, Layout, RichText, ScrollArea, Stroke, Ui, vec2,
};

use crate::{icons, modules::ModuleKind, theme::AppTheme, ui::components::panel};

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

            let max_w = 860.0;
            let avail = ui.available_width();
            let side = ((avail - max_w) * 0.5).max(0.0);

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
                                            RichText::new(spec.title)
                                                .size(22.0)
                                                .strong()
                                                .color(theme.colors.fg),
                                        );
                                        ui.label(
                                            RichText::new(spec.detail)
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

            // ── Content ─────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.allocate_ui_with_layout(
                    vec2(max_w.min(avail), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        if avail >= 720.0 {
                            ui.horizontal_top(|ui| {
                                ui.allocate_ui_with_layout(
                                    vec2(ui.available_width() * 0.55, 0.0),
                                    Layout::top_down(Align::Min),
                                    |ui| render_overview(ui, theme, spec.title, spec.accent),
                                );
                                ui.add_space(12.0);
                                ui.allocate_ui_with_layout(
                                    vec2(ui.available_width(), 0.0),
                                    Layout::top_down(Align::Min),
                                    |ui| render_roadmap(ui, theme, spec.accent),
                                );
                            });
                        } else {
                            render_overview(ui, theme, spec.title, spec.accent);
                            ui.add_space(12.0);
                            render_roadmap(ui, theme, spec.accent);
                        }
                    },
                );
            });

            ui.add_space(24.0);
        });
}

// ─── Panels ─────────────────────────────────────────────────────────────────

fn render_overview(ui: &mut Ui, theme: &AppTheme, title: &str, accent: egui::Color32) {
    panel::tinted(theme, accent).show(ui, |ui| {
        ui.label(
            RichText::new("Overview")
                .size(16.0)
                .strong()
                .color(theme.colors.fg),
        );
        ui.add_space(8.0);
        bullet(
            ui,
            theme,
            &format!("{title} has a dedicated shell and routing ready for implementation."),
        );
        bullet(
            ui,
            theme,
            "The interface keeps panels compact with denser spacing.",
        );
        bullet(
            ui,
            theme,
            "Plug the real workflow into this layout without changing the shared visual system.",
        );
    });
}

fn render_roadmap(ui: &mut Ui, theme: &AppTheme, accent: egui::Color32) {
    panel::card(theme).show(ui, |ui| {
        ui.label(
            RichText::new("Next up")
                .size(16.0)
                .strong()
                .color(theme.colors.fg),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for label in [
                "Responsive layout",
                "Local-first workflow",
                "Reusable components",
            ] {
                panel::chip_accent(theme, accent).show(ui, |ui| {
                    ui.label(RichText::new(label).size(11.5).color(theme.colors.fg_dim));
                });
            }
        });
        ui.add_space(8.0);
        bullet(
            ui,
            theme,
            "Connect module logic to inputs, presets, and progress reporting.",
        );
        bullet(
            ui,
            theme,
            "Reuse compact panels so new features feel native to the shell.",
        );
    });
}

fn bullet(ui: &mut Ui, theme: &AppTheme, text: &str) {
    ui.horizontal_top(|ui| {
        ui.label(RichText::new("·").size(16.0).color(theme.colors.fg_muted));
        ui.add_space(4.0);
        ui.label(RichText::new(text).size(12.5).color(theme.colors.fg_dim));
    });
}
