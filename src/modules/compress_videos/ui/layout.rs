use eframe::egui::{self, Align, Layout, Rect, Ui, vec2};

use crate::{
    modules::{
        ModuleKind,
        compress_videos::{engine::VideoEngineController, models::EngineStatus},
    },
    settings::AppSettings,
    theme::AppTheme,
};

use super::{CompressVideosPage, flush, widgets::render_banner};

impl CompressVideosPage {
    /// Renders the full video compression workspace.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        theme: &AppTheme,
        active_module: &mut Option<ModuleKind>,
        app_settings: &AppSettings,
        engine: &mut VideoEngineController,
    ) {
        engine.ensure_ready();
        if !self.output_dir_user_set {
            self.output_dir = app_settings.preferred_video_output_folder();
        }
        self.handle_dropped_files(ctx, engine);
        flush(ui);

        let panel_rect = ui.max_rect();
        let available_width = panel_rect.width();
        let page_margin = if available_width >= 1280.0 {
            28.0
        } else if available_width >= 960.0 {
            22.0
        } else if available_width >= 720.0 {
            16.0
        } else {
            12.0
        };
        let content_width = (available_width - page_margin * 2.0).max(0.0);
        let bottom_padding = page_margin;

        let content_rect = Rect::from_min_size(
            panel_rect.min + vec2(page_margin, 0.0),
            vec2(
                content_width,
                (panel_rect.height() - bottom_padding).max(0.0),
            ),
        );

        let mut content_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        flush(&mut content_ui);

        self.render_toolbar(&mut content_ui, theme, active_module);
        content_ui.add_space(16.0);

        if let Some(message) = &self.banner {
            render_banner(&mut content_ui, theme, message);
            content_ui.add_space(14.0);
        }

        if !matches!(engine.status(), EngineStatus::Ready(_)) {
            self.render_engine_status(&mut content_ui, theme, engine);
            content_ui.add_space(12.0);
        }

        let workspace_width = content_ui.available_width();
        let workspace_height = content_ui.available_height().max(0.0);
        let has_files = !self.queue.is_empty();

        if has_files && workspace_width >= 900.0 {
            content_ui.allocate_ui_with_layout(
                vec2(workspace_width, workspace_height),
                Layout::left_to_right(Align::Min),
                |ui| {
                    flush(ui);
                    let gutter = 16.0;
                    let usable_width = (workspace_width - gutter * 2.0).max(0.0);
                    let queue_width = usable_width * 0.28;
                    let center_width = usable_width * 0.38;

                    ui.allocate_ui_with_layout(
                        vec2(queue_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_queue(ui, theme, workspace_height);
                        },
                    );
                    ui.add_space(gutter);
                    ui.allocate_ui_with_layout(
                        vec2(center_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_drop_zone(ui, ctx, theme, workspace_height * 0.45, engine);
                            ui.add_space(12.0);
                            self.render_actions(
                                ui,
                                theme,
                                (workspace_height * 0.55 - 12.0).max(0.0),
                                engine,
                            );
                        },
                    );
                    ui.add_space(gutter);
                    let settings_width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        vec2(settings_width, workspace_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            flush(ui);
                            self.render_settings_panel(ui, theme, workspace_height, engine);
                        },
                    );
                },
            );
        } else {
            let drop_height = if has_files {
                workspace_height * 0.22
            } else {
                workspace_height * 0.45
            };
            self.render_drop_zone(&mut content_ui, ctx, theme, drop_height.max(0.0), engine);
            if has_files {
                content_ui.add_space(12.0);
                let remaining_height = (workspace_height - drop_height - 12.0).max(0.0);
                let queue_height = remaining_height * 0.35;
                let settings_height = remaining_height * 0.40;
                let actions_height = remaining_height * 0.25 - 24.0;
                self.render_queue(&mut content_ui, theme, queue_height);
                content_ui.add_space(12.0);
                self.render_settings_panel(&mut content_ui, theme, settings_height, engine);
                content_ui.add_space(12.0);
                self.render_actions(&mut content_ui, theme, actions_height.max(0.0), engine);
            }
        }
    }
}
