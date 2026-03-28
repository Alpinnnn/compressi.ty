use eframe::egui::{self, Align, Layout, RichText, ScrollArea, Slider, Stroke, Ui, vec2};

use crate::{
    modules::{
        compress_audio::{
            logic::estimate_output,
            models::{
                AudioAutoPreset, AudioCompressionSettings, AudioFormat, AudioMetadata,
                AudioWorkflowMode,
            },
        },
        compress_videos::{engine::VideoEngineController, models::EncoderAvailability},
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{
    BannerMessage, BannerTone, CompressAudioPage, compact,
    controls::selection_card,
    helpers::{
        channel_choices, format_audio_channels, format_audio_sample_rate, format_available,
        output_summary, sample_rate_choices,
    },
    is_audio_settings_editable, truncate_filename,
    widgets::{format_bytes, format_duration, render_panel_message},
};

impl CompressAudioPage {
    pub(super) fn render_settings_panel(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
    ) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                let Some(selected_id) = self.selected_id else {
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Settings",
                        "Select an audio from the queue.",
                    );
                    return;
                };

                let Some(item) = self.find_item(selected_id).cloned() else {
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Settings",
                        "Select an audio from the queue.",
                    );
                    return;
                };

                if !is_audio_settings_editable(&item.state) {
                    self.selected_id = None;
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Settings",
                        "Settings are only available while the audio is still in the queue.",
                    );
                    return;
                }

                let Some(metadata) = item.metadata.clone() else {
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Settings",
                        "Settings will be available after the audio finishes analysis.",
                    );
                    return;
                };

                let Some(mut settings) = item.settings.clone() else {
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Settings",
                        "Settings will be available after the audio finishes analysis.",
                    );
                    return;
                };

                let encoders = engine
                    .active_info()
                    .map(|info| info.encoders.clone())
                    .unwrap_or_default();
                let estimate = estimate_output(&metadata, &settings, &encoders);

                render_settings_header(ui, theme, &item.file_name, &metadata);

                ScrollArea::vertical()
                    .id_salt("audio_settings_scroll")
                    .auto_shrink([false, false])
                    .max_height((height - 100.0).max(0.0))
                    .show(ui, |ui| {
                        compact(ui);
                        render_mode_selector(ui, theme, &mut settings);
                        ui.add_space(6.0);
                        render_output_badge(ui, theme, &estimate, settings.mode);
                        ui.add_space(8.0);

                        match settings.mode {
                            AudioWorkflowMode::Auto => {
                                render_auto_controls(ui, theme, &mut settings)
                            }
                            AudioWorkflowMode::Manual => {
                                render_manual_controls(ui, theme, &encoders, &mut settings)
                            }
                        }

                        ui.add_space(8.0);
                        render_bottom_panels(ui, theme, &mut settings, &metadata, &estimate);
                        ui.add_space(8.0);
                        if render_apply_to_all_button(ui, theme) {
                            self.apply_settings_to_ready_audios(&settings);
                        }
                    });

                if let Some(queue_item) = self.find_item_mut(selected_id) {
                    queue_item.settings = Some(settings);
                }
            });
    }

    fn apply_settings_to_ready_audios(&mut self, settings: &AudioCompressionSettings) {
        for queue_item in &mut self.queue {
            if matches!(
                &queue_item.state,
                crate::modules::compress_audio::models::AudioCompressionState::Ready
            ) {
                queue_item.settings = Some(settings.clone());
            }
        }

        self.banner = Some(BannerMessage {
            tone: BannerTone::Info,
            text: "Settings applied to all ready audio files.".to_owned(),
        });
    }
}

fn render_settings_header(
    ui: &mut Ui,
    theme: &AppTheme,
    file_name: &str,
    metadata: &AudioMetadata,
) {
    ui.label(
        RichText::new(format!("Settings - {}", truncate_filename(file_name, 24)))
            .size(14.0)
            .strong()
            .color(theme.colors.fg),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "{} | {} | {} | {}",
            format_bytes(metadata.size_bytes),
            format_duration(metadata.duration_secs),
            format_audio_sample_rate(metadata.sample_rate_hz),
            format_audio_channels(metadata.channels),
        ))
        .size(11.0)
        .color(theme.colors.fg_dim),
    );
    ui.add_space(8.0);
}

