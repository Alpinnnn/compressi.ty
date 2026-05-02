use std::path::PathBuf;

use eframe::egui::{self, Button, Color32, CornerRadius, RichText, Stroke, Ui};

use crate::{
    file_dialog::{self, DialogReceiver},
    icons, runtime,
    settings::AppSettings,
    theme::AppTheme,
    ui::components::{hint, panel},
};

#[derive(Default)]
pub(super) struct OutputSettingsState {
    folder_picker_rx: Option<DialogReceiver<PathBuf>>,
    pending_target: Option<OutputFolderTarget>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputFolderTarget {
    Default,
    Photo,
    Audio,
    Video,
    Document,
}

struct OutputFolderField<'a> {
    title: &'static str,
    detail: &'static str,
    setting: &'a mut Option<PathBuf>,
    fallback_text: &'a str,
    reset_label: &'static str,
    target: OutputFolderTarget,
    dialog_title: &'static str,
}

impl OutputSettingsState {
    fn poll_folder_picker(&mut self, settings: &mut AppSettings) {
        let Some(result) = file_dialog::poll_dialog(&mut self.folder_picker_rx) else {
            return;
        };
        let target = self.pending_target.take();

        if let (Some(target), Some(directory)) = (target, result) {
            output_folder_setting_mut(settings, target).replace(directory);
        }
    }

    fn choose_folder(&mut self, ui: &Ui, title: &'static str, target: OutputFolderTarget) {
        if self.folder_picker_rx.is_some() {
            return;
        }

        self.folder_picker_rx = file_dialog::pick_folder(ui.ctx(), title);
        if self.folder_picker_rx.is_some() {
            self.pending_target = Some(target);
        }
    }
}

pub(super) fn render_output_settings(
    ui: &mut Ui,
    theme: &AppTheme,
    settings: &mut AppSettings,
    state: &mut OutputSettingsState,
) {
    state.poll_folder_picker(settings);

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
                OutputFolderField {
                    title: "Default Output Folder",
                    detail: "Sets the default destination for all compression modules. You can still override this per session.",
                    setting: &mut settings.default_output_folder,
                    fallback_text: &general_auto,
                    reset_label: "Reset to Auto",
                    target: OutputFolderTarget::Default,
                    dialog_title: "Choose default output folder",
                },
                state,
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
                        OutputFolderField {
                            title: "Photo Output Folder",
                            detail: "Overrides the default output location for Compress Photos. Leave empty to follow Default Output Folder.",
                            setting: &mut settings.photo_output_folder,
                            fallback_text: &photo_fallback,
                            reset_label: "Use Default Output Folder",
                            target: OutputFolderTarget::Photo,
                            dialog_title: "Choose photo output folder",
                        },
                        state,
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
                        OutputFolderField {
                            title: "Audio Output Folder",
                            detail: "Overrides the default output location for Compress Audio. Leave empty to follow Default Output Folder.",
                            setting: &mut settings.audio_output_folder,
                            fallback_text: &audio_fallback,
                            reset_label: "Use Default Output Folder",
                            target: OutputFolderTarget::Audio,
                            dialog_title: "Choose audio output folder",
                        },
                        state,
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
                        OutputFolderField {
                            title: "Video Output Folder",
                            detail: "Overrides the default output location for Compress Videos. Files are saved directly into this folder.",
                            setting: &mut settings.video_output_folder,
                            fallback_text: &video_fallback,
                            reset_label: "Use Default Output Folder",
                            target: OutputFolderTarget::Video,
                            dialog_title: "Choose video output folder",
                        },
                        state,
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
                        OutputFolderField {
                            title: "Document Output Folder",
                            detail: "Overrides the default output location for Compress Documents. Leave empty to follow Default Output Folder.",
                            setting: &mut settings.document_output_folder,
                            fallback_text: &document_fallback,
                            reset_label: "Use Default Output Folder",
                            target: OutputFolderTarget::Document,
                            dialog_title: "Choose document output folder",
                        },
                        state,
                    );
                });
        });
}

fn render_output_folder_field(
    ui: &mut Ui,
    theme: &AppTheme,
    field: OutputFolderField<'_>,
    state: &mut OutputSettingsState,
) {
    let OutputFolderField {
        title,
        detail,
        setting,
        fallback_text,
        reset_label,
        target,
        dialog_title,
    } = field;

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
        {
            state.choose_folder(ui, dialog_title, target);
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

fn output_folder_setting_mut(
    settings: &mut AppSettings,
    target: OutputFolderTarget,
) -> &mut Option<PathBuf> {
    match target {
        OutputFolderTarget::Default => &mut settings.default_output_folder,
        OutputFolderTarget::Photo => &mut settings.photo_output_folder,
        OutputFolderTarget::Audio => &mut settings.audio_output_folder,
        OutputFolderTarget::Video => &mut settings.video_output_folder,
        OutputFolderTarget::Document => &mut settings.document_output_folder,
    }
}
