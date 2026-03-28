use eframe::egui::{
    Align, Button, CornerRadius, Layout, ProgressBar, RichText, ScrollArea, Stroke, Ui, vec2,
};

use crate::{
    icons,
    modules::compress_audio::models::AudioCompressionState,
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{CompressAudioPage, helpers::queue_subtitle};

impl CompressAudioPage {
    pub(super) fn render_queue_column(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        panel::card(theme).show(ui, |ui| {
            ui.horizontal(|ui| {
                hint::title(
                    ui,
                    theme,
                    "Batch Queue",
                    16.0,
                    Some(
                        "Each file is analyzed first so Smart Mode can choose a better default strategy.",
                    ),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !self.queue.is_empty()
                        && ui
                            .add(
                                Button::new(
                                    RichText::new("Clear Finished")
                                        .size(12.0)
                                        .color(theme.colors.fg),
                                )
                                .fill(theme.colors.bg_raised)
                                .stroke(Stroke::new(1.0, theme.colors.border))
                                .corner_radius(CornerRadius::ZERO),
                            )
                            .clicked()
                    {
                        self.clear_finished();
                    }
                });
            });
            ui.add_space(12.0);

            if self.queue.is_empty() {
                ui.label(
                    RichText::new("No audio files yet. Add a batch to begin.")
                        .size(12.5)
                        .color(theme.colors.fg_dim),
                );
                return;
            }

            ScrollArea::vertical()
                .max_height(height.max(180.0))
                .show(ui, |ui| {
                    let queue_ids = self.queue.iter().map(|item| item.id).collect::<Vec<_>>();
                    for id in queue_ids {
                        self.render_queue_item(ui, theme, id);
                        ui.add_space(8.0);
                    }
                });
        });
    }

    fn render_queue_item(&mut self, ui: &mut Ui, theme: &AppTheme, id: u64) {
        let Some(item) = self.find_item(id) else {
            return;
        };

        let subtitle = queue_subtitle(item);
        let progress = match &item.state {
            AudioCompressionState::Compressing(progress) => Some(progress.progress),
            _ => None,
        };
        let selected = self.selected_id == Some(id);

        let button = Button::new(
            RichText::new(format!("{}\n{}", item.file_name, subtitle))
                .size(12.0)
                .color(theme.colors.fg),
        )
        .fill(if selected {
            theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.14)
        } else {
            theme.colors.bg_raised
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                theme.mix(theme.colors.border, theme.colors.accent, 0.38)
            } else {
                theme.colors.border
            },
        ))
        .corner_radius(CornerRadius::ZERO)
        .min_size(vec2(ui.available_width() - 48.0, 52.0));

        ui.horizontal(|ui| {
            if ui.add(button).clicked() {
                self.selected_id = Some(id);
            }

            if !self.is_compressing()
                && ui
                    .add(
                        Button::new(
                            RichText::new(icons::TRASH)
                                .size(14.0)
                                .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
            {
                self.remove_item(id);
            }
        });

        if let Some(progress) = progress {
            ui.add_space(6.0);
            ui.add(
                ProgressBar::new(progress)
                    .desired_width(ui.available_width())
                    .show_percentage(),
            );
        }
    }
}