fn render_mode_selector(ui: &mut Ui, theme: &AppTheme, settings: &mut AudioCompressionSettings) {
    for mode in AudioWorkflowMode::ALL {
        if selection_card(
            ui,
            theme,
            mode.title(),
            mode.description(),
            settings.mode == mode,
        )
        .clicked()
        {
            settings.mode = mode;
        }
    }
}

fn render_output_badge(
    ui: &mut Ui,
    theme: &AppTheme,
    estimate: &crate::modules::compress_audio::models::AudioEstimate,
    mode: AudioWorkflowMode,
) {
    let label = format!(
        "{} output: {}",
        match mode {
            AudioWorkflowMode::Auto => "Smart",
            AudioWorkflowMode::Manual => "Current",
        },
        output_summary(estimate.output_format, estimate.target_bitrate_kbps)
    );

    ui.label(
        RichText::new(label)
            .size(11.0)
            .strong()
            .color(theme.colors.accent),
    );
}

fn render_auto_controls(ui: &mut Ui, theme: &AppTheme, settings: &mut AudioCompressionSettings) {
    for preset in AudioAutoPreset::ALL {
        if selection_card(
            ui,
            theme,
            preset.label(),
            preset.detail(),
            settings.auto_preset == preset,
        )
        .clicked()
        {
            settings.auto_preset = preset;
        }
    }
}

fn render_manual_controls(
    ui: &mut Ui,
    theme: &AppTheme,
    encoders: &EncoderAvailability,
    settings: &mut AudioCompressionSettings,
) {
    hint::title(
        ui,
        theme,
        "Format",
        12.0,
        Some("Pick the output codec/container used in Manual mode."),
    );
    ui.horizontal_wrapped(|ui| {
        for format in AudioFormat::ALL {
            let available = format_available(format, encoders);
            ui.add_enabled_ui(available, |ui| {
                if super::controls::choice_button(
                    ui,
                    theme,
                    format.label(),
                    settings.manual_format == format,
                )
                .clicked()
                {
                    settings.manual_format = format;
                }
            });
        }
    });

    ui.add_space(4.0);
    let bitrate_enabled = !settings.manual_format.is_lossless();
    ui.add_enabled_ui(bitrate_enabled, |ui| {
        hint::title(
            ui,
            theme,
            &format!("Bitrate: {} kbps", settings.manual_bitrate_kbps),
            12.0,
            Some("Lower targets shrink the file more aggressively."),
        );
        ui.add(Slider::new(&mut settings.manual_bitrate_kbps, 24..=320).suffix(" kbps"));
    });

    ui.add_space(4.0);
    ui.checkbox(&mut settings.advanced_open, "Advanced");
    if settings.advanced_open {
        panel::inset(theme).show(ui, |ui| {
            hint::title(
                ui,
                theme,
                "Sample Rate",
                12.0,
                Some("Resample the output only when you need smaller files or compatibility."),
            );
            option_row(
                ui,
                theme,
                &mut settings.manual_sample_rate_hz,
                &sample_rate_choices(),
            );

            ui.add_space(10.0);
            hint::title(
                ui,
                theme,
                "Channels",
                12.0,
                Some("Downmix to mono for voice or keep stereo when spatial detail matters."),
            );
            option_row(ui, theme, &mut settings.manual_channels, &channel_choices());
        });
    }
}

fn render_extra_options(ui: &mut Ui, theme: &AppTheme, settings: &mut AudioCompressionSettings) {
    hint::title(
        ui,
        theme,
        "Extra",
        12.0,
        Some("Optional cleanup and conversion behaviors."),
    );
    panel::inset(theme).show(ui, |ui| {
        ui.checkbox(&mut settings.normalize_volume, "Normalize Volume");
        ui.checkbox(&mut settings.remove_metadata, "Remove Metadata");
        ui.checkbox(&mut settings.convert_format_only, "Convert Format Only");
    });
}

