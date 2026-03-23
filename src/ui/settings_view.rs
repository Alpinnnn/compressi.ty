use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, ScrollArea, Stroke, Ui, vec2,
};

use crate::{
    icons,
    modules::{
        ModuleKind,
        compress_videos::{
            engine::VideoEngineController,
            models::{EngineInfo, EngineStatus},
        },
    },
    runtime,
    settings::AppSettings,
    theme::AppTheme,
    ui::components::panel,
};

pub fn show(
    ui: &mut Ui,
    _ctx: &egui::Context,
    theme: &AppTheme,
    app_settings: &mut AppSettings,
    active_module: &mut Option<ModuleKind>,
    video_engine: &mut VideoEngineController,
) {
    let max_w = 860.0;
    let avail = ui.available_width();
    let side = ((avail - max_w) * 0.5).max(0.0);

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.add_space(24.0);

            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.allocate_ui_with_layout(
                    vec2(max_w.min(avail), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        panel::card(theme)
                            .inner_margin(egui::Margin::symmetric(20, 16))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
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
                                        ui.label(
                                            RichText::new("Settings")
                                                .size(22.0)
                                                .strong()
                                                .color(theme.colors.fg),
                                        );
                                        ui.label(
                                            RichText::new(
                                                "Configure global application preferences and bundled video tools.",
                                            )
                                            .size(12.0)
                                            .color(theme.colors.fg_dim),
                                        );
                                    });
                                });
                            });
                    },
                );
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.allocate_ui_with_layout(
                    vec2(max_w.min(avail), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        render_output_settings(ui, theme, app_settings);
                        ui.add_space(16.0);
                        render_engine_settings(ui, theme, video_engine);
                    },
                );
            });

            ui.add_space(24.0);
        });
}

fn render_output_settings(ui: &mut Ui, theme: &AppTheme, settings: &mut AppSettings) {
    panel::card(theme)
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.label(
                RichText::new("Output")
                    .size(16.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Configure where compressed files are saved by default.")
                    .size(12.0)
                    .color(theme.colors.fg_dim),
            );
            ui.add_space(16.0);

            ui.label(
                RichText::new("Default Output Folder")
                    .size(13.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Sets the default destination for all compression modules. You can still override this per session.")
                    .size(11.0)
                    .color(theme.colors.fg_dim),
            );
            ui.add_space(8.0);

            panel::inset(theme)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let display_path = match &settings.default_output_folder {
                            Some(dir) => dir.display().to_string(),
                            None => format!(
                                "Not set — uses {}",
                                runtime::default_output_root().display()
                            ),
                        };
                        ui.label(
                            RichText::new(format!("{} {}", icons::FOLDER, display_path))
                                .size(12.0)
                                .color(if settings.default_output_folder.is_some() {
                                    theme.colors.fg
                                } else {
                                    theme.colors.fg_muted
                                }),
                        );
                    });
                });
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui
                    .add(
                        Button::new(
                            RichText::new(format!("{} Choose Folder", icons::FOLDER))
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
                    settings.default_output_folder = Some(dir);
                }

                if settings.default_output_folder.is_some()
                    && ui
                        .add(
                            Button::new(
                                RichText::new("Reset to Default")
                                    .size(12.0)
                                    .color(theme.colors.fg),
                            )
                            .fill(theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                {
                    settings.default_output_folder = None;
                }
            });
        });
}

