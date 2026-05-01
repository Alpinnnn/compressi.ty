use eframe::egui::{self, Button, Color32, CornerRadius, RichText, Slider, Stroke, Ui, vec2};

use crate::{
    icons,
    modules::compress_documents::models::{DocumentCompressionPreset, DocumentCompressionState},
    runtime,
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{CompressDocumentsPage, compact};

impl CompressDocumentsPage {
    pub(super) fn render_settings_panel(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 32.0).max(0.0));
                hint::title(
                    ui,
                    theme,
                    "Settings",
                    15.0,
                    Some("Compression is lossless for document content. Higher levels may take longer."),
                );
                ui.add_space(10.0);
                self.render_presets(ui, theme);
                ui.add_space(12.0);
                self.render_advanced_settings(ui, theme);
                ui.add_space(12.0);
                self.render_output_picker(ui, theme);
                ui.add_space(12.0);
                self.render_actions(ui, theme);
            });
    }

    fn render_presets(&mut self, ui: &mut Ui, theme: &AppTheme) {
        for preset in DocumentCompressionPreset::ALL {
            let selected = self.settings.preset == preset;
            let fill = if selected {
                theme.mix(theme.colors.surface_hover, theme.colors.accent, 0.18)
            } else {
                theme.colors.bg_raised
            };
            if ui
                .add(
                    Button::new(
                        RichText::new(preset.title())
                            .size(12.5)
                            .strong()
                            .color(theme.colors.fg),
                    )
                    .fill(fill)
                    .stroke(Stroke::new(
                        1.0,
                        if selected {
                            theme.colors.border_focus
                        } else {
                            theme.colors.border
                        },
                    ))
                    .corner_radius(CornerRadius::ZERO)
                    .min_size(vec2(ui.available_width(), 32.0)),
                )
                .on_hover_text(preset.description())
                .clicked()
            {
                self.settings.apply_preset(preset);
            }
            ui.add_space(6.0);
        }
    }

    fn render_advanced_settings(&mut self, ui: &mut Ui, theme: &AppTheme) {
        ui.checkbox(
            &mut self.settings.advanced_mode,
            RichText::new("Advanced controls").color(theme.colors.fg),
        );

        if self.settings.advanced_mode {
            ui.add_space(8.0);
            ui.add(
                Slider::new(&mut self.settings.compression_level, 0..=9)
                    .text("Compression level")
                    .step_by(1.0),
            )
            .on_hover_text("0 is fastest, 9 is smallest for PDF streams and ZIP packages.");
            ui.checkbox(
                &mut self.settings.pdf_object_streams,
                RichText::new("Modern PDF object streams").color(theme.colors.fg),
            )
            .on_hover_text(
                "Creates smaller PDFs but may be less friendly to very old PDF readers.",
            );
        }
    }

    fn render_output_picker(&mut self, ui: &mut Ui, theme: &AppTheme) {
        hint::title(
            ui,
            theme,
            "Output",
            13.0,
            Some("Choose where compressed documents are saved."),
        );
        ui.add_space(8.0);
        let fallback = runtime::default_document_output_root();
        let display_path = self
            .output_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| fallback.display().to_string());

        ui.label(
            RichText::new(format!("{} {}", icons::FOLDER, display_path))
                .size(12.0)
                .color(if self.output_dir.is_some() {
                    theme.colors.fg
                } else {
                    theme.colors.fg_muted
                }),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    Button::new(
                        RichText::new("Choose")
                            .size(12.0)
                            .strong()
                            .color(Color32::BLACK),
                    )
                    .fill(theme.colors.accent)
                    .stroke(Stroke::NONE)
                    .corner_radius(CornerRadius::ZERO),
                )
                .clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                self.output_dir = Some(dir);
                self.output_dir_user_set = true;
            }

            if self.output_dir_user_set
                && ui
                    .add(
                        Button::new(RichText::new("Auto").size(12.0).color(theme.colors.fg))
                            .fill(theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
            {
                self.output_dir = None;
                self.output_dir_user_set = false;
            }
        });
    }

    fn render_actions(&mut self, ui: &mut Ui, theme: &AppTheme) {
        let has_ready = self.has_compressible_documents();
        if ui
            .add_enabled(
                has_ready && self.active_batch.is_none(),
                Button::new(
                    RichText::new(format!("{} Compress All", icons::PLAY))
                        .size(13.0)
                        .strong()
                        .color(Color32::BLACK),
                )
                .fill(theme.colors.accent)
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::ZERO)
                .min_size(vec2(ui.available_width(), 34.0)),
            )
            .clicked()
        {
            self.start_all_compression();
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.active_batch.is_some(),
                    Button::new(RichText::new("Cancel").size(12.0).color(theme.colors.fg))
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                )
                .clicked()
            {
                self.cancel_compression();
            }

            let has_finished = self.queue.iter().any(|item| {
                matches!(
                    item.state,
                    DocumentCompressionState::Completed(_)
                        | DocumentCompressionState::Failed(_)
                        | DocumentCompressionState::Cancelled
                )
            });
            if ui
                .add_enabled(
                    has_finished && self.active_batch.is_none(),
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
    }
}
