use eframe::egui::{
    self, Align, Button, CornerRadius, DragValue, Layout, RichText, ScrollArea, Slider, Stroke, Ui,
    vec2,
};

use crate::{
    modules::compress_videos::{
        engine::VideoEngineController,
        models::{
            CompressionMode, EncoderAvailability, EncoderBackend, ResolutionChoice,
            VideoCompressionState, VideoMetadata, VideoSettings,
        },
        processor,
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{
    BannerMessage, BannerTone, CompressVideosPage, compact,
    controls::{choice_button, mode_card},
    is_video_settings_editable,
    settings_advanced::render_advanced_controls,
    truncate_filename,
    widgets::{format_bytes, format_duration},
};

impl CompressVideosPage {
    pub(super) fn render_settings_panel(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
        use_hardware_acceleration: bool,
    ) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                let Some(selected_id) = self.selected_id else {
                    render_settings_message(ui, theme, height, "Select a video from the queue.");
                    return;
                };

                let Some(item) = self
                    .queue
                    .iter()
                    .find(|item| item.id == selected_id)
                    .cloned()
                else {
                    render_settings_message(ui, theme, height, "Select a video from the queue.");
                    return;
                };

                if !is_video_settings_editable(&item.state) {
                    self.selected_id = None;
                    render_settings_message(
                        ui,
                        theme,
                        height,
                        "Settings are only available while the video is still in the queue.",
                    );
                    return;
                }

                let Some(metadata) = item.metadata.clone() else {
                    render_settings_message(
                        ui,
                        theme,
                        height,
                        "Settings will be available after the video finishes probing.",
                    );
                    return;
                };
                let Some(mut settings) = item.settings.clone() else {
                    render_settings_message(
                        ui,
                        theme,
                        height,
                        "Settings will be available after the video finishes probing.",
                    );
                    return;
                };
                let encoders = engine
                    .active_info()
                    .map(|info| {
                        info.encoders
                            .with_hardware_acceleration(use_hardware_acceleration)
                    })
                    .unwrap_or_default();

                render_settings_header(ui, theme, &item.file_name, &metadata);

                ScrollArea::vertical()
                    .id_salt("video_settings_scroll")
                    .auto_shrink([false, false])
                    .max_height((height - 100.0).max(0.0))
                    .show(ui, |ui| {
                        compact(ui);
                        render_mode_selector(ui, theme, &mut settings);
                        ui.add_space(6.0);
                        render_encoder_badge(ui, theme, &settings, &encoders);
                        ui.add_space(8.0);

                        match settings.mode {
                            CompressionMode::ReduceSize => {
                                render_reduce_size_controls(ui, theme, &metadata, &mut settings);
                            }
                            CompressionMode::GoodQuality => {
                                render_good_quality_controls(ui, theme, &mut settings);
                            }
                            CompressionMode::CustomAdvanced => {
                                render_advanced_controls(
                                    ui,
                                    theme,
                                    selected_id,
                                    &metadata,
                                    &encoders,
                                    &mut settings,
                                );
                            }
                        }

                        render_estimate(ui, theme, &metadata, &encoders, &settings);
                        ui.add_space(8.0);
                        if render_apply_to_all_button(ui, theme) {
                            self.apply_settings_to_ready_videos(&settings);
                        }
                    });

                if let Some(queue_item) = self
                    .queue
                    .iter_mut()
                    .find(|queue_item| queue_item.id == selected_id)
                {
                    queue_item.settings = Some(settings);
                }
            });
    }

    fn apply_settings_to_ready_videos(&mut self, settings: &VideoSettings) {
        for queue_item in &mut self.queue {
            if matches!(queue_item.state, VideoCompressionState::Ready) {
                if let Some(metadata) = &queue_item.metadata {
                    let mut applied = settings.clone();
                    let range = processor::size_slider_range(metadata);
                    applied.target_size_mb =
                        applied.target_size_mb.clamp(range.min_mb, range.max_mb);
                    applied.custom_fps = applied
                        .custom_fps
                        .min(metadata.fps.round().max(12.0) as u32);
                    queue_item.settings = Some(applied);
                }
            }
        }

        self.banner = Some(BannerMessage {
            tone: BannerTone::Info,
            text: "Settings applied to all ready videos.".into(),
        });
    }
}

fn render_settings_message(ui: &mut Ui, theme: &AppTheme, height: f32, message: &str) {
    let body_height = (height - 32.0).max(140.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), body_height),
        Layout::top_down(Align::Min),
        |ui| {
            ui.add_space((body_height - 54.0).max(0.0) * 0.5);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Settings")
                        .size(14.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                ui.add_space(8.0);
                ui.label(RichText::new(message).size(12.0).color(theme.colors.fg_dim));
            });
        },
    );
}

fn render_settings_header(
    ui: &mut Ui,
    theme: &AppTheme,
    file_name: &str,
    metadata: &VideoMetadata,
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
            "{} | {} | {}x{}",
            format_bytes(metadata.size_bytes),
            format_duration(metadata.duration_secs),
            metadata.width,
            metadata.height
        ))
        .size(11.0)
        .color(theme.colors.fg_dim),
    );
    ui.add_space(8.0);
}

