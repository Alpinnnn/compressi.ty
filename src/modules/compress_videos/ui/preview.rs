use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Button, ColorImage, CornerRadius, Layout, Rect, RichText, Sense, Slider, Stroke,
    StrokeKind, TextureOptions, Ui, pos2, vec2,
};

use crate::{
    icons,
    modules::compress_videos::{
        engine::VideoEngineController,
        models::{PreviewClickFeedback, PreviewClickFeedbackIcon},
    },
    theme::AppTheme,
    ui::components::panel,
};

use super::{
    CompressVideosPage, compact,
    preview_helpers::{
        format_timeline_time, paint_centered_texture, paint_pause_feedback_icon,
        paint_play_feedback_icon, paint_preview_status_overlay, render_preview_message,
    },
    truncate_filename,
    widgets::{format_bytes, format_duration},
};

const PLAYER_CLICK_FEEDBACK_DURATION_SECS: f32 = 0.55;

impl CompressVideosPage {
    pub(super) fn render_preview_panel(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
    ) {
        self.ensure_preview_for_selection(engine);
        self.ensure_preview_texture(ctx);

        panel::card(theme)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 24.0).max(0.0));

                let Some(selected_id) = self.selected_id else {
                    render_preview_message(
                        ui,
                        theme,
                        height,
                        "Select a video from the queue to preview it here.",
                    );
                    return;
                };

                let Some(selected_index) =
                    self.queue.iter().position(|item| item.id == selected_id)
                else {
                    self.selected_id = None;
                    self.reset_preview_state();
                    render_preview_message(
                        ui,
                        theme,
                        height,
                        "Select a video from the queue to preview it here.",
                    );
                    return;
                };

                let file_name_display =
                    truncate_filename(&self.queue[selected_index].file_name, 36);
                let metadata_line = self.queue[selected_index]
                    .metadata
                    .as_ref()
                    .map(|metadata| {
                        format!(
                            "{} | {} | {}x{}",
                            format_bytes(metadata.size_bytes),
                            format_duration(metadata.duration_secs),
                            metadata.width,
                            metadata.height
                        )
                    })
                    .unwrap_or_else(|| "Preview will be ready after probing finishes.".to_owned());

                let can_control = self.preview_state.item_id == Some(selected_id)
                    && self.preview_state.load_error.is_none()
                    && self.preview_state.duration_secs > 0.0;

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(file_name_display)
                                .size(12.0)
                                .strong()
                                .color(theme.colors.fg),
                        );
                        ui.label(
                            RichText::new(metadata_line)
                                .size(10.5)
                                .color(theme.colors.fg_dim),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("{}", icons::CLOSE))
                                        .family(icons::font_family())
                                        .size(14.0)
                                        .color(theme.colors.fg_dim),
                                )
                                .fill(theme.colors.bg_base.linear_multiply(0.0))
                                .stroke(Stroke::NONE),
                            )
                            .clicked()
                        {
                            self.selected_id = None;
                            self.reset_preview_state();
                        }
                    });
                });
                ui.add_space(6.0);

                let player_height = (height - 56.0).max(124.0);
                let (player_rect, player_response) = ui
                    .allocate_exact_size(vec2(ui.available_width(), player_height), Sense::click());

                ui.painter()
                    .rect_filled(player_rect, CornerRadius::ZERO, theme.colors.bg_base);
                ui.painter().rect_stroke(
                    player_rect,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, theme.colors.border),
                    StrokeKind::Middle,
                );

                self.ensure_thumbnail_texture(ctx, selected_index, selected_id);
                let active_texture = self.preview_texture.as_ref();
                let fallback_texture = self.thumbnail_textures.get(&selected_id);

                if let Some(texture) = active_texture.or(fallback_texture) {
                    paint_centered_texture(ui, player_rect, texture);
                } else {
                    ui.painter().text(
                        player_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icons::VIDEO,
                        icons::font_id(26.0),
                        theme.colors.fg_muted,
                    );
                }

                paint_preview_status_overlay(
                    ui,
                    theme,
                    player_rect,
                    &self.queue[selected_index].state,
                    self.preview_state.is_loading,
                    self.preview_state.load_error.as_deref(),
                    active_texture.is_some(),
                );
                let feedback_active =
                    self.paint_player_click_feedback(ui, theme, player_rect, can_control);
                let controls_consumed_click = self.render_player_overlay_controls(
                    ui,
                    ctx,
                    theme,
                    engine,
                    player_rect,
                    &player_response,
                    can_control,
                );

                if player_response.clicked()
                    && can_control
                    && !self.preview_state.is_loading
                    && !controls_consumed_click
                {
                    let feedback_icon = if self.preview_state.is_playing {
                        PreviewClickFeedbackIcon::Pause
                    } else {
                        PreviewClickFeedbackIcon::Play
                    };
                    if self.is_preview_at_end() && !self.preview_state.is_playing {
                        self.restart_preview(engine);
                    } else {
                        self.toggle_preview_playback(engine);
                    }
                    self.preview_state.click_feedback = Some(PreviewClickFeedback {
                        icon: feedback_icon,
                        shown_at: Instant::now(),
                    });
                }

                if self.preview_state.is_loading
                    || self.running_preview_stream.is_some()
                    || feedback_active
                {
                    let frame_rate = self.preview_state.preview_frame_rate.max(12.0);
                    ctx.request_repaint_after(Duration::from_secs_f32(1.0 / frame_rate));
                }
            });
    }

    fn render_player_overlay_controls(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        engine: &VideoEngineController,
        player_rect: Rect,
        player_response: &egui::Response,
        can_control: bool,
    ) -> bool {
        let duration_secs = self.preview_state.duration_secs.max(0.0);
        let mut timeline_position = self.displayed_preview_position_secs();
        let controls_visible = can_control
            && (player_response.contains_pointer()
                || player_response.is_pointer_button_down_on()
                || self.preview_state.scrub_position_secs.is_some());
        let controls_alpha = ctx.animate_bool(
            ui.id()
                .with(("video-preview-overlay-controls", self.preview_state.item_id)),
            controls_visible,
        );

        if controls_alpha <= 0.01 {
            return false;
        }

        let overlay_rect = Rect::from_min_max(
            pos2(player_rect.left() + 10.0, player_rect.bottom() - 64.0),
            pos2(player_rect.right() - 10.0, player_rect.bottom() - 12.0),
        );
        let overlay_fill = theme
            .mix(theme.colors.bg_base, theme.colors.surface, 0.42)
            .linear_multiply(0.96 * controls_alpha);
        let overlay_stroke = Stroke::new(
            1.0,
            theme
                .mix(theme.colors.border, theme.colors.border_focus, 0.22)
                .linear_multiply(controls_alpha),
        );
        ui.painter()
            .rect_filled(overlay_rect, CornerRadius::same(10), overlay_fill);
        ui.painter().rect_stroke(
            overlay_rect,
            CornerRadius::same(10),
            overlay_stroke,
            StrokeKind::Middle,
        );

        let inner_rect = Rect::from_min_max(
            pos2(overlay_rect.left() + 10.0, overlay_rect.top() + 7.0),
            pos2(overlay_rect.right() - 10.0, overlay_rect.bottom() - 10.0),
        );
        let mut overlay_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        overlay_ui.set_width(inner_rect.width());
        overlay_ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        overlay_ui.spacing_mut().slider_width = inner_rect.width();

        let mut consumed_click = false;
        overlay_ui.horizontal(|ui| {
            ui.label(
                RichText::new(format_timeline_time(self.displayed_preview_position_secs()))
                    .size(10.5)
                    .color(theme.colors.fg),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format_timeline_time(duration_secs))
                        .size(10.5)
                        .color(theme.colors.fg),
                );
            });
        });

        let slider_response = overlay_ui.add(
            Slider::new(&mut timeline_position, 0.0..=duration_secs.max(0.01)).show_value(false),
        );

        if slider_response.drag_started() {
            self.begin_preview_scrub();
            self.update_preview_scrub(timeline_position);
            consumed_click = true;
        } else if slider_response.dragged() {
            self.update_preview_scrub(timeline_position);
            consumed_click = true;
        } else if slider_response.drag_stopped() {
            self.finish_preview_scrub(engine, timeline_position);
            consumed_click = true;
        } else if slider_response.changed() {
            self.seek_preview(engine, timeline_position, self.preview_state.is_playing);
            consumed_click = true;
        }

        consumed_click
    }

    fn paint_player_click_feedback(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        player_rect: Rect,
        can_control: bool,
    ) -> bool {
        if !can_control {
            self.preview_state.click_feedback = None;
            return false;
        }

        let Some(feedback) = self.preview_state.click_feedback else {
            return false;
        };

        let elapsed = feedback.shown_at.elapsed().as_secs_f32();
        if elapsed >= PLAYER_CLICK_FEEDBACK_DURATION_SECS {
            self.preview_state.click_feedback = None;
            return false;
        }

        let fade = 1.0 - elapsed / PLAYER_CLICK_FEEDBACK_DURATION_SECS;
        let bubble_color = theme
            .mix(theme.colors.bg_base, theme.colors.surface, 0.28)
            .linear_multiply(0.9 * fade.max(0.35));
        let icon_color = theme.colors.fg.linear_multiply(fade.max(0.35));
        let center = player_rect.center();
        let radius = 28.0;

        ui.painter().circle_filled(center, radius, bubble_color);
        match feedback.icon {
            PreviewClickFeedbackIcon::Play => paint_play_feedback_icon(ui, center, icon_color),
            PreviewClickFeedbackIcon::Pause => paint_pause_feedback_icon(ui, center, icon_color),
        }

        true
    }

    fn ensure_preview_texture(&mut self, ctx: &egui::Context) {
        let Some(frame) = self.preview_state.frame.as_ref() else {
            return;
        };
        if !self.preview_texture_dirty && self.preview_texture.is_some() {
            return;
        }

        let color_image = ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        if let Some(texture) = self.preview_texture.as_mut()
            && texture.size() == [frame.width as usize, frame.height as usize]
        {
            texture.set(color_image, TextureOptions::LINEAR);
        } else if let Some(item_id) = self.preview_state.item_id {
            self.preview_texture = Some(ctx.load_texture(
                format!("video-preview-{item_id}"),
                color_image,
                TextureOptions::LINEAR,
            ));
        }

        self.preview_texture_dirty = false;
    }

    fn ensure_thumbnail_texture(&mut self, ctx: &egui::Context, item_index: usize, item_id: u64) {
        if self.thumbnail_textures.contains_key(&item_id) {
            return;
        }

        let color_image = {
            let Some(thumbnail) = self
                .queue
                .get(item_index)
                .and_then(|item| item.thumbnail.as_ref())
            else {
                return;
            };

            ColorImage::from_rgba_unmultiplied(
                [thumbnail.width as usize, thumbnail.height as usize],
                &thumbnail.rgba,
            )
        };

        let texture = ctx.load_texture(
            format!("video-thumb-{item_id}"),
            color_image,
            TextureOptions::LINEAR,
        );
        self.thumbnail_textures.insert(item_id, texture);
    }
}
