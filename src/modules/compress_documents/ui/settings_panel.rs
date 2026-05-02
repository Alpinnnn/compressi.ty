use eframe::egui::{self, Button, CornerRadius, RichText, Slider, Stroke, Ui, vec2};

use crate::{
    icons,
    modules::compress_documents::models::{
        DocumentCompressionPreset, DocumentKind, PackageDocumentCompressionSettings,
        PdfDocumentCompressionSettings,
    },
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{CompressDocumentsPage, compact, truncate_filename};

impl CompressDocumentsPage {
    pub(super) fn render_settings_panel(&mut self, ui: &mut Ui, theme: &AppTheme, height: f32) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                compact(ui);
                ui.set_min_height((height - 32.0).max(0.0));
                let active_kind = self.active_settings_kind();
                hint::title(
                    ui,
                    theme,
                    "Settings",
                    15.0,
                    active_kind.map(DocumentKind::settings_label),
                );
                ui.add_space(10.0);
                if let Some(active_kind) = active_kind {
                    if let Some(file_name) = self.active_settings_file_name() {
                        ui.label(
                            RichText::new(truncate_filename(file_name, 34))
                                .size(12.0)
                                .color(theme.colors.fg_dim),
                        );
                        ui.add_space(8.0);
                    }
                    self.render_presets(ui, theme, active_kind);
                    ui.add_space(12.0);
                    self.render_advanced_settings(ui, theme, active_kind);
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(18.0);
                        ui.label(icons::rich(icons::DOCUMENT, 28.0, theme.colors.fg_dim));
                        ui.label(
                            RichText::new("No document selected")
                                .size(12.5)
                                .color(theme.colors.fg_dim),
                        );
                    });
                }
            });
    }

    fn active_settings_kind(&self) -> Option<DocumentKind> {
        self.selected_id
            .and_then(|id| {
                self.queue
                    .iter()
                    .find(|item| item.asset.id == id)
                    .map(|item| item.asset.kind)
            })
            .or_else(|| self.queue.first().map(|item| item.asset.kind))
    }

    fn active_settings_file_name(&self) -> Option<&str> {
        self.selected_id
            .and_then(|id| {
                self.queue
                    .iter()
                    .find(|item| item.asset.id == id)
                    .map(|item| item.asset.file_name.as_str())
            })
            .or_else(|| self.queue.first().map(|item| item.asset.file_name.as_str()))
    }

    fn render_presets(&mut self, ui: &mut Ui, theme: &AppTheme, active_kind: DocumentKind) {
        for preset in DocumentCompressionPreset::ALL {
            let selected = self.settings.preset(active_kind) == preset;
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
                self.settings.apply_preset(active_kind, preset);
            }
            ui.add_space(6.0);
        }
    }

    fn render_advanced_settings(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        active_kind: DocumentKind,
    ) {
        match active_kind {
            DocumentKind::Pdf => render_pdf_advanced_settings(ui, theme, &mut self.settings.pdf),
            kind => {
                if let Some(settings) = self.settings.package_settings_mut(kind) {
                    render_package_advanced_settings(ui, theme, kind, settings);
                }
            }
        }
    }
}

fn render_pdf_advanced_settings(
    ui: &mut Ui,
    theme: &AppTheme,
    settings: &mut PdfDocumentCompressionSettings,
) {
    ui.checkbox(
        &mut settings.advanced_mode,
        RichText::new("Advanced controls").color(theme.colors.fg),
    );

    if settings.advanced_mode {
        ui.add_space(8.0);
        ui.add(
            Slider::new(&mut settings.compression_level, 0..=9)
                .text("PDF compression level")
                .step_by(1.0),
        )
        .on_hover_text("0 is fastest, 9 is smallest for PDF streams.");
        ui.checkbox(
            &mut settings.pdf_object_streams,
            RichText::new("Modern PDF object streams").color(theme.colors.fg),
        )
        .on_hover_text("Creates smaller PDFs but may be less friendly to very old PDF readers.");
        ui.add(
            Slider::new(&mut settings.pdf_image_quality, 35..=100)
                .text("PDF image quality")
                .step_by(1.0),
        )
        .on_hover_text("Used by the Ghostscript PDF engine.");
        ui.add(
            Slider::new(&mut settings.pdf_image_resolution_dpi, 72..=300)
                .text("PDF image DPI")
                .step_by(1.0),
        )
        .on_hover_text("Lower DPI enables stronger PDF image downsampling.");
    }
}

fn render_package_advanced_settings(
    ui: &mut Ui,
    theme: &AppTheme,
    kind: DocumentKind,
    settings: &mut PackageDocumentCompressionSettings,
) {
    ui.checkbox(
        &mut settings.advanced_mode,
        RichText::new("Advanced controls").color(theme.colors.fg),
    );

    if settings.advanced_mode {
        ui.add_space(8.0);
        ui.add(
            Slider::new(&mut settings.compression_level, 0..=9)
                .text(format!("{} ZIP level", kind.label()))
                .step_by(1.0),
        )
        .on_hover_text("0 is fastest, 9 is smallest for ZIP-packaged documents.");
        ui.add(
            Slider::new(&mut settings.package_image_quality, 35..=100)
                .text(format!("{} image quality", kind.label()))
                .step_by(1.0),
        )
        .on_hover_text("Re-encodes JPEG media inside this document family.");
        ui.add(
            Slider::new(&mut settings.package_image_resize_percent, 40..=100)
                .text(format!("{} image scale", kind.label()))
                .step_by(1.0),
        )
        .on_hover_text("Downscales large embedded PNG/JPEG media before package repacking.");
    }
}
