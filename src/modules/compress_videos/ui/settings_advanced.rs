use eframe::egui::{Align, DragValue, Grid, Layout, RichText, Slider, Ui, vec2};

use crate::{
    modules::compress_videos::models::{
        CodecChoice, EncoderAvailability, ResolutionChoice, VideoMetadata, VideoSettings,
    },
    theme::AppTheme,
    ui::components::hint,
};

use super::controls::{
    advanced_bitrate_presets, advanced_codec_button, choice_button, format_kbps,
    roughly_matches_value,
};

pub(super) fn render_advanced_controls(
    ui: &mut Ui,
    theme: &AppTheme,
    selected_id: u64,
    metadata: &VideoMetadata,
    encoders: &EncoderAvailability,
    settings: &mut VideoSettings,
) {
    let source_video_kbps = metadata
        .video_bitrate_kbps
        .or(metadata.container_bitrate_kbps)
        .unwrap_or(settings.custom_bitrate_kbps)
        .clamp(350, 80_000);
    let source_fps = metadata.fps.round().clamp(12.0, 120.0) as u32;

    render_advanced_codec_controls(ui, theme, selected_id, encoders, settings);
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Video Bitrate")
                .size(12.0)
                .color(theme.colors.fg_dim),
        );
        hint::badge(
            ui,
            theme,
            "Lower bitrate reduces size faster, but detail and motion clarity can soften.",
        );
    });
    ui.label(
        RichText::new(format!("Source: {}", format_kbps(source_video_kbps)))
            .size(10.5)
            .color(theme.colors.fg_muted),
    );
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for (label, target) in advanced_bitrate_presets(metadata, settings.custom_codec) {
            if choice_button(
                ui,
                theme,
                label,
                roughly_matches_value(settings.custom_bitrate_kbps, target),
            )
            .clicked()
            {
                settings.custom_bitrate_kbps = target;
            }
        }
    });
    ui.add_space(4.0);
    ui.add(
        Slider::new(&mut settings.custom_bitrate_kbps, 350..=80_000)
            .logarithmic(true)
            .suffix(" kbps")
            .show_value(true),
    );
    ui.add(
        DragValue::new(&mut settings.custom_bitrate_kbps)
            .range(350..=80_000)
            .speed(50.0)
            .suffix(" kbps"),
    );

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Resolution")
                .size(12.0)
                .color(theme.colors.fg_dim),
        );
        hint::badge(
            ui,
            theme,
            "Lower resolution helps when you need much smaller files than the source.",
        );
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for choice in ResolutionChoice::ADVANCED {
            if choice_button(ui, theme, choice.label(), settings.resolution == choice).clicked() {
                settings.resolution = choice;
            }
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Frame Rate")
                .size(12.0)
                .color(theme.colors.fg_dim),
        );
        hint::badge(
            ui,
            theme,
            "Lower FPS usually works best for screen recordings or low-motion clips.",
        );
    });
    ui.add_space(4.0);
    let fps_choices = frame_rate_choices(source_fps);
    ui.horizontal_wrapped(|ui| {
        for fps in &fps_choices {
            let label = if *fps == source_fps {
                format!("Source ({source_fps})")
            } else {
                format!("{fps} FPS")
            };
            if choice_button(ui, theme, &label, settings.custom_fps == *fps).clicked() {
                settings.custom_fps = *fps;
            }
        }
    });
    ui.add_space(4.0);
    ui.add(
        DragValue::new(&mut settings.custom_fps)
            .range(12..=metadata.fps.round().max(12.0) as u32)
            .speed(1.0)
            .suffix(" fps"),
    );

    ui.add_space(8.0);
    render_audio_controls(ui, theme, metadata, settings);
}

fn render_advanced_codec_controls(
    ui: &mut Ui,
    theme: &AppTheme,
    selected_id: u64,
    encoders: &EncoderAvailability,
    settings: &mut VideoSettings,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Codec").size(12.0).color(theme.colors.fg_dim));
        hint::badge(
            ui,
            theme,
            "H.264 is safest, while H.265 and AV1 usually compress smaller on supported devices.",
        );
    });
    ui.add_space(4.0);
    let codec_columns: usize = if ui.available_width() >= 420.0 {
        3
    } else if ui.available_width() >= 280.0 {
        2
    } else {
        1
    };
    let codec_gap = 8.0;
    let codec_card_width = ((ui.available_width()
        - codec_gap * (codec_columns.saturating_sub(1) as f32))
        / codec_columns as f32)
        .max(0.0);

    Grid::new(ui.id().with(("advanced_codec_grid", selected_id)))
        .num_columns(codec_columns)
        .spacing(vec2(codec_gap, codec_gap))
        .min_col_width(codec_card_width)
        .show(ui, |ui| {
            for (index, codec) in CodecChoice::ALL.iter().copied().enumerate() {
                let enabled = encoders.supports(codec);
                ui.allocate_ui_with_layout(
                    vec2(codec_card_width, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        if advanced_codec_button(
                            ui,
                            theme,
                            codec,
                            settings.custom_codec == codec,
                            enabled,
                            encoders,
                            codec_card_width,
                        )
                        .clicked()
                        {
                            settings.custom_codec = codec;
                        }
                    },
                );
                if (index + 1) % codec_columns == 0 {
                    ui.end_row();
                }
            }
        });
}

fn render_audio_controls(
    ui: &mut Ui,
    theme: &AppTheme,
    metadata: &VideoMetadata,
    settings: &mut VideoSettings,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Audio").size(12.0).color(theme.colors.fg_dim));
        hint::badge(
            ui,
            theme,
            "Disable audio for silent clips or lower the bitrate to save more space.",
        );
    });
    if metadata.has_audio {
        ui.checkbox(&mut settings.custom_audio_enabled, "Keep audio track");
        if settings.custom_audio_enabled {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for kbps in [64_u32, 96, 128, 160, 192] {
                    let label = format!("{kbps} kbps");
                    if choice_button(
                        ui,
                        theme,
                        &label,
                        settings.custom_audio_bitrate_kbps == kbps,
                    )
                    .clicked()
                    {
                        settings.custom_audio_bitrate_kbps = kbps;
                    }
                }
            });
            ui.add_space(4.0);
            ui.add(
                Slider::new(&mut settings.custom_audio_bitrate_kbps, 64..=320)
                    .suffix(" kbps")
                    .show_value(true),
            );
        }
    } else {
        ui.label(
            RichText::new("This source video has no audio track.")
                .size(10.5)
                .color(theme.colors.fg_muted),
        );
    }
}

fn frame_rate_choices(source_fps: u32) -> Vec<u32> {
    let mut choices = Vec::new();
    for fps in [source_fps, 60, 30, 24] {
        let clamped = fps.min(source_fps.max(12));
        if !choices.contains(&clamped) {
            choices.push(clamped);
        }
    }
    choices
}
