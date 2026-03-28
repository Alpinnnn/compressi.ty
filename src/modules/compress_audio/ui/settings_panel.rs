use eframe::egui::{self, Button, Color32, CornerRadius, ProgressBar, RichText, Stroke, Ui};

use crate::{
    modules::{
        compress_audio::{
            logic::estimate_output,
            models::{AudioEstimate, AudioFormat, AudioWorkflowMode},
        },
        compress_videos::engine::VideoEngineController,
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{
    CompressAudioPage,
    helpers::{
        format_available, format_bytes, overall_progress, selectable_option_row, toggle_button,
    },
};

impl CompressAudioPage {
    pub(super) fn render_settings_column(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        let selected_metadata = self
            .selected_id
            .and_then(|id| self.find_item(id))
            .and_then(|item| item.metadata.clone());
        let selected_analysis = self
            .selected_id
            .and_then(|id| self.find_item(id))
            .and_then(|item| item.analysis.clone());
        let estimate = selected_metadata.as_ref().and_then(|metadata| {
            engine
                .active_info()
                .map(|engine_info| estimate_output(metadata, &self.settings, &engine_info.encoders))
        });

        panel::card(theme).show(ui, |ui| {
            hint::title(
                ui,
                theme,
                "Settings Panel",
                16.0,
                Some(
                    "Keep Auto Mode on for the simplest workflow, or switch to Manual when you need a specific output format.",
                ),
            );
            ui.add_space(12.0);

            if let Some(analysis) = selected_analysis.as_ref() {
                panel::tinted(theme, theme.colors.accent).show(ui, |ui| {
                    ui.label(
                        RichText::new(&analysis.headline)
                            .size(13.0)
                            .strong()
                            .color(theme.colors.fg),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&analysis.detail)
                            .size(12.0)
                            .color(theme.colors.fg_dim),
                    );
                });
                ui.add_space(12.0);
            }

            self.render_mode_toggle(ui, theme);
            ui.add_space(12.0);

            match self.settings.mode {
                AudioWorkflowMode::Auto => self.render_auto_controls(ui, theme),
                AudioWorkflowMode::Manual => {
                    self.render_manual_controls(ui, theme, engine.active_info())
                }
            }

            ui.add_space(12.0);
            self.render_extra_options(ui, theme);
            ui.add_space(12.0);
            self.render_output_preview(ui, theme, estimate.as_ref(), selected_metadata.as_ref());
            ui.add_space(12.0);
            self.render_actions(ui, theme, engine);
        });
    }

    fn render_mode_toggle(&mut self, ui: &mut Ui, theme: &AppTheme) {
        ui.horizontal(|ui| {
            if toggle_button(
                ui,
                theme,
                self.settings.mode == AudioWorkflowMode::Auto,
                "Auto (recommended)",
            )
            .clicked()
            {
                self.settings.mode = AudioWorkflowMode::Auto;
            }

            if toggle_button(
                ui,
                theme,
                self.settings.mode == AudioWorkflowMode::Manual,
                "Manual",
            )
            .clicked()
            {
                self.settings.mode = AudioWorkflowMode::Manual;
            }
        });
    }

    fn render_auto_controls(&mut self, ui: &mut Ui, theme: &AppTheme) {
        hint::title(
            ui,
            theme,
            "Smart Mode",
            13.0,
            Some(
                "Auto chooses between AAC and OPUS based on the file type, then falls back to MP3 only when needed.",
            ),
        );
        ui.add_space(8.0);

        for preset in [
            crate::modules::compress_audio::models::AudioAutoPreset::HighQuality,
            crate::modules::compress_audio::models::AudioAutoPreset::Balanced,
            crate::modules::compress_audio::models::AudioAutoPreset::SmallSize,
        ] {
            let selected = self.settings.auto_preset == preset;
            if toggle_button(ui, theme, selected, preset.label()).clicked() {
                self.settings.auto_preset = preset;
            }
            ui.label(
                RichText::new(preset.detail())
                    .size(11.5)
                    .color(theme.colors.fg_dim),
            );
            ui.add_space(6.0);
        }
    }

    fn render_manual_controls(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine_info: Option<&crate::modules::compress_videos::models::EngineInfo>,
    ) {
        hint::title(
            ui,
            theme,
            "Manual Mode",
            13.0,
            Some(
                "Pick the output format yourself, then fine tune bitrate or advanced conversion options.",
            ),
        );
        ui.add_space(8.0);

        egui::ComboBox::from_id_salt("audio_manual_format")
            .selected_text(self.settings.manual_format.label())
            .show_ui(ui, |ui| {
                for format in [
                    AudioFormat::Aac,
                    AudioFormat::Opus,
                    AudioFormat::Mp3,
                    AudioFormat::Flac,
                ] {
                    let available = engine_info
                        .map(|engine| format_available(format, &engine.encoders))
                        .unwrap_or(true);
                    if available {
                        ui.selectable_value(
                            &mut self.settings.manual_format,
                            format,
                            format.label(),
                        );
                    }
                }
            });

        ui.add_space(8.0);
        let bitrate_enabled = !self.settings.manual_format.is_lossless();
        ui.add_enabled_ui(bitrate_enabled, |ui| {
            ui.label(
                RichText::new(format!(
                    "Bitrate: {} kbps",
                    self.settings.manual_bitrate_kbps
                ))
                .size(12.0)
                .color(theme.colors.fg),
            );
            ui.add(
                egui::Slider::new(&mut self.settings.manual_bitrate_kbps, 24..=320).suffix(" kbps"),
            );
        });
        if !bitrate_enabled {
            ui.label(
                RichText::new("FLAC keeps audio lossless, so bitrate is handled automatically.")
                    .size(11.5)
                    .color(theme.colors.fg_dim),
            );
        }

        ui.add_space(8.0);
        ui.checkbox(&mut self.settings.advanced_open, "Show advanced settings");
        if self.settings.advanced_open {
            ui.add_space(8.0);
            panel::inset(theme).show(ui, |ui| {
                ui.label(
                    RichText::new("Sample Rate")
                        .size(12.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                selectable_option_row(
                    ui,
                    theme,
                    &mut self.settings.manual_sample_rate_hz,
                    &[
                        (None, "Original"),
                        (Some(22_050), "22.05 kHz"),
                        (Some(32_000), "32 kHz"),
                        (Some(44_100), "44.1 kHz"),
                        (Some(48_000), "48 kHz"),
                    ],
                );

                ui.add_space(10.0);
                ui.label(
                    RichText::new("Channels")
                        .size(12.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                selectable_option_row(
                    ui,
                    theme,
                    &mut self.settings.manual_channels,
                    &[(None, "Original"), (Some(1), "Mono"), (Some(2), "Stereo")],
                );
            });
        }
    }

    fn render_extra_options(&mut self, ui: &mut Ui, theme: &AppTheme) {
        hint::title(
            ui,
            theme,
            "Extra Options",
            13.0,
            Some(
                "These stay tucked away from beginners, but they are ready when you need cleanup or format conversion workflows.",
            ),
        );
        ui.add_space(8.0);
        panel::inset(theme).show(ui, |ui| {
            ui.checkbox(&mut self.settings.normalize_volume, "Normalize volume");
            ui.checkbox(&mut self.settings.remove_metadata, "Remove metadata");
            ui.checkbox(
                &mut self.settings.convert_format_only,
                "Convert format only (no compression focus)",
            );
        });
    }

    fn render_output_preview(
        &self,
        ui: &mut Ui,
        theme: &AppTheme,
        estimate: Option<&AudioEstimate>,
        selected_metadata: Option<&crate::modules::compress_audio::models::AudioMetadata>,
    ) {
        hint::title(
            ui,
            theme,
            "Output Preview",
            13.0,
            Some(
                "Estimated size uses the current settings, so it updates before you start the batch.",
            ),
        );
        ui.add_space(8.0);

        panel::inset(theme).show(ui, |ui| match (selected_metadata, estimate) {
            (Some(metadata), Some(estimate)) => {
                ui.label(
                    RichText::new(format!(
                        "Original size: {}",
                        format_bytes(metadata.size_bytes)
                    ))
                    .size(12.0)
                    .color(theme.colors.fg),
                );
                ui.label(
                    RichText::new(format!(
                        "Estimated size: {}",
                        format_bytes(estimate.estimated_size_bytes)
                    ))
                    .size(12.0)
                    .color(theme.colors.fg),
                );

                let output_label = estimate
                    .target_bitrate_kbps
                    .map(|bitrate| format!("{} | {} kbps", estimate.output_format.label(), bitrate))
                    .unwrap_or_else(|| estimate.output_format.label().to_owned());
                ui.label(
                    RichText::new(format!("Output: {output_label}"))
                        .size(12.0)
                        .color(theme.colors.fg),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Estimated reduction: {:.0}%",
                        estimate.savings_percent.max(-100.0)
                    ))
                    .size(12.0)
                    .strong()
                    .color(theme.colors.fg),
                );
                if let Some(recommendation) = &estimate.recommendation {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(recommendation)
                            .size(11.5)
                            .color(theme.colors.fg_dim),
                    );
                }
                for warning in &estimate.warnings {
                    ui.add_space(6.0);
                    ui.label(RichText::new(warning).size(11.5).color(theme.colors.fg_dim));
                }
                if let Some(skip_reason) = &estimate.skip_reason {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(skip_reason)
                            .size(11.5)
                            .color(theme.colors.negative),
                    );
                }
            }
            _ => {
                ui.label(
                    RichText::new(
                        "Select an analyzed audio file to see size estimates and warnings.",
                    )
                    .size(12.0)
                    .color(theme.colors.fg_dim),
                );
            }
        });
    }

    fn render_actions(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        let ready_count = self
            .queue
            .iter()
            .filter(|item| item.metadata.is_some())
            .count();

        ui.label(
            RichText::new(format!("Ready in queue: {} file(s)", ready_count))
                .size(11.5)
                .color(theme.colors.fg_dim),
        );
        ui.add_space(8.0);

        if self.is_compressing() {
            ui.label(
                RichText::new(format!(
                    "Overall progress: {:.0}%",
                    overall_progress(&self.queue) * 100.0
                ))
                .size(12.0)
                .color(theme.colors.fg),
            );
            ui.add(
                ProgressBar::new(overall_progress(&self.queue))
                    .desired_width(ui.available_width())
                    .show_percentage(),
            );
            ui.add_space(10.0);
        }

        ui.horizontal(|ui| {
            let start_button = Button::new(
                RichText::new("Start Compression")
                    .size(12.0)
                    .strong()
                    .color(Color32::BLACK),
            )
            .fill(theme.colors.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::ZERO);
            if ui
                .add_enabled(!self.is_compressing() && ready_count > 0, start_button)
                .clicked()
            {
                self.start_compression(engine);
            }

            let cancel_button =
                Button::new(RichText::new("Cancel").size(12.0).color(theme.colors.fg))
                    .fill(theme.colors.bg_raised)
                    .stroke(Stroke::new(1.0, theme.colors.border))
                    .corner_radius(CornerRadius::ZERO);
            if ui
                .add_enabled(self.is_compressing(), cancel_button)
                .clicked()
            {
                self.cancel_compression();
            }
        });
    }
}
