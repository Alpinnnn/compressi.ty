use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, Ui, vec2,
};

use crate::{
    icons,
    theme::AppTheme,
    ui::components::{hint, panel},
};

use super::{CompressDocumentsPage, compact, flush};

impl CompressDocumentsPage {
    pub(super) fn render_workspace(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let workspace_width = ui.available_width();
        if workspace_width >= 1080.0 {
            ui.allocate_ui_with_layout(
                vec2(workspace_width, height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gap = 14.0;
                    let left_width = (workspace_width * 0.28).max(250.0);
                    let queue_width = (workspace_width * 0.43).max(340.0);
                    let settings_width =
                        (workspace_width - left_width - queue_width - gap * 2.0).max(260.0);

                    ui.allocate_ui_with_layout(
                        vec2(left_width, height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_drop_zone(ui, ctx, theme, (height * 0.50).max(220.0));
                            ui.add_space(gap);
                            self.render_support_summary(ui, theme);
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        vec2(queue_width, height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_queue(ui, theme, height);
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        vec2(settings_width, height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_settings_panel(ui, theme, height);
                        },
                    );
                },
            );
        } else {
            self.render_stacked_workspace(ui, ctx, theme, height);
        }
    }

    fn render_stacked_workspace(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let gap = 12.0;
        let drop_height = if self.queue.is_empty() {
            (height * 0.34).max(210.0)
        } else {
            (height * 0.24).max(170.0)
        };
        self.render_drop_zone(ui, ctx, theme, drop_height);
        ui.add_space(gap);
        self.render_queue(ui, theme, (height * 0.38).max(210.0));
        ui.add_space(gap);
        self.render_settings_panel(ui, theme, (height - drop_height - gap * 2.0).max(260.0));
    }

    pub(super) fn render_drop_zone(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        height: f32,
    ) {
        let hovering = ctx.input(|input| !input.raw.hovered_files.is_empty());
        let frame = if hovering {
            panel::tinted(theme, theme.colors.accent)
        } else {
            panel::card(theme)
        };

        frame.show(ui, |ui| {
            compact(ui);
            ui.set_min_height((height - 40.0).max(0.0));
            ui.vertical_centered(|ui| {
                ui.add_space((height * 0.15).max(8.0));
                ui.label(icons::rich(icons::DOCUMENT, 42.0, theme.colors.accent));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Drop documents")
                        .size(20.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                ui.label(
                    RichText::new("PDF, Word, PowerPoint, Excel, ODF, EPUB, XPS")
                        .size(12.0)
                        .color(theme.colors.fg_dim),
                );
                ui.add_space(14.0);
                if ui
                    .add(
                        Button::new(
                            RichText::new("Select Files")
                                .size(13.0)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(theme.colors.accent)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::ZERO)
                        .min_size(vec2(148.0, 34.0)),
                    )
                    .clicked()
                {
                    self.select_documents();
                }
            });
        });
    }

    fn render_support_summary(&mut self, ui: &mut Ui, theme: &AppTheme) {
        panel::card(theme)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                compact(ui);
                hint::title(
                    ui,
                    theme,
                    "Engines",
                    15.0,
                    Some("PDF uses lopdf stream/object optimization. Office, ODF, EPUB, XPS, and Visio use ZIP deflate repacking with required mimetype entries preserved."),
                );
                ui.add_space(8.0);
                for label in [
                    "PDF: stream and object compression",
                    "DOCX/XLSX/PPTX: OOXML ZIP repack",
                    "ODT/ODS/ODP: ODF-safe ZIP repack",
                    "EPUB: OCF-safe ZIP repack",
                    "XPS/Visio: package repack",
                ] {
                    ui.label(RichText::new(label).size(12.0).color(theme.colors.fg_dim));
                }
            });
    }
}
