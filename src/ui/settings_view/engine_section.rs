use eframe::egui::{self, Button, Color32, CornerRadius, RichText, Stroke, Ui, vec2};

use crate::{
    modules::compress_videos::{
        engine::VideoEngineController,
        models::{EngineInfo, EngineStatus},
    },
    theme::AppTheme,
    ui::components::panel,
};

pub(super) fn render_engine_settings(
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
                        ui.label(RichText::new(error).size(11.5).color(theme.colors.fg));
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
                        RichText::new("Inspecting bundled and managed engines...")
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
                    ui.label(RichText::new(stage).size(12.0).color(theme.colors.fg));
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
                let mut gpu_backends = Vec::new();
                if info.encoders.h264_nvidia
                    || info.encoders.h265_nvidia
                    || info.encoders.av1_nvidia
                {
                    gpu_backends.push("NVIDIA");
                }
                if info.encoders.h264_amd || info.encoders.h265_amd || info.encoders.av1_amd {
                    gpu_backends.push("AMD");
                }
                if info.encoders.h264_intel_qsv
                    || info.encoders.h265_intel_qsv
                    || info.encoders.av1_intel_qsv
                {
                    gpu_backends.push("Intel Quick Sync");
                }
                if !gpu_backends.is_empty() {
                    ui.label(
                        RichText::new(format!("Auto GPU encode: {}", gpu_backends.join(", ")))
                            .size(10.5)
                            .color(theme.colors.accent),
                    );
                }
            } else {
                ui.label(
                    RichText::new("Not available on this machine yet.")
                        .size(11.5)
                        .color(theme.colors.fg_muted),
                );
            }
        });
}
