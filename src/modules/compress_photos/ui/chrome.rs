use eframe::egui::{self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, Ui};

use crate::{icons, modules::ModuleKind, theme::AppTheme, ui::components::panel};

use super::{BannerMessage, BannerTone, CompressPhotosPage, compact};

impl CompressPhotosPage {
    pub(super) fn render_toolbar(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
    ) {
        let output_dir = self.last_output_dir.clone();

        panel::card(theme)
            .inner_margin(egui::Margin::symmetric(20, 12))
            .show(ui, |ui| {
                compact(ui);
                ui.horizontal_wrapped(|ui| {
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

                    ui.label(
                        RichText::new("Compress Photos")
                            .size(20.0)
                            .strong()
                            .color(theme.colors.fg),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("{} Change Output", icons::FOLDER))
                                        .size(13.0)
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
                                self.output_dir = Some(dir);
                                self.output_dir_user_set = true;
                            }
                        }

                        if let Some(dir) = &output_dir {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(format!("{} Open Output", icons::FOLDER))
                                            .size(13.0)
                                            .color(theme.colors.fg),
                                    )
                                    .fill(theme.colors.bg_raised)
                                    .stroke(Stroke::new(1.0, theme.colors.border))
                                    .corner_radius(CornerRadius::ZERO),
                                )
                                .clicked()
                            {
                                if let Err(error) = open::that(dir) {
                                    self.banner = Some(BannerMessage {
                                        tone: BannerTone::Error,
                                        text: format!("Could not open output folder: {error}"),
                                    });
                                }
                            }
                        }
                    });
                });
            });
    }

    pub(super) fn render_banner(&self, ui: &mut Ui, theme: &AppTheme, msg: &BannerMessage) {
        let tint = match msg.tone {
            BannerTone::Info => theme.colors.accent,
            BannerTone::Success => theme.colors.positive,
            BannerTone::Error => theme.colors.negative,
        };

        panel::tinted(theme, tint)
            .inner_margin(egui::Margin::symmetric(20, 12))
            .show(ui, |ui| {
                ui.label(RichText::new(&msg.text).size(12.5).color(theme.colors.fg));
            });
    }
}
