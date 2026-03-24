use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Id, Layout, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};

use crate::{icons, theme::AppTheme, ui::components::panel};

use super::super::models::CompressionState;
use super::{CompressPhotosPage, compact, truncate_filename};

impl CompressPhotosPage {
    pub(super) fn render_preview(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let sel_id = match self.selected_file_id {
            Some(id) => id,
            None => return,
        };
        let (is_done, item_input_path, item_output_path, file_name_display) = {
            let item = match self.files.iter().find(|file| file.asset.id == sel_id) {
                Some(item) => item,
                None => {
                    self.selected_file_id = None;
                    return;
                }
            };

            let is_done = matches!(item.state, CompressionState::Completed(_));
            let input_path = item.asset.path.clone();
            let output_path = if let CompressionState::Completed(result) = &item.state {
                Some(result.output_path.clone())
            } else {
                None
            };
            let name = truncate_filename(&item.asset.file_name, 36);
            (is_done, input_path, output_path, name)
        };

        if is_done
            && self.preview_output_texture.as_ref().map(|(id, _)| *id) != Some(sel_id)
            && !self.preview_loading
            && !self.preview_output_failed
        {
            if let Some(out_path) = item_output_path.clone() {
                self.spawn_preview_load(ctx, sel_id, item_input_path.clone(), Some(out_path));
            }
        }

        panel::card(theme)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 24.0).max(0.0));

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&file_name_display)
                            .size(12.0)
                            .strong()
                            .color(theme.colors.fg),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("{}", icons::CLOSE))
                                        .family(icons::font_family())
                                        .size(14.0)
                                        .color(theme.colors.fg_dim),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                            )
                            .clicked()
                        {
                            self.selected_file_id = None;
                        }
                        if ui
                            .add(
                                Button::new(icons::rich(
                                    icons::ZOOM_OUT,
                                    13.0,
                                    theme.colors.fg_dim,
                                ))
                                .fill(theme.colors.bg_raised)
                                .stroke(Stroke::new(1.0, theme.colors.border))
                                .corner_radius(CornerRadius::ZERO),
                            )
                            .clicked()
                        {
                            self.preview_zoom = (self.preview_zoom - 0.25).max(0.25);
                        }
                        ui.label(
                            RichText::new(format!("{:.0}%", self.preview_zoom * 100.0))
                                .size(10.0)
                                .color(theme.colors.fg_dim),
                        );
                        if ui
                            .add(
                                Button::new(icons::rich(icons::ZOOM_IN, 13.0, theme.colors.fg_dim))
                                    .fill(theme.colors.bg_raised)
                                    .stroke(Stroke::new(1.0, theme.colors.border))
                                    .corner_radius(CornerRadius::ZERO),
                            )
                            .clicked()
                        {
                            self.preview_zoom = (self.preview_zoom + 0.25).min(5.0);
                        }
                    });
                });
                ui.add_space(6.0);

                let avail = vec2(ui.available_width(), (height - 64.0).max(40.0));
                let (img_rect, img_resp) = ui.allocate_exact_size(avail, Sense::click_and_drag());

                ui.painter()
                    .rect_filled(img_rect, CornerRadius::ZERO, theme.colors.bg_base);
                ui.painter().rect_stroke(
                    img_rect,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, theme.colors.border),
                    StrokeKind::Middle,
                );

                let scroll = ctx.input(|input| input.smooth_scroll_delta.y);
                if img_resp.hovered() && scroll.abs() > 0.1 {
                    self.preview_zoom = (self.preview_zoom + scroll * 0.002).clamp(0.25, 5.0);
                }

                if img_resp.dragged() {
                    self.preview_offset += img_resp.drag_delta();
                }

                let has_output_tex = is_done
                    && self.preview_output_texture.as_ref().map(|(id, _)| *id) == Some(sel_id);
                let split = self.before_after_split;
                let zoom = self.preview_zoom;
                let offset = self.preview_offset;

                if has_output_tex {
                    let original_tex = self
                        .preview_input_texture
                        .as_ref()
                        .filter(|(id, _)| *id == sel_id)
                        .map(|(_, texture)| texture);
                    let output_tex = self
                        .preview_output_texture
                        .as_ref()
                        .map(|(_, texture)| texture);

                    if let (Some(original), Some(output)) = (original_tex, output_tex) {
                        let clip = ui.painter().with_clip_rect(img_rect);
                        let original_size = original.size_vec2();
                        let scale = (img_rect.width() / original_size.x)
                            .min(img_rect.height() / original_size.y)
                            * zoom;
                        let draw_rect = Rect::from_center_size(
                            img_rect.center() + offset,
                            original_size * scale,
                        );
                        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                        let split_x = img_rect.left() + img_rect.width() * split;

                        clip.with_clip_rect(Rect::from_min_max(
                            img_rect.min,
                            pos2(split_x, img_rect.max.y),
                        ))
                        .image(
                            original.id(),
                            draw_rect,
                            uv,
                            Color32::WHITE,
                        );
                        clip.with_clip_rect(Rect::from_min_max(
                            pos2(split_x, img_rect.min.y),
                            img_rect.max,
                        ))
                        .image(output.id(), draw_rect, uv, Color32::WHITE);

                        self.before_after_split =
                            draw_slider_chrome(ui, &clip, img_rect, split, sel_id, theme);
                    }
                } else if is_done && self.preview_output_failed {
                    let input_tex = self
                        .preview_input_texture
                        .as_ref()
                        .filter(|(id, _)| *id == sel_id)
                        .map(|(_, texture)| texture);

                    if let Some(texture) = input_tex {
                        let texture_size = texture.size_vec2();
                        let scale = (img_rect.width() / texture_size.x)
                            .min(img_rect.height() / texture_size.y)
                            * zoom;
                        let draw_rect = Rect::from_center_size(
                            img_rect.center() + offset,
                            texture_size * scale,
                        );
                        ui.painter().with_clip_rect(img_rect).image(
                            texture.id(),
                            draw_rect,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );

                        let overlay = egui::Color32::from_rgba_premultiplied(0, 0, 0, 160);
                        let label_rect = Rect::from_min_max(
                            pos2(img_rect.left(), img_rect.bottom() - 22.0),
                            img_rect.max,
                        );
                        ui.painter()
                            .rect_filled(label_rect, CornerRadius::ZERO, overlay);
                        ui.painter().text(
                            pos2(img_rect.center().x, img_rect.bottom() - 11.0),
                            egui::Align2::CENTER_CENTER,
                            "AVIF output preview unavailable. Showing the original image only.",
                            egui::FontId::proportional(10.0),
                            theme.colors.fg_dim,
                        );
                    } else if self.preview_loading {
                        let pct = (self.preview_load_progress * 100.0) as u32;
                        ui.painter().text(
                            img_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("Loading preview... {pct}%"),
                            egui::FontId::proportional(13.0),
                            theme.colors.fg_muted,
                        );
                    }
                } else {
                    let preview_tex = self
                        .preview_input_texture
                        .as_ref()
                        .filter(|(id, _)| *id == sel_id)
                        .map(|(_, texture)| texture);

                    if let Some(texture) = preview_tex {
                        let texture_size = texture.size_vec2();
                        let scale = (img_rect.width() / texture_size.x)
                            .min(img_rect.height() / texture_size.y)
                            * zoom;
                        let draw_rect = Rect::from_center_size(
                            img_rect.center() + offset,
                            texture_size * scale,
                        );
                        ui.painter().with_clip_rect(img_rect).image(
                            texture.id(),
                            draw_rect,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        if self.preview_loading {
                            let pct = (self.preview_load_progress * 100.0) as u32;
                            ui.painter().text(
                                pos2(img_rect.left() + 8.0, img_rect.bottom() - 8.0),
                                egui::Align2::LEFT_BOTTOM,
                                format!("Loading preview... {pct}%"),
                                egui::FontId::proportional(11.0),
                                theme.colors.fg_muted,
                            );
                        }
                    } else if self.preview_loading {
                        let pct = (self.preview_load_progress * 100.0) as u32;
                        ui.painter().text(
                            img_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("Loading preview... {pct}%"),
                            egui::FontId::proportional(13.0),
                            theme.colors.fg_muted,
                        );
                    } else {
                        ui.painter().text(
                            img_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Preview not available",
                            egui::FontId::proportional(13.0),
                            theme.colors.fg_muted,
                        );
                    }
                }

                if img_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                if img_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
            });
    }
}

