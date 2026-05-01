use eframe::egui::{
    self, Button, Color32, CornerRadius, ProgressBar, RichText, ScrollArea, Stroke, Ui,
};

use crate::{
    icons,
    modules::compress_documents::models::{DocumentCompressionState, DocumentQueueItem},
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{CompressDocumentsPage, compact, format_bytes, truncate_filename};

enum QueueAction {
    Compress(u64),
    Remove(u64),
    Select(u64),
}

impl CompressDocumentsPage {
    pub(super) fn render_queue(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 32.0).max(0.0));
                hint::title(
                    ui,
                    theme,
                    "Queue",
                    15.0,
                    Some("Run one document from its row or compress every ready item at once."),
                );
                ui.add_space(8.0);

                if self.queue.is_empty() {
                    ui.add_space(18.0);
                    ui.vertical_centered(|ui| {
                        ui.label(icons::rich(icons::DOCUMENT, 32.0, theme.colors.fg_dim));
                        ui.label(
                            RichText::new("No documents queued")
                                .size(13.0)
                                .color(theme.colors.fg_dim),
                        );
                    });
                    return;
                }

                let mut action = None;
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height((height - 78.0).max(120.0))
                    .show(ui, |ui| {
                        for item in &self.queue {
                            if let Some(row_action) = render_queue_row(
                                ui,
                                theme,
                                item,
                                self.selected_id == Some(item.asset.id),
                                self.active_batch.is_some(),
                            ) {
                                action = Some(row_action);
                            }
                            ui.separator();
                        }
                    });

                match action {
                    Some(QueueAction::Compress(id)) => self.start_single_compression(id),
                    Some(QueueAction::Remove(id)) => self.remove_document(id),
                    Some(QueueAction::Select(id)) => self.selected_id = Some(id),
                    None => {}
                }
            });
    }
}

fn render_queue_row(
    ui: &mut Ui,
    theme: &AppTheme,
    item: &DocumentQueueItem,
    selected: bool,
    batch_active: bool,
) -> Option<QueueAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        let title_color = if selected {
            theme.colors.accent
        } else {
            theme.colors.fg
        };
        if ui
            .selectable_label(
                selected,
                RichText::new(truncate_filename(&item.asset.file_name, 34))
                    .size(13.0)
                    .strong()
                    .color(title_color),
            )
            .clicked()
        {
            action = Some(QueueAction::Select(item.asset.id));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    !batch_active,
                    Button::new(icons::rich(icons::TRASH, 14.0, theme.colors.fg_dim))
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                )
                .clicked()
            {
                action = Some(QueueAction::Remove(item.asset.id));
            }

            if can_start(&item.state)
                && ui
                    .add_enabled(
                        !batch_active,
                        Button::new(icons::rich(icons::PLAY, 14.0, Color32::BLACK))
                            .fill(theme.colors.accent)
                            .stroke(Stroke::NONE)
                            .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
            {
                action = Some(QueueAction::Compress(item.asset.id));
            }
        });
    });

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!(
                "{} | {}",
                item.asset.kind.label(),
                format_bytes(item.asset.original_size)
            ))
            .size(11.5)
            .color(theme.colors.fg_dim),
        );
        ui.label(
            RichText::new(item.asset.kind.engine_label())
                .size(11.5)
                .color(theme.colors.fg_muted),
        );
    });
    ui.add_space(5.0);
    render_state(ui, theme, &item.state);
    action
}

fn render_state(ui: &mut Ui, theme: &AppTheme, state: &DocumentCompressionState) {
    match state {
        DocumentCompressionState::Ready => {
            ui.label(RichText::new("Ready").size(12.0).color(theme.colors.fg_dim));
        }
        DocumentCompressionState::Compressing(progress) => {
            ui.add(
                ProgressBar::new(progress.progress.clamp(0.0, 1.0))
                    .text(progress.stage.clone())
                    .animate(true),
            );
        }
        DocumentCompressionState::Completed(result) => {
            let color = if result.reduction_percent >= 0.0 {
                theme.colors.positive
            } else {
                theme.colors.caution
            };
            let output_name = result
                .output_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("compressed document");
            ui.label(
                RichText::new(format!(
                    "{} | {} -> {} | {:.1}% saved",
                    truncate_filename(output_name, 28),
                    format_bytes(result.original_size),
                    format_bytes(result.compressed_size),
                    result.reduction_percent
                ))
                .size(12.0)
                .color(color),
            );
        }
        DocumentCompressionState::Failed(error) => {
            ui.label(
                RichText::new(truncate_filename(error, 72))
                    .size(12.0)
                    .color(theme.colors.negative),
            );
        }
        DocumentCompressionState::Cancelled => {
            ui.label(
                RichText::new("Cancelled")
                    .size(12.0)
                    .color(theme.colors.caution),
            );
        }
    }
}

fn can_start(state: &DocumentCompressionState) -> bool {
    matches!(
        state,
        DocumentCompressionState::Ready
            | DocumentCompressionState::Failed(_)
            | DocumentCompressionState::Cancelled
    )
}
