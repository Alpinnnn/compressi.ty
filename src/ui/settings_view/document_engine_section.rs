use eframe::egui::{self, Button, Color32, CornerRadius, RichText, Stroke, Ui, vec2};

use crate::{
    modules::compress_documents::engine::{
        DocumentEngineController, DocumentEngineInfo, DocumentEngineInventory, DocumentEngineStatus,
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

pub(super) fn render_document_engine_settings(
    ui: &mut Ui,
    ctx: &egui::Context,
    theme: &AppTheme,
    document_engine: &mut DocumentEngineController,
) {
    panel::card(theme)
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            hint::title(
                ui,
                theme,
                "Document Engines",
                16.0,
                Some(
                    "Bundled Ghostscript, qpdf, and 7-Zip stay with the app. Managed updates are stored in app data.",
                ),
            );
            ui.add_space(16.0);

            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !document_engine.is_busy(),
                        Button::new(
                            RichText::new("Refresh Versions")
                                .size(12.0)
                                .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                {
                    document_engine.refresh();
                }

                if ui
                    .add_enabled(
                        !document_engine.is_busy(),
                        Button::new(
                            RichText::new("Update to Latest")
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
                    document_engine.update_to_latest();
                }

                if ui
                    .add_enabled(
                        !document_engine.is_busy()
                            && !document_engine.managed_inventory().infos().is_empty(),
                        Button::new(
                            RichText::new("Use Bundled Engine")
                                .size(12.0)
                                .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                {
                    document_engine.use_bundled_engine();
                }
            });

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                render_document_engine_folder_buttons(ui, theme, document_engine);
            });

            ui.add_space(12.0);
            egui::CollapsingHeader::new("Document Engine Details")
                .id_salt("settings_document_engine_details")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    render_document_engine_activity(ui, theme, document_engine.status());

                    if let Some(error) = document_engine.last_error() {
                        ui.add_space(8.0);
                        panel::tinted(theme, theme.colors.negative)
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(RichText::new(error).size(11.5).color(theme.colors.fg));
                            });
                        if document_engine.needs_administrator_restart() {
                            ui.add_space(8.0);
                            if ui
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
                        }
                    }

                    ui.add_space(12.0);
                    render_document_engine_inventory_card(
                        ui,
                        theme,
                        "Active Engines",
                        document_engine.active_inventory(),
                        true,
                    );
                    ui.add_space(8.0);
                    render_document_engine_inventory_card(
                        ui,
                        theme,
                        "Bundled Engines",
                        Some(document_engine.bundled_inventory()),
                        false,
                    );
                    ui.add_space(8.0);
                    render_document_engine_inventory_card(
                        ui,
                        theme,
                        "Managed Update",
                        Some(document_engine.managed_inventory()),
                        false,
                    );
                    ui.add_space(8.0);
                    render_document_engine_inventory_card(
                        ui,
                        theme,
                        "System PATH",
                        Some(document_engine.system_inventory()),
                        false,
                    );
                });
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

fn render_document_engine_folder_buttons(
    ui: &mut Ui,
    theme: &AppTheme,
    document_engine: &DocumentEngineController,
) {
    if ui
        .add(folder_button(theme, "Open Managed PDF Folder"))
        .clicked()
        && let Some(dir) = document_engine.managed_pdf_engine_dir()
    {
        let _ = std::fs::create_dir_all(&dir);
        let _ = open::that(dir);
    }

    if ui
        .add(folder_button(theme, "Open Managed Package Folder"))
        .clicked()
        && let Some(dir) = document_engine.managed_package_engine_dir()
    {
        let _ = std::fs::create_dir_all(&dir);
        let _ = open::that(dir);
    }

    if ui
        .add(folder_button(theme, "Open Install PDF Folder"))
        .clicked()
        && let Some(dir) = document_engine.bundled_pdf_engine_dir()
    {
        let _ = open::that(dir);
    }

    if ui
        .add(folder_button(theme, "Open Install Package Folder"))
        .clicked()
        && let Some(dir) = document_engine.bundled_package_engine_dir()
    {
        let _ = open::that(dir);
    }
}

fn folder_button(theme: &AppTheme, label: &str) -> Button<'static> {
    Button::new(
        RichText::new(label.to_owned())
            .size(12.0)
            .color(theme.colors.fg),
    )
    .fill(theme.colors.bg_raised)
    .stroke(Stroke::new(1.0, theme.colors.border))
    .corner_radius(CornerRadius::ZERO)
}

fn render_document_engine_activity(ui: &mut Ui, theme: &AppTheme, status: &DocumentEngineStatus) {
    match status {
        DocumentEngineStatus::Checking => {
            panel::tinted(theme, theme.colors.accent)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Inspecting document engines...")
                            .size(12.5)
                            .strong()
                            .color(theme.colors.fg),
                    );
                });
        }
        DocumentEngineStatus::Downloading { progress, stage } => {
            panel::tinted(theme, theme.colors.accent)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.label(RichText::new(stage).size(12.0).color(theme.colors.fg));
                    ui.add_space(6.0);
                    let bar_width = ui.available_width().max(180.0);
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(bar_width, 10.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(2), theme.colors.bg_base);
                    let fill = egui::Rect::from_min_size(
                        rect.min,
                        vec2(rect.width() * progress.clamp(0.0, 1.0), rect.height()),
                    );
                    ui.painter()
                        .rect_filled(fill, CornerRadius::same(2), theme.colors.accent);
                });
        }
        DocumentEngineStatus::Ready(_) | DocumentEngineStatus::Failed(_) => {}
    }
}

fn render_document_engine_inventory_card(
    ui: &mut Ui,
    theme: &AppTheme,
    title: &str,
    inventory: Option<&DocumentEngineInventory>,
    active: bool,
) {
    panel::inset(theme)
        .fill(if active {
            theme.mix(theme.colors.surface, theme.colors.accent, 0.08)
        } else {
            theme.colors.bg_raised
        })
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .size(13.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_space(6.0);

            if let Some(inventory) = inventory {
                render_document_engine_row(
                    ui,
                    theme,
                    "Ghostscript",
                    inventory.ghostscript.as_ref(),
                );
                render_document_engine_row(ui, theme, "qpdf", inventory.qpdf.as_ref());
                render_document_engine_row(ui, theme, "7-Zip", inventory.seven_zip.as_ref());
            } else {
                ui.label(
                    RichText::new("Not available on this machine yet.")
                        .size(11.5)
                        .color(theme.colors.fg_muted),
                );
            }
        });
}

fn render_document_engine_row(
    ui: &mut Ui,
    theme: &AppTheme,
    label: &str,
    info: Option<&DocumentEngineInfo>,
) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(12.0)
                .strong()
                .color(theme.colors.fg),
        );
        if let Some(info) = info {
            ui.add_space(6.0);
            ui.label(
                RichText::new(info.source.label())
                    .size(10.5)
                    .color(theme.colors.accent),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(info.kind.label())
                    .size(10.5)
                    .color(theme.colors.fg_muted),
            );
        }
    });

    if let Some(info) = info {
        ui.label(
            RichText::new(&info.version)
                .size(10.5)
                .color(theme.colors.fg_dim),
        );
        ui.label(
            RichText::new(info.path.display().to_string())
                .size(10.5)
                .color(theme.colors.fg_dim),
        );
    } else {
        ui.label(
            RichText::new("Not detected.")
                .size(10.5)
                .color(theme.colors.fg_muted),
        );
    }
}
