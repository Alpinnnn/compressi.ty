use std::path::PathBuf;

use eframe::egui::{self, Button, Color32, CornerRadius, RichText, Stroke, Ui};

use crate::{
    icons, runtime,
    settings::AppSettings,
    theme::AppTheme,
    ui::components::{hint, panel},
};

pub(super) fn render_output_settings(ui: &mut Ui, theme: &AppTheme, settings: &mut AppSettings) {
    panel::card(theme)
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            hint::title(
                ui,
                theme,
                "Output",
                16.0,
                Some("Choose the default save location for compressed files."),
            );
            ui.add_space(16.0);

            let general_auto = format!("Not set - uses {}", runtime::default_output_root().display());
            render_output_folder_field(
                ui,
                theme,
                "Default Output Folder",
                "Sets the default destination for all compression modules. You can still override this per session.",
                &mut settings.default_output_folder,
                &general_auto,
                "Reset to Auto",
            );

            ui.add_space(14.0);

            egui::CollapsingHeader::new("Photo, Audio, Video & Document Overrides")
                .id_salt("settings_output_overrides")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(8.0);

                    let photo_fallback = settings
                        .default_output_folder
                        .as_ref()
                        .map(|dir| {
                            format!("Not set - uses Default Output Folder ({})", dir.display())
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "Not set - uses {}",
                                runtime::default_photo_output_root().display()
                            )
                        });
                    render_output_folder_field(
                        ui,
                        theme,
                        "Photo Output Folder",
                        "Overrides the default output location for Compress Photos. Leave empty to follow Default Output Folder.",
                        &mut settings.photo_output_folder,
                        &photo_fallback,
                        "Use Default Output Folder",
                    );

                    ui.add_space(14.0);

                    let audio_fallback = settings
                        .default_output_folder
                        .as_ref()
                        .map(|dir| {
                            format!("Not set - uses Default Output Folder ({})", dir.display())
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "Not set - uses {}",
                                runtime::default_audio_output_root().display()
                            )
                        });
                    render_output_folder_field(
                        ui,
                        theme,
                        "Audio Output Folder",
                        "Overrides the default output location for Compress Audio. Leave empty to follow Default Output Folder.",
                        &mut settings.audio_output_folder,
                        &audio_fallback,
                        "Use Default Output Folder",
                    );

                    ui.add_space(14.0);

                    let video_fallback = settings
                        .default_output_folder
                        .as_ref()
                        .map(|dir| {
                            format!("Not set - uses Default Output Folder ({})", dir.display())
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "Not set - uses {}",
                                runtime::default_video_output_root().display()
                            )
                        });
                    render_output_folder_field(
                        ui,
                        theme,
                        "Video Output Folder",
                        "Overrides the default output location for Compress Videos. Files are saved directly into this folder.",
                        &mut settings.video_output_folder,
                        &video_fallback,
                        "Use Default Output Folder",
                    );

                    ui.add_space(14.0);

                    let document_fallback = settings
                        .default_output_folder
                        .as_ref()
                        .map(|dir| {
                            format!("Not set - uses Default Output Folder ({})", dir.display())
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "Not set - uses {}",
                                runtime::default_document_output_root().display()
                            )
                        });
                    render_output_folder_field(
                        ui,
                        theme,
                        "Document Output Folder",
                        "Overrides the default output location for Compress Documents. Leave empty to follow Default Output Folder.",
                        &mut settings.document_output_folder,
                        &document_fallback,
                        "Use Default Output Folder",
                    );
                });
        });
}

fn render_output_folder_field(
    ui: &mut Ui,
    theme: &AppTheme,
    title: &str,
    detail: &str,
    setting: &mut Option<PathBuf>,
    fallback_text: &str,
    reset_label: &str,
) {
    hint::title(ui, theme, title, 13.0, Some(detail));
    ui.add_space(8.0);

    panel::inset(theme)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let display_path = setting
                .as_ref()
                .map(|dir| dir.display().to_string())
                .unwrap_or_else(|| fallback_text.to_owned());
            ui.label(
                RichText::new(format!("{} {}", icons::FOLDER, display_path))
                    .size(12.0)
                    .color(if setting.is_some() {
                        theme.colors.fg
                    } else {
                        theme.colors.fg_muted
                    }),
            );
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
            *setting = Some(dir);
        }

        if setting.is_some()
            && ui
                .add(
                    Button::new(RichText::new(reset_label).size(12.0).color(theme.colors.fg))
                        .fill(theme.colors.bg_raised)
                        .stroke(Stroke::new(1.0, theme.colors.border))
                        .corner_radius(CornerRadius::ZERO),
                )
                .clicked()
        {
            *setting = None;
        }
    });
}
