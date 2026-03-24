use eframe::egui::{self, Color32, ColorImage, ScrollArea, TextureOptions, Ui};

use crate::{
    modules::compress_videos::models::VideoCompressionState, theme::AppTheme, ui::components::panel,
};

use super::{
    BannerMessage, BannerTone, CompressVideosPage, compact, flush, is_video_settings_editable,
    widgets::{queue_section_header, video_queue_row},
};

struct QueueCategory {
    title: &'static str,
    tint: Color32,
    matches_state: fn(&VideoCompressionState) -> bool,
}

impl CompressVideosPage {
    pub(super) fn render_queue(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        if self.queue.is_empty() {
            return;
        }

        for item in &self.queue {
            if let Some(thumbnail) = &item.thumbnail {
                if !self.thumbnail_textures.contains_key(&item.id) {
                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [thumbnail.width as usize, thumbnail.height as usize],
                        &thumbnail.rgba,
                    );
                    let texture = ui.ctx().load_texture(
                        format!("video-thumb-{}", item.id),
                        color_image,
                        TextureOptions::LINEAR,
                    );
                    self.thumbnail_textures.insert(item.id, texture);
                }
            }
        }

        let mut clicked_id = None;
        let mut delete_id = None;
        let mut locked_settings_click = false;
        let is_compressing = self.active_batch.is_some();
        let categories = queue_categories(theme);

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));
                let scroll_height = (height - 42.0).max(0.0);
                ScrollArea::vertical()
                    .id_salt("video_queue_scroll")
                    .auto_shrink([false, false])
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        flush(ui);
                        ui.set_width(ui.available_width());

                        for category in categories {
                            let indices: Vec<usize> = self
                                .queue
                                .iter()
                                .enumerate()
                                .filter(|(_, item)| (category.matches_state)(&item.state))
                                .map(|(index, _)| index)
                                .collect();
                            if indices.is_empty() {
                                continue;
                            }

                            queue_section_header(
                                ui,
                                theme,
                                category.title,
                                indices.len(),
                                category.tint,
                            );
                            for &index in &indices {
                                let item = &self.queue[index];
                                let settings_editable = is_video_settings_editable(&item.state);
                                let selected =
                                    settings_editable && self.selected_id == Some(item.id);
                                let can_delete = !is_compressing
                                    && matches!(
                                        item.state,
                                        VideoCompressionState::Ready
                                            | VideoCompressionState::Failed(_)
                                            | VideoCompressionState::Cancelled
                                    );
                                let thumbnail = self.thumbnail_textures.get(&item.id);
                                let action = video_queue_row(
                                    ui, theme, item, selected, can_delete, thumbnail,
                                );

                                if action.clicked && settings_editable {
                                    clicked_id = Some(item.id);
                                } else if action.clicked {
                                    locked_settings_click = true;
                                }

                                if action.deleted {
                                    delete_id = Some(item.id);
                                }
                            }
                            ui.add_space(8.0);
                        }
                    });
            });

        if let Some(id) = delete_id {
            self.queue.retain(|item| item.id != id);
            self.thumbnail_textures.remove(&id);
            if self.selected_id == Some(id) {
                self.selected_id = None;
            }
        }

        if let Some(id) = clicked_id {
            self.selected_id = if self.selected_id == Some(id) {
                None
            } else {
                Some(id)
            };
        }

        if locked_settings_click {
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Settings are only available for videos that are still editable.".into(),
            });
        }
    }
}

fn queue_categories(theme: &AppTheme) -> [QueueCategory; 4] {
    [
        QueueCategory {
            title: "Probing",
            tint: theme.colors.fg_muted,
            matches_state: is_probing,
        },
        QueueCategory {
            title: "Queue",
            tint: theme.colors.fg_muted,
            matches_state: is_ready,
        },
        QueueCategory {
            title: "Progress",
            tint: theme.colors.accent,
            matches_state: is_processing,
        },
        QueueCategory {
            title: "Done",
            tint: theme.colors.positive,
            matches_state: is_finished,
        },
    ]
}

fn is_probing(state: &VideoCompressionState) -> bool {
    matches!(state, VideoCompressionState::Probing)
}

fn is_ready(state: &VideoCompressionState) -> bool {
    matches!(state, VideoCompressionState::Ready)
}

fn is_processing(state: &VideoCompressionState) -> bool {
    matches!(state, VideoCompressionState::Compressing(_))
}

fn is_finished(state: &VideoCompressionState) -> bool {
    matches!(
        state,
        VideoCompressionState::Completed(_)
            | VideoCompressionState::Failed(_)
            | VideoCompressionState::Cancelled
    )
}
