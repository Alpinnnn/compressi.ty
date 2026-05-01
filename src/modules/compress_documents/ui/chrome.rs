use eframe::egui::{self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, Ui};

use crate::{
    icons,
    modules::ModuleKind,
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{BannerTone, CompressDocumentsPage, compact};

impl CompressDocumentsPage {
    pub(super) fn render_header(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
    ) {
        panel::card(theme)
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                compact(ui);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            Button::new(icons::rich(icons::BACK, 16.0, theme.colors.fg))
                                .fill(theme.colors.bg_raised)
                                .stroke(Stroke::new(1.0, theme.colors.border))
                                .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        *active_module = None;
                    }

                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        hint::title(
                            ui,
                            theme,
                            "Compress Documents",
                            22.0,
                            Some("PDF, Office Open XML, OpenDocument, EPUB, XPS, and Visio packages."),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} queued | {} ready",
                                self.queue.len(),
                                self.queue
                                    .iter()
                                    .filter(|item| matches!(
                                        item.state,
                                        crate::modules::compress_documents::models::DocumentCompressionState::Ready
                                    ))
                                    .count()
                            ))
                            .size(12.0)
                            .color(theme.colors.fg_dim),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(path) = &self.last_output_dir
                            && ui
                                .add(
                                    Button::new(
                                        RichText::new(format!("{} Open Output", icons::FOLDER))
                                            .size(12.0)
                                            .color(theme.colors.fg),
                                    )
                                    .fill(theme.colors.bg_raised)
                                    .stroke(Stroke::new(1.0, theme.colors.border))
                                    .corner_radius(CornerRadius::ZERO),
                                )
                                .clicked()
                        {
                            let _ = open::that(path);
                        }
                    });
                });

                if let Some(banner) = &self.banner {
                    ui.add_space(10.0);
                    let color = match banner.tone {
                        BannerTone::Info => theme.colors.fg_dim,
                        BannerTone::Success => theme.colors.positive,
                        BannerTone::Error => theme.colors.negative,
                    };
                    ui.label(RichText::new(&banner.text).size(12.0).color(color));
                }

                if self.file_loader_rx.is_some() {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new(format!(
                                "Reading {} document(s)",
                                self.pending_add_count
                            ))
                            .size(12.0)
                            .color(Color32::from_gray(190)),
                        );
                    });
                }
            });
    }
}
