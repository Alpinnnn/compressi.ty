use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, ProgressBar, Rect, RichText, ScrollArea,
    Stroke, Ui, vec2,
};

use crate::{
    icons,
    modules::{
        ModuleKind,
        compress_audio::{
            logic::estimate_output,
            models::{
                AudioCompressionState, AudioEstimate, AudioFormat, AudioQueueItem,
                AudioWorkflowMode,
            },
        },
        compress_videos::engine::VideoEngineController,
    },
    settings::AppSettings,
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{BannerMessage, BannerTone, CompressAudioPage};

impl CompressAudioPage {
    fn pick_audio_files(&mut self, engine: &mut VideoEngineController) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                "Audio",
                &[
                    "aac", "aif", "aiff", "flac", "m4a", "m4b", "mka", "mp2", "mp3", "oga", "ogg",
                    "opus", "wav", "wma",
                ],
            )
            .pick_files()
        {
            self.add_paths(paths, engine);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context, engine: &mut VideoEngineController) {
        let paths = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if !paths.is_empty() {
            self.add_paths(paths, engine);
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        app_settings: &AppSettings,
        engine: &mut VideoEngineController,
    ) {
        if !self.output_dir_user_set {
            self.output_dir = app_settings.preferred_audio_output_folder();
        }

        self.handle_dropped_files(ctx, engine);
        self.flush_deferred_paths(engine);

        let panel_rect = ui.max_rect();
        let avail_width = panel_rect.width();
        let page_margin = if avail_width >= 1280.0 {
            28.0
        } else if avail_width >= 960.0 {
            22.0
        } else if avail_width >= 720.0 {
            16.0
        } else {
            12.0
        };
        let content_rect = Rect::from_min_size(
            panel_rect.min + vec2(page_margin, 0.0),
            vec2(
                (panel_rect.width() - page_margin * 2.0).max(0.0),
                (panel_rect.height() - page_margin).max(0.0),
            ),
        );

        let mut content_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        content_ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

        self.render_toolbar(&mut content_ui, theme, active_module, engine);
        content_ui.add_space(16.0);

        if let Some(message) = &self.banner {
            self.render_banner(&mut content_ui, theme, message);
            content_ui.add_space(14.0);
        }

        let workspace_width = content_ui.available_width();
        let workspace_height = content_ui.available_height().max(0.0);
        if workspace_width >= 980.0 {
            content_ui.allocate_ui_with_layout(
                vec2(workspace_width, workspace_height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let gutter = 16.0;
                    let left_width = (workspace_width * 0.56 - gutter * 0.5).max(300.0);

                    ui.allocate_ui_with_layout(
                        vec2(left_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.render_drop_zone(ui, theme, engine);
                            ui.add_space(12.0);
                            self.render_queue_column(ui, theme, workspace_height - 12.0);
                        },
                    );

                    ui.add_space(gutter);
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| self.render_settings_column(ui, theme, engine),
                    );
                },
            );
        } else {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut content_ui, |ui| {
                    self.render_drop_zone(ui, theme, engine);
                    ui.add_space(12.0);
                    self.render_queue_column(ui, theme, 320.0);
                    ui.add_space(12.0);
                    self.render_settings_column(ui, theme, engine);
                });
        }
    }

    fn render_toolbar(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        engine: &mut VideoEngineController,
    ) {
        panel::card(theme)
            .inner_margin(egui::Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            Button::new(
                                RichText::new(format!("{} Back", icons::BACK))
                                    .size(13.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        *active_module = None;
                    }

                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        hint::title(
                            ui,
                            theme,
                            "Compress Audio",
                            22.0,
                            Some(
                                "Drop files, keep Auto Mode on, and let the workspace pick a modern codec for you.",
                            ),
                        );
                        ui.label(
                            RichText::new(
                                "Fast workflow for music, podcasts, voice notes, and batch exports.",
                            )
                            .size(12.0)
                            .color(theme.colors.fg_dim),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(last_output_dir) = &self.last_output_dir {
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new("Open Output")
                                            .size(12.0)
                                            .color(theme.colors.fg),
                                    )
                                    .fill(theme.colors.bg_raised)
                                    .stroke(Stroke::new(1.0, theme.colors.border))
                                    .corner_radius(CornerRadius::ZERO),
                                )
                                .clicked()
                            {
                                let _ = open::that(last_output_dir);
                            }
                        }

                        if ui
                            .add(
                                Button::new(
                                    RichText::new(format!("{} Add Audio", icons::PLAY))
                                        .size(12.0)
                                        .strong()
                                        .color(Color32::BLACK),
                                )
                                .fill(theme.colors.accent)
                                .stroke(Stroke::NONE)
                                .corner_radius(CornerRadius::ZERO),
                            )
                            .clicked()
                        {
                            self.pick_audio_files(engine);
                        }
                    });
                });
            });
    }

    fn render_banner(&self, ui: &mut Ui, theme: &AppTheme, message: &BannerMessage) {
        let tint = match message.tone {
            BannerTone::Info => theme.colors.accent,
            BannerTone::Success => theme.colors.positive,
            BannerTone::Error => theme.colors.negative,
        };

        panel::tinted(theme, tint).show(ui, |ui| {
            ui.label(
                RichText::new(&message.text)
                    .size(12.5)
                    .color(theme.colors.fg),
            );
        });
    }

    fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        panel::card(theme).show(ui, |ui| {
            hint::title(
                ui,
                theme,
                "Drop Audio Files",
                16.0,
                Some(
                    "Drag files here or use the Add Audio button. Files are analyzed automatically as soon as FFmpeg is ready.",
                ),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("Drag audio here -> choose Auto or Manual -> Start Compression")
                    .size(13.0)
                    .color(theme.colors.fg),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Supported: MP3, M4A, AAC, OPUS, OGG, FLAC, WAV, AIFF, WMA and more.",
                )
                .size(12.0)
                .color(theme.colors.fg_dim),
            );
            ui.add_space(12.0);

            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        Button::new(
                            RichText::new("Choose Audio Files")
                                .size(12.0)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(theme.colors.accent)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                {
                    self.pick_audio_files(engine);
                }

                if ui
                    .add(
                        Button::new(
                            RichText::new(format!(
                                "{} Output: {}",
                                icons::FOLDER,
                                self.output_dir
                                    .as_ref()
                                    .map(|path| path.display().to_string())
                                    .unwrap_or_else(|| "Auto".to_owned())
                            ))
                            .size(12.0)
                            .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    self.output_dir = Some(folder);
                    self.output_dir_user_set = true;
                }

                if self.output_dir_user_set
                    && ui
                        .add(
                            Button::new(
                                RichText::new("Use Auto Output")
                                    .size(12.0)
                                    .color(theme.colors.fg),
                            )
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
        });
    }

    fn render_queue_column(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
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

    fn render_settings_column(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        let selected_metadata = self
            .selected_id
            .and_then(|id| self.find_item(id))
            .and_then(|item| item.metadata.clone());
        let selected_analysis = self
            .selected_id
            .and_then(|id| self.find_item(id))
            .and_then(|item| item.analysis.clone());
        let estimate = selected_metadata.as_ref().and_then(|metadata| {
            engine
                .active_info()
                .map(|engine_info| estimate_output(metadata, &self.settings, &engine_info.encoders))
        });

        panel::card(theme).show(ui, |ui| {
            hint::title(
                ui,
                theme,
                "Settings Panel",
                16.0,
                Some(
                    "Keep Auto Mode on for the simplest workflow, or switch to Manual when you need a specific output format.",
                ),
            );
            ui.add_space(12.0);

            if let Some(analysis) = selected_analysis.as_ref() {
                panel::tinted(theme, theme.colors.accent).show(ui, |ui| {
                    ui.label(
                        RichText::new(&analysis.headline)
                            .size(13.0)
                            .strong()
                            .color(theme.colors.fg),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&analysis.detail)
                            .size(12.0)
                            .color(theme.colors.fg_dim),
                    );
                });
                ui.add_space(12.0);
            }

            self.render_mode_toggle(ui, theme);
            ui.add_space(12.0);

            match self.settings.mode {
                AudioWorkflowMode::Auto => self.render_auto_controls(ui, theme),
                AudioWorkflowMode::Manual => {
                    self.render_manual_controls(ui, theme, engine.active_info())
                }
            }

            ui.add_space(12.0);
            self.render_extra_options(ui, theme);
            ui.add_space(12.0);
            self.render_output_preview(ui, theme, estimate.as_ref(), selected_metadata.as_ref());
            ui.add_space(12.0);
            self.render_actions(ui, theme, engine);
        });
    }

    fn render_mode_toggle(&mut self, ui: &mut Ui, theme: &AppTheme) {
        ui.horizontal(|ui| {
            if toggle_button(
                ui,
                theme,
                self.settings.mode == AudioWorkflowMode::Auto,
                "Auto (recommended)",
            )
            .clicked()
            {
                self.settings.mode = AudioWorkflowMode::Auto;
            }

            if toggle_button(
                ui,
                theme,
                self.settings.mode == AudioWorkflowMode::Manual,
                "Manual",
            )
            .clicked()
            {
                self.settings.mode = AudioWorkflowMode::Manual;
            }
        });
    }

    fn render_auto_controls(&mut self, ui: &mut Ui, theme: &AppTheme) {
        hint::title(
            ui,
            theme,
            "Smart Mode",
            13.0,
            Some(
                "Auto chooses between AAC and OPUS based on the file type, then falls back to MP3 only when needed.",
            ),
        );
        ui.add_space(8.0);

        for preset in [
            crate::modules::compress_audio::models::AudioAutoPreset::HighQuality,
            crate::modules::compress_audio::models::AudioAutoPreset::Balanced,
            crate::modules::compress_audio::models::AudioAutoPreset::SmallSize,
        ] {
            let selected = self.settings.auto_preset == preset;
            if toggle_button(ui, theme, selected, preset.label()).clicked() {
                self.settings.auto_preset = preset;
            }
            ui.label(
                RichText::new(preset.detail())
                    .size(11.5)
                    .color(theme.colors.fg_dim),
            );
            ui.add_space(6.0);
        }
    }

    fn render_manual_controls(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine_info: Option<&crate::modules::compress_videos::models::EngineInfo>,
    ) {
        hint::title(
            ui,
            theme,
            "Manual Mode",
            13.0,
            Some(
                "Pick the output format yourself, then fine tune bitrate or advanced conversion options.",
            ),
        );
        ui.add_space(8.0);

        egui::ComboBox::from_id_salt("audio_manual_format")
            .selected_text(self.settings.manual_format.label())
            .show_ui(ui, |ui| {
                for format in [
                    AudioFormat::Aac,
                    AudioFormat::Opus,
                    AudioFormat::Mp3,
                    AudioFormat::Flac,
                ] {
                    let available = engine_info
                        .map(|engine| format_available(format, &engine.encoders))
                        .unwrap_or(true);
                    if available {
                        ui.selectable_value(
                            &mut self.settings.manual_format,
                            format,
                            format.label(),
                        );
                    }
                }
            });

        ui.add_space(8.0);
        let bitrate_enabled = !self.settings.manual_format.is_lossless();
        ui.add_enabled_ui(bitrate_enabled, |ui| {
            ui.label(
                RichText::new(format!(
                    "Bitrate: {} kbps",
                    self.settings.manual_bitrate_kbps
                ))
                .size(12.0)
                .color(theme.colors.fg),
            );
            ui.add(
                egui::Slider::new(&mut self.settings.manual_bitrate_kbps, 24..=320).suffix(" kbps"),
            );
        });
        if !bitrate_enabled {
            ui.label(
                RichText::new("FLAC keeps audio lossless, so bitrate is handled automatically.")
                    .size(11.5)
                    .color(theme.colors.fg_dim),
            );
        }

        ui.add_space(8.0);
        ui.checkbox(&mut self.settings.advanced_open, "Show advanced settings");
        if self.settings.advanced_open {
            ui.add_space(8.0);
            panel::inset(theme).show(ui, |ui| {
                ui.label(
                    RichText::new("Sample Rate")
                        .size(12.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                selectable_option_row(
                    ui,
                    theme,
                    &mut self.settings.manual_sample_rate_hz,
                    &[
                        (None, "Original"),
                        (Some(22_050), "22.05 kHz"),
                        (Some(32_000), "32 kHz"),
                        (Some(44_100), "44.1 kHz"),
                        (Some(48_000), "48 kHz"),
                    ],
                );

                ui.add_space(10.0);
                ui.label(
                    RichText::new("Channels")
                        .size(12.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                selectable_option_row(
                    ui,
                    theme,
                    &mut self.settings.manual_channels,
                    &[(None, "Original"), (Some(1), "Mono"), (Some(2), "Stereo")],
                );
            });
        }
    }

    fn render_extra_options(&mut self, ui: &mut Ui, theme: &AppTheme) {
        hint::title(
            ui,
            theme,
            "Extra Options",
            13.0,
            Some(
                "These stay tucked away from beginners, but they are ready when you need cleanup or format conversion workflows.",
            ),
        );
        ui.add_space(8.0);
        panel::inset(theme).show(ui, |ui| {
            ui.checkbox(&mut self.settings.normalize_volume, "Normalize volume");
            ui.checkbox(&mut self.settings.remove_metadata, "Remove metadata");
            ui.checkbox(
                &mut self.settings.convert_format_only,
                "Convert format only (no compression focus)",
            );
        });
    }

    fn render_output_preview(
        &self,
        ui: &mut Ui,
        theme: &AppTheme,
        estimate: Option<&AudioEstimate>,
        selected_metadata: Option<&crate::modules::compress_audio::models::AudioMetadata>,
    ) {
        hint::title(
            ui,
            theme,
            "Output Preview",
            13.0,
            Some(
                "Estimated size uses the current settings, so it updates before you start the batch.",
            ),
        );
        ui.add_space(8.0);

        panel::inset(theme).show(ui, |ui| match (selected_metadata, estimate) {
            (Some(metadata), Some(estimate)) => {
                ui.label(
                    RichText::new(format!(
                        "Original size: {}",
                        format_bytes(metadata.size_bytes)
                    ))
                    .size(12.0)
                    .color(theme.colors.fg),
                );
                ui.label(
                    RichText::new(format!(
                        "Estimated size: {}",
                        format_bytes(estimate.estimated_size_bytes)
                    ))
                    .size(12.0)
                    .color(theme.colors.fg),
                );

                let output_label = estimate
                    .target_bitrate_kbps
                    .map(|bitrate| format!("{} | {} kbps", estimate.output_format.label(), bitrate))
                    .unwrap_or_else(|| estimate.output_format.label().to_owned());
                ui.label(
                    RichText::new(format!("Output: {output_label}"))
                        .size(12.0)
                        .color(theme.colors.fg),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Estimated reduction: {:.0}%",
                        estimate.savings_percent.max(-100.0)
                    ))
                    .size(12.0)
                    .strong()
                    .color(theme.colors.fg),
                );
                if let Some(recommendation) = &estimate.recommendation {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(recommendation)
                            .size(11.5)
                            .color(theme.colors.fg_dim),
                    );
                }
                for warning in &estimate.warnings {
                    ui.add_space(6.0);
                    ui.label(RichText::new(warning).size(11.5).color(theme.colors.fg_dim));
                }
                if let Some(skip_reason) = &estimate.skip_reason {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(skip_reason)
                            .size(11.5)
                            .color(theme.colors.negative),
                    );
                }
            }
            _ => {
                ui.label(
                    RichText::new(
                        "Select an analyzed audio file to see size estimates and warnings.",
                    )
                    .size(12.0)
                    .color(theme.colors.fg_dim),
                );
            }
        });
    }

    fn render_actions(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        engine: &mut VideoEngineController,
    ) {
        let ready_count = self
            .queue
            .iter()
            .filter(|item| item.metadata.is_some())
            .count();

        ui.label(
            RichText::new(format!("Ready in queue: {} file(s)", ready_count))
                .size(11.5)
                .color(theme.colors.fg_dim),
        );
        ui.add_space(8.0);

        if self.is_compressing() {
            ui.label(
                RichText::new(format!(
                    "Overall progress: {:.0}%",
                    overall_progress(&self.queue) * 100.0
                ))
                .size(12.0)
                .color(theme.colors.fg),
            );
            ui.add(
                ProgressBar::new(overall_progress(&self.queue))
                    .desired_width(ui.available_width())
                    .show_percentage(),
            );
            ui.add_space(10.0);
        }

        ui.horizontal(|ui| {
            let start_button = Button::new(
                RichText::new("Start Compression")
                    .size(12.0)
                    .strong()
                    .color(Color32::BLACK),
            )
            .fill(theme.colors.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::ZERO);
            if ui
                .add_enabled(!self.is_compressing() && ready_count > 0, start_button)
                .clicked()
            {
                self.start_compression(engine);
            }

            let cancel_button =
                Button::new(RichText::new("Cancel").size(12.0).color(theme.colors.fg))
                    .fill(theme.colors.bg_raised)
                    .stroke(Stroke::new(1.0, theme.colors.border))
                    .corner_radius(CornerRadius::ZERO);
            if ui
                .add_enabled(self.is_compressing(), cancel_button)
                .clicked()
            {
                self.cancel_compression();
            }
        });
    }
}

