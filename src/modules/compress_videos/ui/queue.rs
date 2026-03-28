use eframe::egui::{self, Color32, ColorImage, ScrollArea, TextureOptions, Ui};

use crate::{
    modules::compress_videos::{engine::VideoEngineController, models::VideoCompressionState},
    theme::AppTheme,
    ui::components::panel,
};

use super::{
    BannerMessage, BannerTone, CompressVideosPage, compact, flush, is_video_settings_editable,
    widgets::{QueuePrimaryAction, queue_section_header, video_queue_row},
};

struct QueueCategory {
    title: &'static str,
    tint: Color32,
}

impl CompressVideosPage {
    pub(super) fn render_queue(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
        use_hardware_acceleration: bool,
    ) {
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
        let mut start_id = None;
        let mut cancel_current_video = false;
        let mut locked_settings_click = false;
        let categories = queue_categories(theme);
        let mut category_indices: [Vec<usize>; 5] = std::array::from_fn(|_| Vec::new());

        for (index, item) in self.queue.iter().enumerate() {
            category_indices[queue_category_index(&item.state)].push(index);
        }

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

                        for (category, indices) in
                            categories.into_iter().zip(category_indices.iter())
                        {
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
                            for &index in indices {
                                let item = &self.queue[index];
                                let settings_editable = is_video_settings_editable(&item.state);
                                let selected =
                                    settings_editable && self.selected_id == Some(item.id);
                                let is_pending = self.is_video_pending_compression(item.id);
                                let can_delete = !self.has_pending_compression()
                                    && matches!(
                                        item.state,
                                        VideoCompressionState::Ready
                                            | VideoCompressionState::Failed(_)
                                            | VideoCompressionState::Cancelled
                                    );
                                let primary_action = match &item.state {
                                    VideoCompressionState::Ready if is_pending => {
                                        QueuePrimaryAction::Queued
                                    }
                                    VideoCompressionState::Ready => {
                                        QueuePrimaryAction::StartCompress
                                    }
                                    VideoCompressionState::Compressing(_) => {
                                        QueuePrimaryAction::Cancel
                                    }
                                    _ => QueuePrimaryAction::None,
                                };
                                let thumbnail = self.thumbnail_textures.get(&item.id);
                                let action = video_queue_row(
                                    ui,
                                    theme,
                                    item,
                                    selected,
                                    can_delete,
                                    primary_action,
                                    thumbnail,
                                );

                                if action.clicked && settings_editable {
                                    clicked_id = Some(item.id);
                                } else if action.clicked {
                                    locked_settings_click = true;
                                }

                                if action.deleted {
                                    delete_id = Some(item.id);
                                }

                                if action.start_requested {
                                    start_id = Some(item.id);
                                }

                                if action.cancel_requested {
                                    cancel_current_video = true;
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
                self.reset_preview_state();
            }
        }

        if let Some(id) = start_id {
            self.start_single_compression(id, engine, use_hardware_acceleration);
        }

        if cancel_current_video {
            self.cancel_active_video();
        }

        if let Some(id) = clicked_id {
            self.selected_id = if self.selected_id == Some(id) {
                None
            } else {
                Some(id)
            };
            if self.selected_id.is_none() {
                self.reset_preview_state();
            }
        }

        if locked_settings_click {
            self.selected_id = None;
            self.reset_preview_state();
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Settings are only available for videos that are still in the queue.".into(),
            });
        }
    }
}

fn queue_categories(theme: &AppTheme) -> [QueueCategory; 5] {
    [
        QueueCategory {
            title: "Probing",
            tint: theme.colors.fg_muted,
        },
        QueueCategory {
            title: "Queue",
            tint: theme.colors.fg_muted,
        },
        QueueCategory {
            title: "Progress",
            tint: theme.colors.accent,
        },
        QueueCategory {
            title: "Done",
            tint: theme.colors.positive,
        },
        QueueCategory {
            title: "Cancelled",
            tint: theme.colors.caution,
        },
    ]
}

fn queue_category_index(state: &VideoCompressionState) -> usize {
    match state {
        VideoCompressionState::Probing => 0,
        VideoCompressionState::Ready => 1,
        VideoCompressionState::Compressing(_) => 2,
        VideoCompressionState::Completed(_) | VideoCompressionState::Failed(_) => 3,
        VideoCompressionState::Cancelled => 4,
    }
}
