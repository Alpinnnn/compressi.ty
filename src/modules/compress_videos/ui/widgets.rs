use eframe::egui::{
    self, Color32, CornerRadius, Id, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Ui,
    pos2, vec2,
};

use crate::{
    icons,
    modules::compress_videos::models::{VideoCompressionState, VideoQueueItem},
    theme::AppTheme,
    ui::components::panel,
};

use super::{BannerMessage, BannerTone, truncate_filename};

pub(super) fn render_banner(ui: &mut Ui, theme: &AppTheme, banner: &BannerMessage) {
    let tint = match banner.tone {
        BannerTone::Info => theme.colors.accent,
        BannerTone::Success => theme.colors.positive,
        BannerTone::Error => theme.colors.negative,
    };

    panel::tinted(theme, tint)
        .inner_margin(egui::Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.label(
                RichText::new(&banner.text)
                    .size(12.5)
                    .color(theme.colors.fg),
            );
        });
}

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

pub(super) fn video_queue_row(
    ui: &mut Ui,
    theme: &AppTheme,
    item: &VideoQueueItem,
    selected: bool,
    can_delete: bool,
    thumbnail: Option<&TextureHandle>,
) -> QueueRowAction {
    let mut action = QueueRowAction {
        clicked: false,
        deleted: false,
    };
    let row_id = Id::new("vq_row").with(item.id);
    let row_width = ui.available_width();
    let row_height = 64.0_f32;
    let (row_rect, _) = ui.allocate_exact_size(vec2(row_width, row_height), Sense::hover());
    let row_response = ui.interact(row_rect, row_id.with("click"), Sense::click());

    let button_size = vec2(24.0, 24.0);
    let button_position = pos2(
        row_rect.right() - button_size.x - 8.0,
        row_rect.center().y - button_size.y * 0.5,
    );
    let button_rect = Rect::from_min_size(button_position, button_size);
    let button_response = if can_delete {
        Some(ui.interact(button_rect, row_id.with("trash"), Sense::click()))
    } else {
        None
    };
    let row_hovered = row_response.hovered()
        || button_response
            .as_ref()
            .map(|response| response.hovered())
            .unwrap_or(false);

    let fill = if selected {
        theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.10)
    } else {
        theme.colors.bg_raised
    };
    ui.painter().rect_filled(row_rect, CornerRadius::ZERO, fill);
    ui.painter().rect_stroke(
        row_rect,
        CornerRadius::ZERO,
        Stroke::new(
            1.0,
            if row_hovered || selected {
                theme.colors.border_focus
            } else {
                theme.colors.border
            },
        ),
        StrokeKind::Middle,
    );

    let thumbnail_size = 42.0;
    let thumbnail_rect = Rect::from_min_size(
        pos2(row_rect.left() + 10.0, row_rect.top() + 10.0),
        vec2(thumbnail_size, thumbnail_size),
    );
    ui.painter()
        .rect_filled(thumbnail_rect, CornerRadius::ZERO, theme.colors.bg_base);
    ui.painter().rect_stroke(
        thumbnail_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, theme.colors.border),
        StrokeKind::Middle,
    );
    if let Some(texture) = thumbnail {
        let image_rect = thumbnail_rect.shrink(4.0);
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            thumbnail_rect.center(),
            egui::Align2::CENTER_CENTER,
            icons::VIDEO,
            icons::font_id(14.0),
            theme.colors.fg_muted,
        );
    }

    let text_left = row_rect.left() + 10.0 + thumbnail_size + 8.0;
    let text_right = if can_delete && row_hovered {
        button_rect.left() - 4.0
    } else {
        row_rect.right() - 10.0
    };
    let text_width = (text_right - text_left).max(0.0);
    let mut text_top = row_rect.top() + 10.0;

    let name_galley = ui.painter().layout_no_wrap(
        truncate_filename(&item.file_name, 28),
        egui::FontId::proportional(12.0),
        theme.colors.fg,
    );
    ui.painter()
        .galley(pos2(text_left, text_top), name_galley, theme.colors.fg);
    text_top += 16.0;

    match &item.state {
        VideoCompressionState::Probing => {
            let galley = ui.painter().layout_no_wrap(
                "Probing...".to_owned(),
                egui::FontId::proportional(10.0),
                theme.colors.fg_muted,
            );
            ui.painter()
                .galley(pos2(text_left, text_top), galley, theme.colors.fg_muted);
        }
        VideoCompressionState::Ready => {
            if let Some(metadata) = &item.metadata {
                let info = format!(
                    "{} | {}",
                    format_bytes(metadata.size_bytes),
                    format_duration(metadata.duration_secs)
                );
                let galley = ui.painter().layout(
                    info,
                    egui::FontId::proportional(10.0),
                    theme.colors.fg_dim,
                    text_width,
                );
                ui.painter()
                    .galley(pos2(text_left, text_top), galley, theme.colors.fg_dim);
            } else {
                let galley = ui.painter().layout_no_wrap(
                    "Ready".to_owned(),
                    egui::FontId::proportional(10.0),
                    theme.colors.fg_muted,
                );
                ui.painter()
                    .galley(pos2(text_left, text_top), galley, theme.colors.fg_muted);
            }
        }
        VideoCompressionState::Compressing(progress) => {
            let galley = ui.painter().layout_no_wrap(
                format!("Compressing {:.0}%", progress.progress * 100.0),
                egui::FontId::proportional(10.0),
                theme.colors.accent,
            );
            ui.painter()
                .galley(pos2(text_left, text_top), galley, theme.colors.accent);
            text_top += 12.0;
            let bar_width = text_width.max(20.0);
            let bar_rect = Rect::from_min_size(pos2(text_left, text_top), vec2(bar_width, 4.0));
            ui.painter()
                .rect_filled(bar_rect, CornerRadius::same(2), theme.colors.bg_base);
            if progress.progress > 0.0 {
                let fill_rect = Rect::from_min_size(
                    bar_rect.min,
                    vec2(bar_rect.width() * progress.progress.clamp(0.0, 1.0), 4.0),
                );
                ui.painter()
                    .rect_filled(fill_rect, CornerRadius::same(2), theme.colors.accent);
            }
        }
        VideoCompressionState::Completed(result) => {
            let text = format!(
                "Done, {} -> {} ({:.1}%)",
                format_bytes(result.original_size_bytes),
                format_bytes(result.output_size_bytes),
                result.reduction_percent.abs()
            );
            let galley = ui.painter().layout(
                text,
                egui::FontId::proportional(10.0),
                theme.colors.positive,
                text_width,
            );
            ui.painter()
                .galley(pos2(text_left, text_top), galley, theme.colors.positive);
        }
        VideoCompressionState::Failed(error) => {
            let galley = ui.painter().layout(
                format!("Failed: {error}"),
                egui::FontId::proportional(10.0),
                theme.colors.negative,
                text_width,
            );
            ui.painter()
                .galley(pos2(text_left, text_top), galley, theme.colors.negative);
        }
        VideoCompressionState::Cancelled => {
            let galley = ui.painter().layout_no_wrap(
                "Cancelled".to_owned(),
                egui::FontId::proportional(10.0),
                theme.colors.caution,
            );
            ui.painter()
                .galley(pos2(text_left, text_top), galley, theme.colors.caution);
        }
    }

    if let Some(button_response) = &button_response {
        if row_hovered {
            let animation = ui
                .ctx()
                .animate_bool(button_response.id, button_response.hovered());
            let button_fill = theme.mix(
                theme.colors.bg_raised,
                theme.colors.negative,
                0.10 + animation * 0.15,
            );
            ui.painter()
                .rect_filled(button_rect, CornerRadius::ZERO, button_fill);
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
                theme.mix(theme.colors.negative, Color32::WHITE, 0.2 + animation * 0.3),
            );
            if button_response.clicked() {
                action.deleted = true;
            }
        }
    }

    if row_response.clicked() && !action.deleted {
        action.clicked = true;
    }

    action
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_index = 0_usize;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

pub(super) fn format_duration(seconds: f32) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}
