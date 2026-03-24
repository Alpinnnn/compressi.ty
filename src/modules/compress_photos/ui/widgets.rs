use eframe::egui::{
    self, Color32, CornerRadius, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::{icons, theme::AppTheme};

use super::{PhotoListItem, truncate_filename};
use crate::modules::compress_photos::models::CompressionState;

pub(super) fn queue_section_header(
    ui: &mut Ui,
    theme: &AppTheme,
    title: &str,
    count: usize,
    tint: Color32,
) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{title} - {count}"))
                .size(12.0)
                .strong()
                .color(tint),
        );
    });
    let width = ui.available_width();
    let (line_rect, _) = ui.allocate_exact_size(vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(
        line_rect,
        CornerRadius::ZERO,
        theme.mix(theme.colors.border, tint, 0.30),
    );
    ui.add_space(4.0);
}

pub(super) struct QueueRowAction {
    pub(super) clicked: bool,
    pub(super) deleted: bool,
}

pub(super) fn queue_row_interactive(
    ui: &mut Ui,
    theme: &AppTheme,
    item: &PhotoListItem,
    show_delete: bool,
    can_delete: bool,
) -> QueueRowAction {
    let mut action = QueueRowAction {
        clicked: false,
        deleted: false,
    };
    let row_id = Id::new("queue_row").with(item.asset.id);
    let row_width = ui.available_width();
    let row_height = 64.0_f32;
    let (row_rect, _) = ui.allocate_exact_size(vec2(row_width, row_height), Sense::hover());

    let row_resp = ui.interact(row_rect, row_id.with("click"), Sense::click());

    let button_size = vec2(24.0, 24.0);
    let button_pos = pos2(
        row_rect.right() - button_size.x - 8.0,
        row_rect.center().y - button_size.y * 0.5,
    );
    let button_rect = Rect::from_min_size(button_pos, button_size);
    let button_resp = if show_delete && can_delete {
        Some(ui.interact(button_rect, row_id.with("trash"), Sense::click()))
    } else {
        None
    };

    let row_or_button_hovered = row_resp.hovered()
        || button_resp
            .as_ref()
            .map(|resp| resp.hovered())
            .unwrap_or(false);

    ui.painter()
        .rect_filled(row_rect, CornerRadius::ZERO, theme.colors.bg_raised);
    ui.painter().rect_stroke(
        row_rect,
        CornerRadius::ZERO,
        Stroke::new(
            1.0,
            if row_or_button_hovered {
                theme.colors.border_focus
            } else {
                theme.colors.border
            },
        ),
        StrokeKind::Middle,
    );

    let text_x = row_rect.left() + 10.0 + 42.0 + 8.0;
    let text_right = if show_delete && can_delete && row_or_button_hovered {
        button_rect.left() - 4.0
    } else {
        row_rect.right() - 10.0
    };
    let text_w = (text_right - text_x).max(0.0);
    let mut y = row_rect.top() + 10.0;

    let thumb_rect = Rect::from_min_size(
        pos2(row_rect.left() + 10.0, row_rect.top() + 10.0),
        vec2(42.0, 42.0),
    );
    ui.painter()
        .rect_filled(thumb_rect, CornerRadius::ZERO, theme.colors.bg_base);
    ui.painter().rect_stroke(
        thumb_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme.colors.border),
        StrokeKind::Middle,
    );
    if let Some(texture) = item.preview_texture.as_ref() {
        let image_rect = thumb_rect.shrink(4.0);
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            thumb_rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::IMAGE,
            icons::font_id(14.0),
            theme.colors.fg_muted,
        );
    }

    let file_name = ui.painter().layout_no_wrap(
        truncate_filename(&item.asset.file_name, 28),
        egui::FontId::proportional(12.0),
        theme.colors.fg,
    );
    ui.painter()
        .galley(pos2(text_x, y), file_name, theme.colors.fg);
    y += 16.0;

    let meta = format!(
        "{} | {} | {}x{}",
        item.asset.format.label(),
        format_bytes(item.asset.original_size),
        item.asset.dimensions.0,
        item.asset.dimensions.1,
    );
    let meta_galley = ui.painter().layout(
        meta,
        egui::FontId::proportional(10.0),
        theme.colors.fg_dim,
        text_w,
    );
    ui.painter()
        .galley(pos2(text_x, y), meta_galley, theme.colors.fg_dim);
    y += 14.0;

    match &item.state {
        CompressionState::Ready => {
            let status = ui.painter().layout_no_wrap(
                "Waiting For Compress".to_owned(),
                egui::FontId::proportional(10.0),
                theme.colors.fg_muted,
            );
            ui.painter()
                .galley(pos2(text_x, y), status, theme.colors.fg_muted);
        }
        CompressionState::Compressing(progress) => {
            let status = ui.painter().layout(
                format!(
                    "{} {:.0}%",
                    progress.stage,
                    progress.progress.clamp(0.0, 1.0) * 100.0
                ),
                egui::FontId::proportional(10.0),
                theme.colors.accent,
                text_w,
            );
            ui.painter()
                .galley(pos2(text_x, y), status, theme.colors.accent);
            y += 14.0;

            let fraction = progress.progress.clamp(0.0, 1.0);
            let bar_rect = Rect::from_min_size(pos2(text_x, y), vec2(text_w.max(20.0), 4.0));
            ui.painter()
                .rect_filled(bar_rect, CornerRadius::same(2), theme.colors.bg_base);
            ui.painter().rect_stroke(
                bar_rect,
                CornerRadius::same(2),
                Stroke::new(0.5, theme.colors.border),
                StrokeKind::Middle,
            );
            if fraction > 0.0 {
                let fill_rect = Rect::from_min_size(
                    bar_rect.min,
                    vec2(bar_rect.width() * fraction, bar_rect.height()),
                );
                ui.painter()
                    .rect_filled(fill_rect, CornerRadius::same(2), theme.colors.accent);
            }
        }
        CompressionState::Completed(result) => {
            let status = ui.painter().layout(
                format!(
                    "Done, {} output, {} -> {} ({:.1}%)",
                    result.output_format.label(),
                    format_bytes(result.original_size),
                    format_bytes(result.compressed_size),
                    result.reduction_percent.abs()
                ),
                egui::FontId::proportional(10.0),
                theme.colors.positive,
                text_w,
            );
            ui.painter()
                .galley(pos2(text_x, y), status, theme.colors.positive);
        }
        CompressionState::Failed(error) => {
            let status = ui.painter().layout(
                format!("Failed: {error}"),
                egui::FontId::proportional(10.0),
                theme.colors.negative,
                text_w,
            );
            ui.painter()
                .galley(pos2(text_x, y), status, theme.colors.negative);
        }
        CompressionState::Cancelled => {
            let status = ui.painter().layout_no_wrap(
                "Cancelled".to_owned(),
                egui::FontId::proportional(10.0),
                theme.colors.caution,
            );
            ui.painter()
                .galley(pos2(text_x, y), status, theme.colors.caution);
        }
    }

    if let Some(button) = &button_resp {
        if row_or_button_hovered {
            let t = ui.ctx().animate_bool(button.id, button.hovered());
            let fill = theme.mix(
                theme.colors.bg_raised,
                theme.colors.negative,
                0.10 + t * 0.15,
            );
            ui.painter()
                .rect_filled(button_rect, CornerRadius::ZERO, fill);
            ui.painter().rect_stroke(
                button_rect,
                CornerRadius::ZERO,
                Stroke::new(
                    1.0,
                    theme.mix(theme.colors.border, theme.colors.negative, 0.3),
                ),
                StrokeKind::Middle,
            );
            ui.painter().text(
                button_rect.center(),
                egui::Align2::CENTER_CENTER,
                icons::TRASH,
                icons::font_id(13.0),
                theme.mix(theme.colors.negative, Color32::WHITE, 0.2 + t * 0.3),
            );
            if button.clicked() {
                action.deleted = true;
            }
        }
    }

    if row_resp.clicked() && !action.deleted {
        action.clicked = true;
    }

    action
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
