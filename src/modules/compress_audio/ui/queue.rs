use eframe::egui::{self, ScrollArea, Ui};

use crate::{
    modules::{
        compress_audio::models::AudioCompressionState,
        compress_videos::engine::VideoEngineController,
    },
    theme::AppTheme,
    ui::components::panel,
};

use super::{
    BannerMessage, BannerTone, CompressAudioPage, compact, flush, is_audio_settings_editable,
    widgets::{QueuePrimaryAction, audio_queue_row, queue_section_header},
};

struct QueueCategory {
    title: &'static str,
    tint: egui::Color32,
    matches_state: fn(&AudioCompressionState) -> bool,
}

impl CompressAudioPage {
    pub(super) fn render_queue(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        height: f32,
        engine: &VideoEngineController,
    ) {
        if self.queue.is_empty() {
            return;
        }

        let mut clicked_id = None;
        let mut delete_id = None;
        let mut start_id = None;
        let mut cancel_current_audio = false;
        let mut locked_settings_click = false;
        let categories = queue_categories(theme);

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));
                let scroll_height = (height - 42.0).max(0.0);
                ScrollArea::vertical()
                    .id_salt("audio_queue_scroll")
                    .auto_shrink([false, false])
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        flush(ui);
                        ui.set_width(ui.available_width());

                        for category in categories {
                            let indices = self
                                .queue
                                .iter()
                                .enumerate()
                                .filter(|(_, item)| (category.matches_state)(&item.state))
                                .map(|(index, _)| index)
                                .collect::<Vec<_>>();
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
                                let settings_editable = is_audio_settings_editable(&item.state);
                                let selected =
                                    settings_editable && self.selected_id == Some(item.id);
                                let is_pending = self.is_audio_pending_compression(item.id);
                                let can_delete = !self.has_pending_compression()
                                    && matches!(
                                        &item.state,
                                        AudioCompressionState::Ready
                                            | AudioCompressionState::Failed(_)
                                            | AudioCompressionState::Cancelled
                                    );
                                let primary_action = match &item.state {
                                    AudioCompressionState::Ready if is_pending => {
                                        QueuePrimaryAction::Queued
                                    }
                                    AudioCompressionState::Ready => {
                                        QueuePrimaryAction::StartCompress
                                    }
                                    AudioCompressionState::Compressing(_) => {
                                        QueuePrimaryAction::Cancel
                                    }
                                    _ => QueuePrimaryAction::None,
                                };
                                let action = audio_queue_row(
                                    ui,
                                    theme,
                                    item,
                                    selected,
                                    can_delete,
                                    primary_action,
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
                                    cancel_current_audio = true;
                                }
                            }
                            ui.add_space(8.0);
                        }
                    });
            });

        if let Some(id) = delete_id {
            self.remove_item(id);
        }

        if let Some(id) = start_id {
            self.start_single_compression(id, engine);
        }

        if cancel_current_audio {
            self.cancel_active_audio();
        }

        if let Some(id) = clicked_id {
            self.selected_id = if self.selected_id == Some(id) {
                None
            } else {
                Some(id)
            };
        }

        if locked_settings_click {
            self.selected_id = None;
            self.banner = Some(BannerMessage {
                tone: BannerTone::Info,
                text: "Settings are only available for audio files that are still in the queue."
                    .to_owned(),
            });
        }
    }
}

fn queue_categories(theme: &AppTheme) -> [QueueCategory; 5] {
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
        QueueCategory {
            title: "Cancelled",
            tint: theme.colors.caution,
            matches_state: is_cancelled,
        },
    ]
}

fn is_probing(state: &AudioCompressionState) -> bool {
    matches!(state, AudioCompressionState::Analyzing)
}

fn is_ready(state: &AudioCompressionState) -> bool {
    matches!(state, AudioCompressionState::Ready)
}

fn is_processing(state: &AudioCompressionState) -> bool {
    matches!(state, AudioCompressionState::Compressing(_))
}

fn is_finished(state: &AudioCompressionState) -> bool {
    matches!(
        state,
        AudioCompressionState::Completed(_)
            | AudioCompressionState::Failed(_)
            | AudioCompressionState::Skipped(_)
    )
}

fn is_cancelled(state: &AudioCompressionState) -> bool {
    matches!(state, AudioCompressionState::Cancelled)
}
