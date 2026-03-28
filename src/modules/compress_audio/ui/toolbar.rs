use eframe::egui::{Align, Button, Color32, CornerRadius, Layout, Margin, RichText, Stroke, Ui};

use crate::{
    icons,
    modules::{
        ModuleKind,
        compress_audio::{BannerMessage, BannerTone, CompressAudioPage},
        compress_videos::engine::VideoEngineController,
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

impl CompressAudioPage {
    pub(super) fn render_toolbar(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        engine: &mut VideoEngineController,
    ) {
        panel::card(theme)
            .inner_margin(Margin::symmetric(20, 16))
            .show(ui, |ui| {
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

                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        hint::title(
                            ui,
                            theme,
                            "Compress Audio",
                            22.0,
                            Some(
                                "Drop files, keep Auto Mode on, and let the workspace pick a modern codec for you.",
                            ),
                        );
                        ui.label(
                            RichText::new(
                                "Fast workflow for music, podcasts, voice notes, and batch exports.",
                            )
                            .size(12.0)
                            .color(theme.colors.fg_dim),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(last_output_dir) = &self.last_output_dir {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new("Open Output")
                                            .size(12.0)
                                            .color(theme.colors.fg),
                                    )
                                    .fill(theme.colors.bg_raised)
                                    .stroke(Stroke::new(1.0, theme.colors.border))
                                    .corner_radius(CornerRadius::ZERO),
                                )
                                .clicked()
                            {
                                let _ = open::that(last_output_dir);
                            }
                        }

                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("{} Add Audio", icons::PLAY))
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
                            self.pick_audio_files(engine);
                        }
                    });
                });
            });
    }

    pub(super) fn render_banner(&self, ui: &mut Ui, theme: &AppTheme, message: &BannerMessage) {
        let tint = match message.tone {
            BannerTone::Info => theme.colors.accent,
            BannerTone::Success => theme.colors.positive,
            BannerTone::Error => theme.colors.negative,
        };

        panel::tinted(theme, tint).show(ui, |ui| {
            ui.label(
                RichText::new(&message.text)
                    .size(12.5)
                    .color(theme.colors.fg),
            );
        });
    }

    pub(super) fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        panel::card(theme).show(ui, |ui| {
            hint::title(
                ui,
                theme,
                "Drop Audio Files",
                16.0,
                Some(
                    "Drag files here or use the Add Audio button. Files are analyzed automatically as soon as FFmpeg is ready.",
                ),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("Drag audio here -> choose Auto or Manual -> Start Compression")
                    .size(13.0)
                    .color(theme.colors.fg),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Supported: MP3, M4A, AAC, OPUS, OGG, FLAC, WAV, AIFF, WMA and more.",
                )
                .size(12.0)
                .color(theme.colors.fg_dim),
            );
            ui.add_space(12.0);

            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        Button::new(
                            RichText::new("Choose Audio Files")
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
                    self.pick_audio_files(engine);
                }

                if ui
                    .add(
                        Button::new(
                            RichText::new(format!(
                                "{} Output: {}",
                                icons::FOLDER,
                                self.output_dir
                                    .as_ref()
                                    .map(|path| path.display().to_string())
                                    .unwrap_or_else(|| "Auto".to_owned())
                            ))
                            .size(12.0)
                            .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    self.output_dir = Some(folder);
                    self.output_dir_user_set = true;
                }

                if self.output_dir_user_set
                    && ui
                        .add(
                            Button::new(
                                RichText::new("Use Auto Output")
                                    .size(12.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                {
                    self.output_dir = None;
                    self.output_dir_user_set = false;
                }
            });
        });
    }
}