fn render_engine_settings(
    ui: &mut Ui,
    theme: &AppTheme,
    video_engine: &mut VideoEngineController,
) {
    panel::card(theme)
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            ui.label(
                RichText::new("Video Engine")
                    .size(16.0)
                    .strong()
                    .color(theme.colors.fg),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "The installer can ship a bundled FFmpeg build. Any update you install from here is stored in app data so the installation folder stays read-only.",
                )
                .size(12.0)
                .color(theme.colors.fg_dim),
            );
            ui.add_space(16.0);

            render_engine_activity(ui, theme, video_engine.status());

            if let Some(error) = video_engine.last_error() {
                ui.add_space(8.0);
                panel::tinted(theme, theme.colors.negative)
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(error)
                                .size(11.5)
                                .color(theme.colors.fg),
                        );
                    });
            }

            ui.add_space(12.0);
            render_engine_card(ui, theme, "Active Engine", video_engine.active_info(), true);
            ui.add_space(8.0);
            render_engine_card(ui, theme, "Bundled Engine", video_engine.bundled_info(), false);
            ui.add_space(8.0);
            render_engine_card(ui, theme, "Managed Update", video_engine.managed_info(), false);

            if let Some(system) = video_engine.system_info()
                && video_engine
                    .active_info()
                    .map(|active| active.source != system.source)
                    .unwrap_or(true)
            {
                ui.add_space(8.0);
                render_engine_card(ui, theme, "System PATH", Some(system), false);
            }

            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !video_engine.is_busy(),
                        Button::new(
                            RichText::new("Refresh Versions")
                                .size(12.0)
                                .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                {
                    video_engine.refresh();
                }

                if ui
                    .add_enabled(
                        !video_engine.is_busy(),
                        Button::new(
                            RichText::new("Update to Latest")
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
                    video_engine.update_to_latest();
                }

                if ui
                    .add_enabled(
                        !video_engine.is_busy() && video_engine.managed_info().is_some(),
                        Button::new(
                            RichText::new("Use Bundled Engine")
                                .size(12.0)
                                .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                {
                    video_engine.use_bundled_engine();
                }
            });

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        Button::new(
                            RichText::new("Open Managed Folder")
                                .size(12.0)
                                .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                    && let Some(dir) = video_engine.managed_engine_dir()
                {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that(dir);
                }

                if ui
                    .add(
                        Button::new(
                            RichText::new("Open Install Folder")
                                .size(12.0)
                                .color(theme.colors.fg),
                        )
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                    )
                    .clicked()
                    && let Some(dir) = video_engine.bundled_engine_dir()
                {
                    let _ = open::that(dir);
                }
            });
        });
}

fn render_engine_activity(ui: &mut Ui, theme: &AppTheme, status: &EngineStatus) {
    match status {
        EngineStatus::Checking => {
            panel::tinted(theme, theme.colors.accent)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Inspecting bundled and managed engines…")
                            .size(12.5)
                            .strong()
                            .color(theme.colors.fg),
                    );
                });
        }
        EngineStatus::Downloading { progress, stage } => {
            panel::tinted(theme, theme.colors.accent)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(stage)
                            .size(12.0)
                            .color(theme.colors.fg),
                    );
                    ui.add_space(6.0);
                    let bar_width = ui.available_width().max(180.0);
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(bar_width, 10.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(2), theme.colors.bg_base);
                    let fill = egui::Rect::from_min_size(
                        rect.min,
                        vec2(rect.width() * progress.clamp(0.0, 1.0), rect.height()),
                    );
                    ui.painter()
                        .rect_filled(fill, CornerRadius::same(2), theme.colors.accent);
                });
        }
        EngineStatus::Ready(_) | EngineStatus::Failed(_) => {}
    }
}

fn render_engine_card(
    ui: &mut Ui,
    theme: &AppTheme,
    title: &str,
    info: Option<&EngineInfo>,
    active: bool,
) {
    panel::inset(theme)
        .fill(if active {
            theme.mix(theme.colors.surface, theme.colors.accent, 0.08)
        } else {
            theme.colors.bg_raised
        })
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .size(13.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                if let Some(info) = info {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(info.source.label())
                            .size(10.5)
                            .color(theme.colors.accent),
                    );
                }
            });
            ui.add_space(4.0);

            if let Some(info) = info {
                ui.label(
                    RichText::new(&info.version)
                        .size(12.0)
                        .color(theme.colors.fg),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("FFmpeg: {}", info.ffmpeg_path.display()))
                        .size(10.5)
                        .color(theme.colors.fg_dim),
                );
                ui.label(
                    RichText::new(format!("FFprobe: {}", info.ffprobe_path.display()))
                        .size(10.5)
                        .color(theme.colors.fg_dim),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Encoders: {}{}{}",
                        if info.encoders.h264 { "H.264 " } else { "" },
                        if info.encoders.h265 { "H.265 " } else { "" },
                        if info.encoders.av1 { "AV1" } else { "" },
                    ))
                    .size(10.5)
                    .color(theme.colors.fg_dim),
                );
            } else {
                ui.label(
                    RichText::new("Not available on this machine yet.")
                        .size(11.5)
                        .color(theme.colors.fg_muted),
                );
            }
        });
}
