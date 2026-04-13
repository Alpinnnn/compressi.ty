use std::time::Duration;

use eframe::egui::{self, Align, Button, Layout, RichText, Slider, Stroke, Ui, vec2};

use crate::{
    icons,
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
        self.preview_player.sync_selected_track(self.selected_id);
        if self
            .preview_scrub_position
            .as_ref()
            .is_some_and(|(track_id, _)| Some(*track_id) != self.selected_id)
        {
            self.preview_scrub_position = None;
        }

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
                    self.preview_player.stop();
                    self.preview_scrub_position = None;
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
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "Auto Detection: {}",
                                        analysis.headline.trim_start_matches("Detected ")
                                    ))
                                    .size(10.5)
                                    .strong()
                                    .color(theme.colors.accent),
                                );
                                hint::badge(ui, theme, &analysis.detail);
                            });
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
                    render_preview_player(
                        ui,
                        theme,
                        &mut self.preview_player,
                        &mut self.preview_scrub_position,
                        item.id,
                        &item.source_path,
                        Duration::from_secs_f32(metadata.duration_secs.max(0.0)),
                    );
                    ui.add_space(4.0);
                    egui::CollapsingHeader::new(
                        RichText::new("Track Info")
                            .size(11.5)
                            .strong()
                            .color(theme.colors.fg),
                    )
                    .id_salt("audio_track_info_details")
                    .default_open(true)
                    .show_unindented(ui, |ui| {
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
            });
    }
}

fn render_preview_player(
    ui: &mut Ui,
    theme: &AppTheme,
    preview_player: &mut crate::modules::compress_audio::logic::AudioPreviewPlayer,
    scrub_position: &mut Option<(u64, f32)>,
    track_id: u64,
    source_path: &std::path::Path,
    total_duration: Duration,
) {
    let max_position_secs = total_duration.as_secs_f32().max(0.01);
    let live_position_secs = preview_player
        .playback_position(total_duration)
        .as_secs_f32()
        .min(max_position_secs);
    let mut position_secs = scrub_position
        .as_ref()
        .and_then(|(scrub_track_id, scrub_secs)| {
            (*scrub_track_id == track_id).then_some(*scrub_secs)
        })
        .unwrap_or(live_position_secs)
        .min(max_position_secs);
    let is_playing = preview_player.is_playing();
    let play_button = if is_playing {
        RichText::new("||")
            .size(11.0)
            .strong()
            .color(theme.colors.fg)
    } else {
        icons::rich(icons::PLAY, 14.0, theme.colors.fg)
    };
    let timestamp = format!(
        "{} / {}",
        format_duration(position_secs),
        format_duration(total_duration.as_secs_f32())
    );
    let time_width = 100.0;

    ui.horizontal(|ui| {
        if ui
            .add(
                Button::new(play_button)
                    .fill(if is_playing {
                        theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.18)
                    } else {
                        theme.colors.bg_raised
                    })
                    .stroke(Stroke::new(1.0, theme.colors.border))
                    .corner_radius(egui::CornerRadius::ZERO)
                    .min_size(vec2(28.0, 28.0)),
            )
            .clicked()
        {
            *scrub_position = None;
            preview_player.toggle_playback(track_id, source_path);
            position_secs = preview_player
                .playback_position(total_duration)
                .as_secs_f32()
                .min(max_position_secs);
        }

        let slider_width =
            (ui.available_width() - time_width - ui.spacing().item_spacing.x).max(56.0);
        ui.spacing_mut().slider_width = slider_width;
        let slider = ui.add_sized(
            [slider_width, 18.0],
            Slider::new(&mut position_secs, 0.0..=max_position_secs).show_value(false),
        );
        if slider.drag_started() || slider.dragged() {
            *scrub_position = Some((track_id, position_secs));
        } else if slider.drag_stopped() || slider.changed() {
            preview_player.seek_to(
                track_id,
                source_path,
                Duration::from_secs_f32(position_secs.max(0.0)),
            );
            *scrub_position = None;
        }

        ui.allocate_ui_with_layout(
            vec2(time_width, 18.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.label(
                    RichText::new(timestamp)
                        .size(10.5)
                        .color(theme.colors.fg_dim),
                );
            },
        );

        if is_playing || slider.dragged() {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
    });

    if let Some(error) = preview_player.last_error() {
        ui.label(RichText::new(error).size(10.0).color(theme.colors.negative));
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
