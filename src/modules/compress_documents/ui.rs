mod chrome;
mod queue;
mod settings_panel;
mod workspace;

use eframe::egui::{self, Align, Layout, Rect, Ui, vec2};

use crate::{modules::ModuleKind, settings::AppSettings, theme::AppTheme};

pub(super) use super::{BannerTone, CompressDocumentsPage};

pub(super) fn flush(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
}

pub(super) fn compact(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
}

pub(super) fn truncate_filename(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        return name.to_owned();
    }

    if let Some(dot_pos) = name.rfind('.') {
        let extension = &name[dot_pos..];
        let stem_budget = max_chars.saturating_sub(extension.len()).saturating_sub(1);
        if stem_budget >= 4 {
            return format!("{}...{}", &name[..stem_budget], extension);
        }
    }

    format!("{}...", &name[..max_chars.saturating_sub(1)])
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let size = bytes as f64;

    if size >= GB {
        format!("{:.2} GB", size / GB)
    } else if size >= MB {
        format!("{:.2} MB", size / MB)
    } else if size >= KB {
        format!("{:.1} KB", size / KB)
    } else {
        format!("{bytes} B")
    }
}

impl CompressDocumentsPage {
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let paths = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if !paths.is_empty() {
            self.add_paths(paths);
        }
    }

    pub(super) fn select_documents(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Documents", super::processor::supported_extensions())
            .pick_files()
        {
            self.add_paths(paths);
        }
    }

    /// Renders the full document compression workspace.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        app_settings: &AppSettings,
    ) {
        if !self.output_dir_user_set {
            self.output_dir = app_settings.preferred_document_output_folder();
        }
        self.handle_dropped_files(ctx);
        flush(ui);

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
        let content_width = (avail_width - page_margin * 2.0).max(0.0);
        let content_rect = Rect::from_min_size(
            panel_rect.min + vec2(page_margin, 0.0),
            vec2(content_width, (panel_rect.height() - page_margin).max(0.0)),
        );

        let mut content_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        content_ui.add_space(page_margin);
        self.render_header(&mut content_ui, theme, active_module);
        content_ui.add_space(14.0);

        let workspace_height = content_ui.available_height().max(0.0);
        self.render_workspace(&mut content_ui, ctx, theme, workspace_height);
    }
}
