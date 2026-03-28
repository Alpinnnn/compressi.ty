use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Stroke, TextureHandle, Ui,
    pos2, vec2,
};

use crate::{modules::compress_videos::models::VideoCompressionState, theme::AppTheme};

use super::truncate_filename;

pub(super) fn render_preview_message(ui: &mut Ui, theme: &AppTheme, height: f32, message: &str) {
    let body_height = (height - 32.0).max(140.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), body_height),
        Layout::top_down(Align::Min),
        |ui| {
            ui.add_space((body_height - 54.0).max(0.0) * 0.5);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Preview")
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

pub(super) fn paint_centered_texture(ui: &mut Ui, rect: Rect, texture: &TextureHandle) {
    let texture_size = texture.size_vec2();
    let scale = (rect.width() / texture_size.x).min(rect.height() / texture_size.y);
    let draw_rect = Rect::from_center_size(rect.center(), texture_size * scale);
    ui.painter().with_clip_rect(rect).image(
        texture.id(),
        draw_rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

pub(super) fn paint_play_feedback_icon(ui: &mut Ui, center: Pos2, color: Color32) {
    let points = vec![
        pos2(center.x - 7.0, center.y - 10.0),
        pos2(center.x - 7.0, center.y + 10.0),
        pos2(center.x + 10.5, center.y),
    ];
    ui.painter()
        .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

pub(super) fn paint_pause_feedback_icon(ui: &mut Ui, center: Pos2, color: Color32) {
    let bar_size = vec2(6.0, 20.0);
    let gap = 4.0;
    let left_bar = Rect::from_center_size(pos2(center.x - gap - 3.0, center.y), bar_size);
    let right_bar = Rect::from_center_size(pos2(center.x + gap + 3.0, center.y), bar_size);
    ui.painter()
        .rect_filled(left_bar, CornerRadius::same(2), color);
    ui.painter()
        .rect_filled(right_bar, CornerRadius::same(2), color);
}

pub(super) fn paint_preview_status_overlay(
    ui: &mut Ui,
    theme: &AppTheme,
    rect: Rect,
    state: &VideoCompressionState,
    is_loading: bool,
    load_error: Option<&str>,
    has_frame: bool,
) {
    if let Some(error) = load_error {
        let overlay_rect = Rect::from_min_max(
            pos2(rect.left(), rect.bottom() - 26.0),
            pos2(rect.right(), rect.bottom()),
        );
        let overlay_fill = theme
            .mix(theme.colors.bg_base, theme.colors.surface, 0.24)
            .linear_multiply(0.96);
        ui.painter()
            .rect_filled(overlay_rect, CornerRadius::ZERO, overlay_fill);
        ui.painter().text(
            overlay_rect.center(),
            egui::Align2::CENTER_CENTER,
            truncate_filename(error, 72),
            egui::FontId::proportional(10.5),
            theme.colors.fg_dim,
        );
        return;
    }

    if is_loading {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Loading full preview...",
            egui::FontId::proportional(12.5),
            theme.colors.fg,
        );
    } else if !has_frame {
        let fallback_message = match state {
            VideoCompressionState::Probing => "Preview will be ready after probing finishes",
            VideoCompressionState::Compressing(_) => {
                "Preview unavailable while compression is running"
            }
            VideoCompressionState::Failed(_) => "Preview unavailable for this video",
            VideoCompressionState::Cancelled => "Preview unavailable for this video",
            VideoCompressionState::Ready | VideoCompressionState::Completed(_) => {
                "Preview not available yet"
            }
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            fallback_message,
            egui::FontId::proportional(12.5),
            theme.colors.fg_muted,
        );
    }
}

pub(super) fn format_timeline_time(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0).round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}
