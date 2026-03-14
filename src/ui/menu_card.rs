use eframe::egui::{
    self, Color32, CornerRadius, Layout, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2, pos2, vec2,
};

use crate::{modules::ModuleSpec, theme::AppTheme};

/// Render a single module card and return click response.
pub fn show(ui: &mut Ui, theme: &AppTheme, spec: ModuleSpec, size: Vec2) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool(response.id, response.hovered());

    let fill = theme.mix(
        theme.colors.surface,
        theme.mix(theme.colors.surface_hover, spec.accent, 0.08),
        t * 0.6,
    );
    let stroke = Stroke::new(
        1.0,
        theme.mix(
            theme.colors.border,
            theme.mix(theme.colors.border_focus, spec.accent, 0.15),
            0.3 + t * 0.6,
        ),
    );

    ui.painter().rect_filled(rect, theme.rounded(14), fill);
    ui.painter()
        .rect_stroke(rect, theme.rounded(14), stroke, StrokeKind::Middle);

    let bar = Rect::from_min_max(
        pos2(rect.left() + 12.0, rect.top() + 1.0),
        pos2(rect.right() - 12.0, rect.top() + 3.0),
    );
    ui.painter().rect_filled(
        bar,
        CornerRadius::ZERO,
        theme.mix(spec.accent, Color32::TRANSPARENT, 0.55 - t * 0.25),
    );

    let pad = vec2(14.0, 14.0);
    let content_rect = rect.shrink2(pad);
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(content_rect)
            .layout(Layout::top_down(egui::Align::Min)),
    );

    let icon_size = vec2(38.0, 38.0);
    let (icon_rect, _) = child.allocate_exact_size(icon_size, Sense::hover());
    paint_badge(&child, theme, spec, icon_rect, t);

    child.add_space(10.0);

    child.label(
        RichText::new(spec.title)
            .size(15.0)
            .strong()
            .color(theme.colors.fg),
    );
    child.add_space(4.0);
    child.add_sized(
        [content_rect.width(), 36.0],
        egui::Label::new(
            RichText::new(spec.description)
                .size(11.5)
                .color(theme.colors.fg_dim),
        )
        .wrap(),
    );

    response
}

fn paint_badge(ui: &Ui, theme: &AppTheme, spec: ModuleSpec, rect: Rect, t: f32) {
    let fill = theme.mix(theme.colors.bg_raised, spec.accent, 0.05 + t * 0.04);
    let stroke = Stroke::new(
        1.0,
        theme.mix(theme.colors.border, spec.accent, 0.12 + t * 0.08),
    );

    ui.painter().rect_filled(rect, theme.rounded(10), fill);
    ui.painter()
        .rect_stroke(rect, theme.rounded(10), stroke, StrokeKind::Middle);

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        spec.icon.glyph(),
        crate::icons::font_id(20.0),
        theme.mix(spec.accent, Color32::WHITE, 0.04 + t * 0.06),
    );
}
