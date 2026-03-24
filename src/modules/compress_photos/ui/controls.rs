use eframe::egui::{self, Button, CornerRadius, RichText, Sense, Stroke, Ui};

use crate::{theme::AppTheme, ui::components::panel};

use super::super::models::{CompressionPreset, ConvertFormat};

pub(super) fn format_selector(ui: &mut Ui, theme: &AppTheme, format_choice: &mut ConvertFormat) {
    let accent = theme.colors.accent;
    let selected_fill = theme.mix(theme.colors.surface, accent, 0.10);
    let selected_stroke = Stroke::new(1.0, theme.mix(theme.colors.border_focus, accent, 0.22));

    ui.label(
        RichText::new("Format")
            .size(12.0)
            .color(theme.colors.fg_dim),
    );
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for format in ConvertFormat::ALL {
            let is_selected = *format_choice == format;
            let fill = if is_selected {
                selected_fill
            } else {
                theme.colors.bg_raised
            };
            let stroke = if is_selected {
                selected_stroke
            } else {
                Stroke::new(1.0, theme.colors.border)
            };
            let text_color = if is_selected {
                theme.colors.accent
            } else {
                theme.colors.fg_dim
            };

            if ui
                .add(
                    Button::new(RichText::new(format.label()).size(11.0).color(text_color))
                        .fill(fill)
                        .stroke(stroke)
                        .corner_radius(CornerRadius::ZERO),
                )
                .clicked()
            {
                *format_choice = format;
            }
        }
    });
}

pub(super) fn preset_row(
    ui: &mut Ui,
    theme: &AppTheme,
    preset: CompressionPreset,
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
            ui.set_min_height(68.0);
            ui.label(
                RichText::new(preset.title())
                    .size(12.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_sized(
                [ui.available_width(), 0.0],
                egui::Label::new(
                    RichText::new(preset.description())
                        .size(11.0)
                        .color(theme.colors.fg_dim),
                )
                .wrap(),
            );
        });

    ui.interact(
        frame.response.rect,
        ui.id().with(preset.title()),
        Sense::click(),
    )
}
