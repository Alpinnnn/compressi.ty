use eframe::egui::{self, Align, Layout, RichText, Ui, vec2};

use crate::{
    modules::{
        compress_audio::models::AudioCompressionState,
        compress_videos::engine::VideoEngineController,
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{
    CompressAudioPage, compact,
    helpers::{format_audio_channels, format_audio_sample_rate},
    truncate_filename,
    widgets::{format_bytes, format_duration, render_panel_message},
};

impl CompressAudioPage {
    pub(super) fn render_details_panel(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        _engine: &VideoEngineController,
    ) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 24.0).max(0.0));

                let Some(selected_id) = self.selected_id else {
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Track Info",
                        "Select an audio from the queue.",
                    );
                    return;
                };

                let Some(item) = self.find_item(selected_id).cloned() else {
                    self.selected_id = None;
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Track Info",
                        "Select an audio from the queue.",
                    );
                    return;
                };

                let Some(metadata) = item.metadata.as_ref() else {
                    render_panel_message(
                        ui,
                        theme,
                        height,
                        "Track Info",
                        "Track details will be available after analysis finishes.",
                    );
                    return;
                };

                ui.horizontal(|ui| {
                    hint::title(
                        ui,
                        theme,
                        "Track Info",
                        14.0,
                        Some("Metadata for the selected audio file."),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(analysis) = item.analysis.as_ref() {
                            ui.label(
                                RichText::new(format!(
                                    "Auto Detection: {}",
                                    analysis.headline.trim_start_matches("Detected ")
                                ))
                                .size(10.5)
                                .strong()
                                .color(theme.colors.accent),
                            );
                        }
                    });
                });
                ui.add_space(10.0);

                panel::inset(theme).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);

                    ui.label(
                        RichText::new(truncate_filename(&item.file_name, 34))
                            .size(12.0)
                            .strong()
                            .color(theme.colors.fg),
                    );

                    if let Some(analysis) = item.analysis.as_ref() {
                        ui.label(
                            RichText::new(&analysis.detail)
                                .size(10.5)
                                .color(theme.colors.fg_dim),
                        );
                    }

                    ui.add_space(2.0);
                    render_info_row(ui, theme, "Status", &status_text(&item.state));
                    render_info_row(ui, theme, "Size", &format_bytes(metadata.size_bytes));
                    render_info_row(
                        ui,
                        theme,
                        "Duration",
                        &format_duration(metadata.duration_secs),
                    );
                    render_info_row(
                        ui,
                        theme,
                        "Sample Rate",
                        &format_audio_sample_rate(metadata.sample_rate_hz),
                    );
                    render_info_row(
                        ui,
                        theme,
                        "Channels",
                        format_audio_channels(metadata.channels),
                    );
                    render_info_row(
                        ui,
                        theme,
                        "Source",
                        &format!(
                            "{} | {}",
                            metadata.codec_name.to_uppercase(),
                            metadata.container_name.to_uppercase()
                        ),
                    );
                });
            });
    }
}

fn render_info_row(ui: &mut Ui, theme: &AppTheme, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(label).size(10.5).color(theme.colors.fg_muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(10.5).color(theme.colors.fg));
        });
    });
}

fn status_text(state: &AudioCompressionState) -> String {
    match state {
        AudioCompressionState::Analyzing => "Analyzing".to_owned(),
        AudioCompressionState::Ready => "Ready".to_owned(),
        AudioCompressionState::Compressing(progress) => {
            format!(
                "{} {:.0}% | {:.1}x",
                progress.stage,
                progress.progress * 100.0,
                progress.speed_x
            )
        }
        AudioCompressionState::Completed(result) => format!(
            "Done in {} | {:.0}% smaller",
            format_duration(result.elapsed_secs),
            result.reduction_percent.max(0.0)
        ),
        AudioCompressionState::Skipped(reason) => format!("Skipped | {reason}"),
        AudioCompressionState::Failed(error) => format!("Failed | {error}"),
        AudioCompressionState::Cancelled => "Cancelled".to_owned(),
    }
}
