use eframe::egui::{self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, Ui};

use crate::{
    icons,
    modules::{
        ModuleKind,
        compress_videos::{engine::VideoEngineController, models::EngineStatus},
    },
    theme::AppTheme,
    ui::components::panel,
};

use super::{
    BannerMessage, BannerTone, CompressVideosPage, compact,
    controls::{render_simple_bar, secondary_button},
};

impl CompressVideosPage {
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
                        RichText::new("Compress Videos")
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
                            self.pick_output_folder(ui.ctx());
                        }

                        if let Some(directory) = &output_dir {
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
                                if let Err(error) = open::that(directory) {
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

    pub(super) fn render_engine_status(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        match engine.status().clone() {
            EngineStatus::Checking => {
                panel::tinted(theme, theme.colors.accent).show(ui, |ui| {
                    ui.label(
                        RichText::new("Preparing video tools...")
                            .size(13.0)
                            .strong()
                            .color(theme.colors.fg),
                    );
                });
            }
            EngineStatus::Downloading { progress, stage } => {
                panel::tinted(theme, theme.colors.accent).show(ui, |ui| {
                    render_simple_bar(ui, theme, progress, &stage);
                });
            }
            EngineStatus::Ready(_) => {}
            EngineStatus::Failed(error) => {
                panel::tinted(theme, theme.colors.negative).show(ui, |ui| {
                    ui.label(
                        RichText::new("Video tools could not be prepared")
                            .size(13.0)
                            .strong()
                            .color(theme.colors.fg),
                    );
                    ui.label(RichText::new(&error).size(11.5).color(theme.colors.fg_dim));
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if secondary_button(ui, theme, "Retry Setup").clicked() {
                            engine.ensure_ready();
                        }
                        if secondary_button(ui, theme, "Refresh Engine").clicked() {
                            engine.refresh();
                        }
                    });
                });
            }
        }
    }
}