fn render_bottom_panels(
    ui: &mut Ui,
    theme: &AppTheme,
    settings: &mut AudioCompressionSettings,
    metadata: &AudioMetadata,
    estimate: &crate::modules::compress_audio::models::AudioEstimate,
) {
    let available_width = ui.available_width();
    if available_width >= 360.0 {
        let gutter = 12.0;
        let panel_width = ((available_width - gutter) * 0.5).max(0.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(gutter, 0.0);
            ui.allocate_ui_with_layout(
                vec2(panel_width, 0.0),
                Layout::top_down(Align::Min),
                |ui| render_extra_options(ui, theme, settings),
            );
            ui.allocate_ui_with_layout(
                vec2(panel_width, 0.0),
                Layout::top_down(Align::Min),
                |ui| render_estimate(ui, theme, metadata, estimate),
            );
        });
    } else {
        render_extra_options(ui, theme, settings);
        ui.add_space(8.0);
        render_estimate(ui, theme, metadata, estimate);
    }
}

fn render_estimate(
    ui: &mut Ui,
    theme: &AppTheme,
    metadata: &AudioMetadata,
    estimate: &crate::modules::compress_audio::models::AudioEstimate,
) {
    hint::title(
        ui,
        theme,
        "Estimate",
        12.0,
        Some("Preview the current output size before compression starts."),
    );
    panel::inset(theme).show(ui, |ui| {
        ui.label(
            RichText::new(format!("Original: {}", format_bytes(metadata.size_bytes)))
                .size(11.5)
                .color(theme.colors.fg),
        );
        ui.label(
            RichText::new(format!(
                "Estimated: {}",
                format_bytes(estimate.estimated_size_bytes)
            ))
            .size(11.5)
            .color(theme.colors.fg),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "Estimated reduction: {:.0}%",
                estimate.savings_percent.max(-100.0)
            ))
            .size(11.5)
            .strong()
            .color(if estimate.savings_percent >= 0.0 {
                theme.colors.positive
            } else {
                theme.colors.caution
            }),
        );

        if let Some(sample_rate_hz) = estimate.effective_sample_rate_hz {
            ui.label(
                RichText::new(format!(
                    "Sample rate: {}",
                    format_audio_sample_rate(sample_rate_hz)
                ))
                .size(10.5)
                .color(theme.colors.fg_dim),
            );
        }

        if let Some(channels) = estimate.effective_channels {
            ui.label(
                RichText::new(format!("Channels: {}", format_audio_channels(channels)))
                    .size(10.5)
                    .color(theme.colors.fg_dim),
            );
        }

        if let Some(recommendation) = &estimate.recommendation {
            ui.add_space(6.0);
            ui.label(
                RichText::new(recommendation)
                    .size(10.5)
                    .color(theme.colors.fg_dim),
            );
        }

        for warning in &estimate.warnings {
            ui.add_space(4.0);
            ui.label(
                RichText::new(warning)
                    .size(10.5)
                    .color(theme.colors.fg_muted),
            );
        }

        if let Some(skip_reason) = &estimate.skip_reason {
            ui.add_space(6.0);
            ui.label(
                RichText::new(skip_reason)
                    .size(10.5)
                    .color(theme.colors.negative),
            );
        }
    });
}

fn render_apply_to_all_button(ui: &mut Ui, theme: &AppTheme) -> bool {
    ui.add(
        egui::Button::new(
            RichText::new("Apply Settings to All Ready Audio")
                .size(11.5)
                .color(theme.colors.fg),
        )
        .fill(theme.colors.bg_raised)
        .stroke(Stroke::new(1.0, theme.colors.border))
        .corner_radius(egui::CornerRadius::ZERO),
    )
    .clicked()
}

fn option_row<T: Copy + PartialEq>(
    ui: &mut Ui,
    theme: &AppTheme,
    value: &mut Option<T>,
    options: &[(Option<T>, &'static str)],
) {
    ui.horizontal_wrapped(|ui| {
        for (candidate, label) in options {
            if super::controls::choice_button(ui, theme, label, *value == *candidate).clicked() {
                *value = *candidate;
            }
        }
    });
}
