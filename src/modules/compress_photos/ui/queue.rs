use eframe::egui::{self, ScrollArea, Ui, Vec2};

use crate::{theme::AppTheme, ui::components::panel};

use super::super::models::CompressionState;
use super::{
    CompressPhotosPage, compact, flush,
    widgets::{queue_row_interactive, queue_section_header},
};

impl CompressPhotosPage {
    pub(super) fn render_queue(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        if self.files.is_empty() {
            return;
        }

        let mut clicked_id: Option<u64> = None;
        let mut delete_id: Option<u64> = None;
        let is_compressing = self.active_batch.is_some();

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                let scroll_h = (height - 42.0).max(0.0);
                ScrollArea::vertical()
                    .id_salt("queue_scroll")
                    .auto_shrink([false, false])
                    .max_height(scroll_h)
                    .show(ui, |ui| {
                        flush(ui);
                        ui.set_width(ui.available_width());

                        let queued: Vec<usize> = self
                            .files
                            .iter()
                            .enumerate()
                            .filter(|(_, file)| matches!(file.state, CompressionState::Ready))
                            .map(|(index, _)| index)
                            .collect();
                        let in_progress: Vec<usize> = self
                            .files
                            .iter()
                            .enumerate()
                            .filter(|(_, file)| {
                                matches!(file.state, CompressionState::Compressing(_))
                            })
                            .map(|(index, _)| index)
                            .collect();
                        let finished: Vec<usize> = self
                            .files
                            .iter()
                            .enumerate()
                            .filter(|(_, file)| {
                                matches!(
                                    file.state,
                                    CompressionState::Completed(_) | CompressionState::Failed(_)
                                )
                            })
                            .map(|(index, _)| index)
                            .collect();
                        let cancelled: Vec<usize> = self
                            .files
                            .iter()
                            .enumerate()
                            .filter(|(_, file)| matches!(file.state, CompressionState::Cancelled))
                            .map(|(index, _)| index)
                            .collect();

                        if !queued.is_empty() {
                            queue_section_header(
                                ui,
                                theme,
                                "Queue",
                                queued.len(),
                                theme.colors.fg_muted,
                            );
                            for &index in &queued {
                                let action = queue_row_interactive(
                                    ui,
                                    theme,
                                    &self.files[index],
                                    true,
                                    !is_compressing,
                                );
                                if action.clicked {
                                    clicked_id = Some(self.files[index].asset.id);
                                }
                                if action.deleted {
                                    delete_id = Some(self.files[index].asset.id);
                                }
                            }
                            ui.add_space(8.0);
                        }

                        if !in_progress.is_empty() {
                            queue_section_header(
                                ui,
                                theme,
                                "Progress",
                                in_progress.len(),
                                theme.colors.accent,
                            );
                            for &index in &in_progress {
                                let action = queue_row_interactive(
                                    ui,
                                    theme,
                                    &self.files[index],
                                    false,
                                    false,
                                );
                                if action.clicked {
                                    clicked_id = Some(self.files[index].asset.id);
                                }
                            }
                            ui.add_space(8.0);
                        }

                        if !finished.is_empty() {
                            queue_section_header(
                                ui,
                                theme,
                                "Done",
                                finished.len(),
                                theme.colors.positive,
                            );
                            for &index in &finished {
                                let action = queue_row_interactive(
                                    ui,
                                    theme,
                                    &self.files[index],
                                    false,
                                    false,
                                );
                                if action.clicked {
                                    clicked_id = Some(self.files[index].asset.id);
                                }
                            }
                            ui.add_space(8.0);
                        }

                        if !cancelled.is_empty() {
                            queue_section_header(
                                ui,
                                theme,
                                "Cancelled",
                                cancelled.len(),
                                theme.colors.caution,
                            );
                            for &index in &cancelled {
                                let action = queue_row_interactive(
                                    ui,
                                    theme,
                                    &self.files[index],
                                    false,
                                    false,
                                );
                                if action.clicked {
                                    clicked_id = Some(self.files[index].asset.id);
                                }
                            }
                        }
                    });
            });

        if let Some(id) = delete_id {
            self.files.retain(|file| file.asset.id != id);
            if self.selected_file_id == Some(id) {
                self.selected_file_id = None;
            }
        }

        if let Some(id) = clicked_id {
            if self.selected_file_id == Some(id) {
                self.selected_file_id = None;
                return;
            }

            self.selected_file_id = Some(id);
            self.preview_zoom = 1.0;
            self.preview_offset = Vec2::ZERO;
            self.before_after_split = 0.5;

            if let Some(item) = self.files.iter().find(|file| file.asset.id == id) {
                let input_path = item.asset.path.clone();
                let output_path = if let CompressionState::Completed(result) = &item.state {
                    Some(result.output_path.clone())
                } else {
                    None
                };

                self.reset_preview_state();
                self.spawn_preview_load(ui.ctx(), id, input_path, output_path);
            }
        }
    }
}
