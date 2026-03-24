use eframe::egui::{self, Button, CornerRadius, RichText, Sense, Stroke, Ui};

use crate::{
    modules::compress_videos::models::{
        CodecChoice, CompressionMode, EncoderAvailability, VideoMetadata,
    },
    theme::AppTheme,
    ui::components::panel,
};

pub(super) fn choice_button(
    ui: &mut Ui,
    theme: &AppTheme,
    label: &str,
    selected: bool,
) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).size(11.5).color(theme.colors.fg))
            .fill(if selected {
                theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.18)
            } else {
                theme.colors.bg_raised
            })
            .stroke(Stroke::new(1.0, theme.colors.border))
            .corner_radius(CornerRadius::ZERO),
    )
}

pub(super) fn advanced_codec_button(
    ui: &mut Ui,
    theme: &AppTheme,
    codec: CodecChoice,
    selected: bool,
    enabled: bool,
    encoders: &EncoderAvailability,
    card_width: f32,
) -> egui::Response {
    let (headline, detail) = match codec {
        CodecChoice::H264 => (
            "H.264",
            "Best compatibility with phones, browsers, and social apps.",
        ),
        CodecChoice::H265 => (
            "HEVC / H.265",
            "Smaller files than H.264, but some older devices may struggle.",
        ),
        CodecChoice::Av1 => (
            "AV1",
            "Best compression efficiency, with the heaviest compatibility tradeoff.",
        ),
    };
    let backend = if codec == CodecChoice::H264 && encoders.h264_nvidia
        || codec == CodecChoice::H265 && encoders.h265_nvidia
        || codec == CodecChoice::Av1 && encoders.av1_nvidia
    {
        "Auto GPU: NVIDIA"
    } else if codec == CodecChoice::H264 && encoders.h264_amd
        || codec == CodecChoice::H265 && encoders.h265_amd
        || codec == CodecChoice::Av1 && encoders.av1_amd
    {
        "Auto GPU: AMD"
    } else if enabled {
        "CPU encode"
    } else {
        "Unavailable"
    };

    let fill = if selected {
        theme.mix(theme.colors.surface, theme.colors.accent, 0.10)
    } else {
        theme.colors.bg_raised
    };
    let stroke = if selected {
        Stroke::new(
            1.0,
            theme.mix(theme.colors.border_focus, theme.colors.accent, 0.22),
        )
    } else {
        Stroke::new(1.0, theme.colors.border)
    };
    let card_min_height = if card_width >= 170.0 {
        72.0
    } else if card_width >= 136.0 {
        84.0
    } else {
        96.0
    };

    let card_margin = if card_width >= 150.0 { 10.0 } else { 8.0 };
    let frame = panel::inset(theme)
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::same(card_margin as i8))
        .show(ui, |ui| {
            ui.set_width((card_width - card_margin * 2.0 - 2.0).max(0.0));
            ui.set_min_height(card_min_height);
            ui.label(
                RichText::new(headline)
                    .size(12.0)
                    .strong()
                    .color(if enabled {
                        theme.colors.fg
                    } else {
                        theme.colors.fg_muted
                    }),
            );
            ui.add_sized(
                [ui.available_width(), 0.0],
                egui::Label::new(RichText::new(detail).size(10.5).color(if enabled {
                    theme.colors.fg_dim
                } else {
                    theme.colors.fg_muted
                }))
                .wrap(),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(backend).size(10.0).color(if enabled {
                theme.colors.accent
            } else {
                theme.colors.fg_muted
            }));
        });

    ui.interact(
        frame.response.rect,
        ui.id().with(("advanced_codec", codec.label())),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    )
}

pub(super) fn advanced_bitrate_presets(
    video: &VideoMetadata,
    codec: CodecChoice,
) -> [(&'static str, u32); 4] {
    let source = video
        .video_bitrate_kbps
        .or(video.container_bitrate_kbps)
        .unwrap_or(3_500)
        .clamp(350, 80_000) as f32;
    let codec_factor = match codec {
        CodecChoice::H264 => 1.0,
        CodecChoice::H265 => 0.82,
        CodecChoice::Av1 => 0.72,
    };
    let base = (source * codec_factor).round().clamp(350.0, 80_000.0) as u32;

    [
        ("Near Source", base),
        (
            "Balanced",
            ((base as f32) * 0.72).round().clamp(350.0, 80_000.0) as u32,
        ),
        (
            "Smaller",
            ((base as f32) * 0.50).round().clamp(350.0, 80_000.0) as u32,
        ),
        (
            "Tiny",
            ((base as f32) * 0.35).round().clamp(350.0, 80_000.0) as u32,
        ),
    ]
}

pub(super) fn roughly_matches_value(current: u32, target: u32) -> bool {
    current.abs_diff(target) <= current.max(target) / 20 + 60
}

pub(super) fn format_kbps(kbps: u32) -> String {
    if kbps >= 1000 {
        format!("{:.1} Mbps", kbps as f32 / 1000.0)
    } else {
        format!("{kbps} kbps")
    }
}

pub(super) fn secondary_button(ui: &mut Ui, theme: &AppTheme, label: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).size(11.5).color(theme.colors.fg))
            .fill(theme.colors.bg_raised)
            .stroke(Stroke::new(1.0, theme.colors.border))
            .corner_radius(CornerRadius::ZERO),
    )
}

pub(super) fn render_simple_bar(ui: &mut Ui, theme: &AppTheme, progress: f32, label: &str) {
    if !label.is_empty() {
        ui.label(RichText::new(label).size(11.5).color(theme.colors.fg_dim));
        ui.add_space(4.0);
    }
    let width = ui.available_width().max(180.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 10.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), theme.colors.bg_base);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, theme.colors.border),
        egui::StrokeKind::Middle,
    );
    let fill_rect = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width() * progress.clamp(0.0, 1.0), rect.height()),
    );
    ui.painter()
        .rect_filled(fill_rect, CornerRadius::same(2), theme.colors.accent);
}

/// Card-style mode selector matching the compress_photos `preset_row` design.
pub(super) fn mode_card(
    ui: &mut Ui,
    theme: &AppTheme,
    mode: CompressionMode,
    selected: bool,
) -> egui::Response {
    let accent = theme.colors.accent;
    let fill = if selected {
        theme.mix(theme.colors.surface, accent, 0.10)
    } else {
        theme.colors.bg_raised
    };
    let stroke = if selected {
        Stroke::new(1.0, theme.mix(theme.colors.border_focus, accent, 0.22))
    } else {
        Stroke::new(1.0, theme.colors.border)
    };

    let frame = panel::inset(theme)
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(52.0);
            ui.label(
                RichText::new(mode.title())
                    .size(12.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_sized(
                [ui.available_width(), 0.0],
                egui::Label::new(
                    RichText::new(mode.description())
                        .size(11.0)
                        .color(theme.colors.fg_dim),
                )
                .wrap(),
            );
        });

    ui.interact(
        frame.response.rect,
        ui.id().with(mode.title()),
        Sense::click(),
    )
}