fn toggle_button(ui: &mut Ui, theme: &AppTheme, selected: bool, label: &str) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).size(12.0).color(theme.colors.fg))
            .fill(if selected {
                theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.16)
            } else {
                theme.colors.bg_raised
            })
            .stroke(Stroke::new(
                1.0,
                if selected {
                    theme.mix(theme.colors.border, theme.colors.accent, 0.32)
                } else {
                    theme.colors.border
                },
            ))
            .corner_radius(CornerRadius::ZERO),
    )
}

fn selectable_option_row<T: Copy + PartialEq>(
    ui: &mut Ui,
    theme: &AppTheme,
    value: &mut Option<T>,
    options: &[(Option<T>, &str)],
) {
    ui.horizontal_wrapped(|ui| {
        for (candidate, label) in options {
            if toggle_button(ui, theme, *value == *candidate, label).clicked() {
                *value = *candidate;
            }
        }
    });
}

fn format_available(
    format: AudioFormat,
    encoders: &crate::modules::compress_videos::models::EncoderAvailability,
) -> bool {
    match format {
        AudioFormat::Aac => encoders.supports_aac(),
        AudioFormat::Opus => encoders.supports_opus(),
        AudioFormat::Mp3 => encoders.supports_mp3(),
        AudioFormat::Flac => encoders.supports_flac(),
    }
}

