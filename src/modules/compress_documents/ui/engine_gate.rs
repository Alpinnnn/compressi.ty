use eframe::egui::{self, Button, Color32, CornerRadius, ProgressBar, RichText, Stroke, Ui};

use crate::{
    modules::compress_documents::engine::{DocumentEngineController, DocumentEngineStatus},
    theme::AppTheme,
    ui::components::{hint, panel},
};

pub(super) fn render_document_engine_gate(
    ui: &mut Ui,
    ctx: &egui::Context,
    theme: &AppTheme,
    document_engine: &mut DocumentEngineController,
) -> bool {
    if let DocumentEngineStatus::Ready(inventory) = document_engine.status()
        && inventory.is_ready()
    {
        return true;
    }

    panel::card(theme)
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            hint::title(
                ui,
                theme,
                "Document Engines Required",
                16.0,
                Some("Document compression is disabled until Ghostscript and 7-Zip are ready."),
            );
            ui.add_space(12.0);

            match document_engine.status().clone() {
                DocumentEngineStatus::Checking => render_checking(ui, theme),
                DocumentEngineStatus::Downloading { progress, stage } => {
                    render_downloading(ui, theme, progress, &stage)
                }
                DocumentEngineStatus::Failed(error) => {
                    render_failed(ui, ctx, theme, document_engine, &error)
                }
                DocumentEngineStatus::Ready(_) => render_failed(
                    ui,
                    ctx,
                    theme,
                    document_engine,
                    "The required document engines are not complete yet.",
                ),
            }
        });

    false
}

fn render_checking(ui: &mut Ui, theme: &AppTheme) {
    panel::tinted(theme, theme.colors.accent)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new("Checking bundled, managed, and system document engines...")
                        .size(12.5)
                        .color(theme.colors.fg),
                );
            });
        });
}

fn render_downloading(ui: &mut Ui, theme: &AppTheme, progress: f32, stage: &str) {
    panel::tinted(theme, theme.colors.accent)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.label(RichText::new(stage).size(12.5).color(theme.colors.fg));
            ui.add_space(8.0);
            ui.add(
                ProgressBar::new(progress.clamp(0.0, 1.0))
                    .text(format!("{:.0}%", progress.clamp(0.0, 1.0) * 100.0))
                    .animate(true),
            );
        });
}

fn render_failed(
    ui: &mut Ui,
    ctx: &egui::Context,
    theme: &AppTheme,
    document_engine: &mut DocumentEngineController,
    error: &str,
) {
    panel::tinted(theme, theme.colors.negative)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.label(RichText::new(error).size(12.5).color(theme.colors.fg));
        });

    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(
                Button::new(
                    RichText::new("Install Document Engines")
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
            document_engine.ensure_ready();
        }

        if document_engine.needs_administrator_restart()
            && ui
                .add(
                    Button::new(
                        RichText::new("Restart as Administrator")
                            .size(12.0)
                            .strong()
                            .color(theme.colors.fg),
                    )
                    .fill(theme.colors.bg_raised)
                    .stroke(Stroke::new(1.0, theme.colors.border_focus))
                    .corner_radius(CornerRadius::ZERO),
                )
                .clicked()
        {
            restart_as_administrator(ctx, document_engine);
        }
    });
}

fn restart_as_administrator(ctx: &egui::Context, document_engine: &mut DocumentEngineController) {
    match crate::process_lifecycle::restart_as_administrator() {
        Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        Err(error) => document_engine.record_error(format!(
            "Could not restart Compressi.ty as Administrator: {error}"
        )),
    }
}