fn draw_slider_chrome(
    ui: &mut Ui,
    clip: &egui::Painter,
    img_rect: Rect,
    split: f32,
    sel_id: u64,
    theme: &AppTheme,
) -> f32 {
    let split_x = img_rect.left() + img_rect.width() * split;

    clip.vline(
        split_x,
        img_rect.y_range(),
        Stroke::new(2.0, theme.colors.accent),
    );

    let handle_rect = Rect::from_center_size(pos2(split_x, img_rect.center().y), vec2(28.0, 20.0));
    clip.rect_filled(handle_rect, CornerRadius::same(3), theme.colors.accent);
    clip.text(
        handle_rect.center(),
        egui::Align2::CENTER_CENTER,
        "< >",
        egui::FontId::proportional(11.0),
        Color32::BLACK,
    );

    clip.text(
        pos2(img_rect.left() + 8.0, img_rect.top() + 8.0),
        egui::Align2::LEFT_TOP,
        "Before",
        egui::FontId::proportional(11.0),
        theme.colors.fg_dim,
    );
    clip.text(
        pos2(img_rect.right() - 8.0, img_rect.top() + 8.0),
        egui::Align2::RIGHT_TOP,
        "After",
        egui::FontId::proportional(11.0),
        theme.colors.positive,
    );

    let divider_resp = ui.interact(
        Rect::from_center_size(
            pos2(split_x, img_rect.center().y),
            vec2(20.0, img_rect.height()),
        ),
        Id::new("ba_slider").with(sel_id),
        Sense::drag(),
    );

    if divider_resp.dragged() {
        let dx = divider_resp.drag_delta().x;
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
        (split + dx / img_rect.width()).clamp(0.02, 0.98)
    } else {
        if divider_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
        }
        split
    }
}
