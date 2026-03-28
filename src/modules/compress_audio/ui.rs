mod helpers;
mod queue;
mod settings_panel;
mod toolbar;

use eframe::egui::{self, Align, Layout, Rect, ScrollArea, Ui, vec2};

use crate::{
    modules::{ModuleKind, compress_videos::engine::VideoEngineController},
    settings::AppSettings,
    theme::AppTheme,
};

use super::CompressAudioPage;

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

    /// Renders the full audio compression workspace.
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
}
