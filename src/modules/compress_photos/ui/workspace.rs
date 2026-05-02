use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, ScrollArea, Slider, Stroke, Ui,
    vec2,
};

use crate::{
    icons, runtime,
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::super::models::{CompressionPreset, CompressionState};
use super::controls::{format_selector, preset_row};
use super::{BannerMessage, BannerTone, CompressPhotosPage, compact};

impl CompressPhotosPage {
    pub(super) fn render_queue_column(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        if self.files.is_empty() {
            return;
        }

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), height),
            Layout::top_down(Align::Min),
            |ui| self.render_queue(ui, theme, height),
        );
    }

    pub(super) fn render_settings_column(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        let actions_height = if self.active_batch.is_some() {
            132.0
        } else {
            112.0
        };
        let settings_height = (height - actions_height - 12.0).max(0.0);

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), settings_height),
            Layout::top_down(Align::Min),
            |ui| self.render_settings(ui, theme, settings_height),
        );
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), actions_height),
            Layout::top_down(Align::Min),
            |ui| self.render_actions(ui, theme, actions_height),
        );
    }

    pub(super) fn render_stacked_workspace(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let has_files = !self.files.is_empty();
        let gap = 12.0;
        let drop_h = if has_files {
            height * 0.28
        } else {
            height * 0.42
        };
        let settings_h = height * 0.30;
        let queue_h = if has_files {
            (height - drop_h - settings_h - gap * 2.0).max(0.0)
        } else {
            0.0
        };

        self.render_drop_zone(ui, ctx, theme, drop_h.max(0.0));
        if has_files {
            ui.add_space(gap);
            self.render_queue_column(ui, theme, queue_h);
        }
        ui.add_space(gap);
        self.render_settings_column(ui, theme, settings_h.max(0.0));
    }

    pub(super) fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let hovering = ctx.input(|input| !input.raw.hovered_files.is_empty());
        let has_files = !self.files.is_empty();
        let accent = theme.colors.accent;
        let fill = if hovering {
            theme.mix(theme.colors.bg_raised, accent, 0.06)
        } else {
            theme.colors.surface
        };
        let stroke = Stroke::new(
            1.0,
            if hovering {
                theme.mix(theme.colors.border_focus, accent, 0.2)
            } else {
                theme.colors.border
            },
        );

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), height.max(0.0)),
            Layout::top_down(Align::Min),
            |ui| {
                panel::card(theme)
                    .fill(fill)
                    .stroke(stroke)
                    .inner_margin(egui::Margin::same(18))
                    .show(ui, |ui| {
                        compact(ui);
                        ui.set_min_height((height - 36.0).max(0.0));

                        let content_offset = if has_files { 86.0 } else { 110.0 };
                        ui.add_space(((ui.available_height() - content_offset) * 0.5).max(8.0));
                        ui.vertical_centered(|ui| {
                            let headline = if has_files {
                                let ready_count = self
                                    .files
                                    .iter()
                                    .filter(|file| matches!(file.state, CompressionState::Ready))
                                    .count();
                                format!("{ready_count} image(s) ready")
                            } else {
                                "Drop images or folders here".to_owned()
                            };
                            ui.label(
                                RichText::new(headline)
                                    .size(if has_files { 13.0 } else { 16.0 })
                                    .strong()
                                    .color(theme.colors.fg),
                            );
                            ui.add_space(10.0);

                            if has_files {
                                ui.horizontal(|ui| {
                                    ui.add_space((ui.available_width() - 250.0).max(0.0) / 2.0);
                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new(format!(
                                                    "{} Add More Images",
                                                    icons::IMAGES
                                                ))
                                                .size(12.0)
                                                .color(Color32::BLACK),
                                            )
                                            .fill(accent)
                                            .stroke(Stroke::NONE)
                                            .corner_radius(CornerRadius::ZERO),
                                        )
                                        .clicked()
                                    {
                                        self.select_images(ui.ctx());
                                    }

                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new(format!(
                                                    "{} Change Output",
                                                    icons::FOLDER
                                                ))
                                                .size(12.0)
                                                .color(theme.colors.fg),
                                            )
                                            .fill(theme.colors.bg_raised)
                                            .stroke(Stroke::new(1.0, theme.colors.border))
                                            .corner_radius(CornerRadius::ZERO),
                                        )
                                        .clicked()
                                    {
                                        self.select_output_folder(ui.ctx());
                                    }
                                });

                                let output_text = if let Some(dir) = &self.output_dir {
                                    (format!("Output: {}", dir.display()), theme.colors.fg_dim)
                                } else {
                                    (
                                        format!(
                                            "Output: Auto ({})",
                                            runtime::default_photo_output_root().display()
                                        ),
                                        theme.colors.fg_muted,
                                    )
                                };
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(output_text.0).size(10.0).color(output_text.1),
                                );
                            } else if ui
                                .add(
                                    Button::new(
                                        RichText::new(format!("{} Select Images", icons::IMAGES))
                                            .size(13.0)
                                            .strong()
                                            .color(Color32::BLACK),
                                    )
                                    .fill(accent)
                                    .stroke(Stroke::NONE)
                                    .corner_radius(CornerRadius::ZERO),
                                )
                                .clicked()
                            {
                                self.select_images(ui.ctx());
                            }
                        });
                    });
            },
        );
    }

    fn render_settings(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                hint::title(
                    ui,
                    theme,
                    "Settings",
                    14.0,
                    Some(
                        "Use presets for quick results. Enable Advanced mode for quality, resize, metadata, and format control.",
                    ),
                );
                ui.add_space(8.0);

                ScrollArea::vertical()
                    .id_salt("settings_scroll")
                    .auto_shrink([false, false])
                    .max_height((height - 34.0).max(0.0))
                    .show(ui, |ui| {
                        compact(ui);
                        ui.set_width(ui.available_width());

                        for preset in CompressionPreset::ALL {
                            let selected = self.settings.preset == preset;
                            if preset_row(ui, theme, preset, selected).clicked() {
                                self.settings.apply_preset(preset);
                            }
                        }

                        ui.checkbox(&mut self.settings.advanced_mode, "Advanced mode");

                        if self.settings.advanced_mode {
                            ui.add(
                                Slider::new(&mut self.settings.quality, 1..=100)
                                    .text("Quality")
                                    .show_value(true),
                            );
                            ui.add(
                                Slider::new(&mut self.settings.resize_percent, 25..=100)
                                    .text("Resize")
                                    .suffix("%")
                                    .show_value(true),
                            );
                            ui.checkbox(&mut self.settings.strip_metadata, "Strip metadata");
                            format_selector(ui, theme, &mut self.settings.format_choice);
                        }
                    });
            });
    }

    fn render_actions(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        let accent = theme.colors.accent;
        let can_go = !self.files.is_empty() && self.active_batch.is_none();

        panel::card(theme)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 28.0).max(0.0));

                if ui
                    .add_enabled(
                        can_go,
                        Button::new(
                            RichText::new(format!("{} Compress", icons::PLAY))
                                .size(13.0)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(accent)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::ZERO)
                        .min_size(vec2(ui.available_width(), 34.0)),
                    )
                    .clicked()
                {
                    self.start_compression();
                }

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.active_batch.is_some(),
                            Button::new(RichText::new("Cancel").size(12.0).color(theme.colors.fg))
                                .fill(theme.mix(theme.colors.surface, theme.colors.caution, 0.1))
                                .stroke(Stroke::new(
                                    1.0,
                                    theme.mix(theme.colors.border, theme.colors.caution, 0.24),
                                ))
                                .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        if let Some(batch) = &self.active_batch {
                            batch.cancel();
                            self.banner = Some(BannerMessage {
                                tone: BannerTone::Info,
                                text: "Cancel requested.".into(),
                            });
                        }
                    }

                    if ui
                        .add_enabled(
                            self.active_batch.is_none() && !self.files.is_empty(),
                            Button::new(RichText::new("Clear").size(12.0).color(theme.colors.fg))
                                .fill(theme.mix(theme.colors.surface, theme.colors.negative, 0.08))
                                .stroke(Stroke::new(
                                    1.0,
                                    theme.mix(theme.colors.border, theme.colors.negative, 0.2),
                                ))
                                .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        self.files.clear();
                        self.banner = None;
                    }
                });

                if self.active_batch.is_some() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(
                            RichText::new("Compressing...")
                                .size(11.0)
                                .color(theme.colors.fg_dim),
                        );
                    });
                }
            });
    }
}