fn render_mode_selector(ui: &mut Ui, theme: &AppTheme, settings: &mut VideoSettings) {
    for mode in CompressionMode::ALL {
        if mode_card(ui, theme, mode, settings.mode == mode).clicked() {
            settings.mode = mode;
        }
    }
}

fn render_encoder_badge(
    ui: &mut Ui,
    theme: &AppTheme,
    settings: &VideoSettings,
    encoders: &EncoderAvailability,
) {
    let active_codec = match settings.mode {
        CompressionMode::ReduceSize => encoders.reduce_size_codec(),
        CompressionMode::GoodQuality => encoders.quality_codec(),
        CompressionMode::CustomAdvanced => settings.custom_codec,
    };
    let resolved = encoders.resolved_encoder(active_codec);
    let (badge_text, badge_color) = match resolved.backend {
        EncoderBackend::Nvidia => (
            format!("\u{26A1} GPU: NVIDIA ({})", resolved.ffmpeg_name()),
            theme.colors.accent,
        ),
        EncoderBackend::Amd => (
            format!("\u{26A1} GPU: AMD ({})", resolved.ffmpeg_name()),
            theme.colors.accent,
        ),
        EncoderBackend::IntelQuickSync => (
            format!(
                "\u{26A1} GPU: Intel Quick Sync ({})",
                resolved.ffmpeg_name()
            ),
            theme.colors.accent,
        ),
        EncoderBackend::Software => (
            format!("\u{1F5A5} CPU Encoding ({})", resolved.ffmpeg_name()),
            theme.colors.fg_muted,
        ),
    };

    ui.label(
        RichText::new(badge_text)
            .size(11.0)
            .strong()
            .color(badge_color),
    );
}

fn render_reduce_size_controls(
    ui: &mut Ui,
    theme: &AppTheme,
    metadata: &VideoMetadata,
    settings: &mut VideoSettings,
) {
    let slider_range = processor::size_slider_range(metadata);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Target size (MB)")
                .size(12.0)
                .color(theme.colors.fg_dim),
        );
        hint::badge(
            ui,
            theme,
            "Smaller targets reduce size faster, but quality drops sooner.",
        );
    });
    ui.add(
        Slider::new(
            &mut settings.target_size_mb,
            slider_range.min_mb..=slider_range.max_mb,
        )
        .suffix(" MB")
        .show_value(true),
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Direct input:")
                .size(11.0)
                .color(theme.colors.fg_dim),
        );
        ui.add(
            DragValue::new(&mut settings.target_size_mb)
                .range(slider_range.min_mb..=slider_range.max_mb)
                .speed(1.0)
                .suffix(" MB"),
        );
    });
}

fn render_good_quality_controls(ui: &mut Ui, theme: &AppTheme, settings: &mut VideoSettings) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Quality")
                .size(12.0)
                .color(theme.colors.fg_dim),
        );
        hint::badge(
            ui,
            theme,
            "Higher values preserve more detail and usually save less space.",
        );
    });
    ui.add(Slider::new(&mut settings.quality, 20..=95).show_value(true));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Resolution")
                .size(12.0)
                .color(theme.colors.fg_dim),
        );
        hint::badge(
            ui,
            theme,
            "Lower resolution can cut file size when bitrate alone is not enough.",
        );
    });
    ui.horizontal_wrapped(|ui| {
        for choice in ResolutionChoice::QUICK {
            if choice_button(ui, theme, choice.label(), settings.resolution == choice).clicked() {
                settings.resolution = choice;
            }
        }
    });
}

fn render_estimate(
    ui: &mut Ui,
    theme: &AppTheme,
    metadata: &VideoMetadata,
    encoders: &EncoderAvailability,
    settings: &VideoSettings,
) {
    let estimate = processor::estimate_output(metadata, settings, encoders);
    ui.add_space(8.0);
    let (estimate_text, estimate_color) = if estimate.savings_percent >= 0.0 {
        (
            format!(
                "Est. {} ({:.0}% smaller)",
                format_bytes(estimate.estimated_size_bytes),
                estimate.savings_percent
            ),
            theme.colors.positive,
        )
    } else {
        (
            format!(
                "Est. {} ({:.0}% larger)",
                format_bytes(estimate.estimated_size_bytes),
                estimate.savings_percent.abs()
            ),
            theme.colors.caution,
        )
    };
    ui.label(
        RichText::new(estimate_text)
            .size(11.5)
            .color(estimate_color),
    );
}

fn render_apply_to_all_button(ui: &mut Ui, theme: &AppTheme) -> bool {
    ui.add(
        Button::new(
            RichText::new("Apply Settings to All Ready Videos")
                .size(11.5)
                .color(theme.colors.fg),
        )
        .fill(theme.colors.bg_raised)
        .stroke(Stroke::new(1.0, theme.colors.border))
        .corner_radius(CornerRadius::ZERO),
    )
    .clicked()
}