fn queue_subtitle(item: &AudioQueueItem) -> String {
    match &item.state {
        AudioCompressionState::Analyzing => "Analyzing audio...".to_owned(),
        AudioCompressionState::Ready => item
            .analysis
            .as_ref()
            .map(|analysis| analysis.headline.clone())
            .unwrap_or_else(|| "Ready".to_owned()),
        AudioCompressionState::Compressing(progress) => {
            format!("{} | {:.0}%", progress.stage, progress.progress * 100.0)
        }
        AudioCompressionState::Completed(result) => {
            format!("Done | saved {:.0}%", result.reduction_percent.max(0.0))
        }
        AudioCompressionState::Skipped(reason) => format!("Skipped | {}", reason),
        AudioCompressionState::Failed(error) => format!("Failed | {}", error),
        AudioCompressionState::Cancelled => "Cancelled".to_owned(),
    }
}

fn overall_progress(queue: &[AudioQueueItem]) -> f32 {
    let mut total = 0.0;
    let mut count = 0.0;

    for item in queue {
        let progress = match &item.state {
            AudioCompressionState::Compressing(progress) => progress.progress,
            AudioCompressionState::Completed(_)
            | AudioCompressionState::Skipped(_)
            | AudioCompressionState::Cancelled => 1.0,
            _ => 0.0,
        };
        total += progress;
        count += 1.0;
    }

    if count <= 0.0 {
        0.0
    } else {
        (total / count).clamp(0.0, 1.0)
    }
}

fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < units.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{bytes} {}", units[unit_index])
    } else {
        format!("{value:.1} {}", units[unit_index])
    }
}
