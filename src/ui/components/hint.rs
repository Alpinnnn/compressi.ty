use eframe::egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, vec2,
};

use crate::theme::AppTheme;

/// Renders a strong title with an optional hover hint badge beside it.
pub fn title(ui: &mut Ui, theme: &AppTheme, label: &str, size: f32, hint: Option<&str>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(label)
                .size(size)
                .strong()
                .color(theme.colors.fg),
        );

        if let Some(hint) = hint.filter(|hint| !hint.trim().is_empty()) {
            ui.add_space(4.0);
            badge(ui, theme, hint);
        }
    });
}

/// Renders the small `?` badge used to tuck supporting copy behind hover.
pub fn badge(ui: &mut Ui, theme: &AppTheme, hint: &str) -> Response {
    let size = vec2(14.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let t = ui.ctx().animate_bool(response.id, response.hovered());
    let fill = theme.mix(
        theme.colors.bg_raised,
        theme.mix(theme.colors.surface_hover, theme.colors.accent, 0.08),
        0.38 + t * 0.28,
    );
    let stroke = Stroke::new(
        1.0,
        theme.mix(theme.colors.border, theme.colors.accent, 0.08 + t * 0.18),
    );
    let text_color = theme.mix(theme.colors.fg_muted, Color32::WHITE, 0.12 + t * 0.10);

    ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(7), stroke, StrokeKind::Middle);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "?",
        FontId::new(9.0, FontFamily::Proportional),
        text_color,
    );

    response.on_hover_text(hint)
}
