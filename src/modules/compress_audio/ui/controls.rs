use eframe::egui::{self, Button, CornerRadius, RichText, Sense, Stroke, Ui};

use crate::{
    theme::AppTheme,
    ui::components::{hint, panel},
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

pub(super) fn selection_card(
    ui: &mut Ui,
    theme: &AppTheme,
    title: &str,
    detail: &str,
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
            ui.set_min_height(40.0);
            hint::title(ui, theme, title, 12.0, Some(detail));
        });

    ui.interact(frame.response.rect, ui.id().with(title), Sense::click())
        .on_hover_text(detail)
}
