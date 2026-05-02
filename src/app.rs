use crate::launch::LaunchImport;
use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, TextureHandle, vec2,
};

use crate::{
    branding,
    modules::{
        ModuleKind,
        compress_audio::CompressAudioPage,
        compress_documents::{CompressDocumentsPage, engine::DocumentEngineController},
        compress_photos::CompressPhotosPage,
        compress_videos::{CompressVideosPage, engine::VideoEngineController},
    },
    settings::AppSettings,
    single_instance::{ExternalLaunchReceiver, PrimaryInstance},
    theme::AppTheme,
    ui,
};

pub struct CompressityApp {
    active_module: Option<ModuleKind>,
    compress_audio: CompressAudioPage,
    compress_documents: CompressDocumentsPage,
    compress_photos: CompressPhotosPage,
    compress_videos: CompressVideosPage,
    show_about: bool,
    show_exit_confirm: bool,
    allow_close: bool,
    theme: AppTheme,
    app_icon: Option<TextureHandle>,
    app_settings: AppSettings,
    video_engine: VideoEngineController,
    document_engine: DocumentEngineController,
    pending_launch_import: LaunchImport,
    external_launches: Option<ExternalLaunchReceiver>,
    /// Snapshot of settings from previous frame to detect changes and save.
    prev_settings_snapshot: Option<AppSettings>,
    settings_view_state: ui::settings_view::SettingsViewState,
}

impl CompressityApp {
    fn request_repaint_if_needed(ctx: &egui::Context, repaint_after: Option<std::time::Duration>) {
        if let Some(repaint_after) = repaint_after {
            ctx.request_repaint_after(repaint_after);
        }
    }

    pub fn new(
        cc: &eframe::CreationContext<'_>,
        pending_launch_import: LaunchImport,
        primary_instance: Option<PrimaryInstance>,
    ) -> Self {
        let theme = AppTheme::default();
        theme.apply(&cc.egui_ctx);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Maximized(true));

        let app_settings = AppSettings::load();
        let mut video_engine = VideoEngineController::default();
        video_engine.refresh();
        let mut document_engine = DocumentEngineController::default();
        document_engine.ensure_ready();
        let active_module = pending_launch_import.preferred_module();
        let external_launches = primary_instance.map(|instance| instance.start(&cc.egui_ctx));

        Self {
            active_module,
            compress_audio: CompressAudioPage::default(),
            compress_documents: CompressDocumentsPage::default(),
            compress_photos: CompressPhotosPage::default(),
            compress_videos: CompressVideosPage::default(),
            show_about: false,
            show_exit_confirm: false,
            allow_close: false,
            theme,
            app_icon: branding::load_app_icon_texture(&cc.egui_ctx),
            video_engine,
            document_engine,
            pending_launch_import,
            external_launches,
            prev_settings_snapshot: Some(app_settings.clone()),
            settings_view_state: ui::settings_view::SettingsViewState::default(),
            app_settings,
        }
    }

    fn poll_external_launches(&mut self, ctx: &egui::Context) {
        let Some(external_launches) = &mut self.external_launches else {
            return;
        };

        while let Some(launch_import) = external_launches.try_recv() {
            if let Some(module) = launch_import.preferred_module() {
                self.active_module = Some(module);
            }

            self.pending_launch_import.merge(launch_import);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn apply_pending_launch_import(&mut self) {
        if self.pending_launch_import.has_audio_paths() && self.video_engine.active_info().is_some()
        {
            let audio_paths = self.pending_launch_import.take_audio_paths();
            self.compress_audio
                .queue_external_paths(audio_paths, &mut self.video_engine);
        }

        if self.pending_launch_import.has_photo_paths() {
            let photo_paths = self.pending_launch_import.take_photo_paths();
            self.compress_photos.queue_external_paths(photo_paths);
        }

        if self.pending_launch_import.has_document_paths() {
            let document_paths = self.pending_launch_import.take_document_paths();
            self.compress_documents.queue_external_paths(document_paths);
        }

        if self.pending_launch_import.has_video_paths() && self.video_engine.active_info().is_some()
        {
            let video_paths = self.pending_launch_import.take_video_paths();
            self.compress_videos
                .queue_external_paths(video_paths, &self.video_engine);
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }

        if self.allow_close {
            self.allow_close = false;
            return;
        }

        if self.compress_audio.is_compressing()
            || self.compress_documents.is_compressing()
            || self.compress_photos.is_compressing()
            || self.compress_videos.is_compressing()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_exit_confirm = true;
        }
    }

    fn render_exit_confirm(&mut self, ctx: &egui::Context) {
        if !self.show_exit_confirm {
            return;
        }

        // Dark scrim painted via an invisible Area so it sits behind the popup Window.
        egui::Area::new(egui::Id::new("exit_confirm_overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_TOP, vec2(0.0, 0.0))
            .interactable(false)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                ui.painter().rect_filled(
                    screen,
                    CornerRadius::ZERO,
                    Color32::from_rgba_premultiplied(0, 0, 0, 180),
                );
            });

        egui::Window::new("Exit confirmation")
            .id(egui::Id::new("exit_confirmation_window"))
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .frame(
                egui::Frame::new()
                    .fill(self.theme.colors.surface)
                    .stroke(Stroke::new(1.0, self.theme.colors.border))
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(egui::Margin::same(24)),
            )
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);

                ui.label(
                    RichText::new("Compression is still running")
                        .size(18.0)
                        .strong()
                        .color(self.theme.colors.fg),
                );
                ui.label(
                    RichText::new(
                        "Closing the app now will stop the ongoing compression process. Do you want to exit anyway?",
                    )
                    .size(12.5)
                    .color(self.theme.colors.fg_dim),
                );

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let exit = ui.add(
                            Button::new(
                                RichText::new("Exit app")
                                    .strong()
                                    .color(self.theme.colors.fg),
                            )
                            .fill(self.theme.colors.negative)
                            .stroke(Stroke::new(1.0, self.theme.colors.negative))
                            .corner_radius(CornerRadius::ZERO),
                        );
                        let stay = ui.add(
                            Button::new(
                                RichText::new("Stay here").color(self.theme.colors.fg),
                            )
                            .fill(self.theme.colors.bg_raised)
                            .stroke(Stroke::new(1.0, self.theme.colors.border))
                            .corner_radius(CornerRadius::ZERO),
                        );

                        if exit.clicked() {
                            self.compress_audio.cancel_compression();
                            self.compress_documents.cancel_compression();
                            self.compress_photos.cancel_compression();
                            self.compress_videos.cancel_compression();
                            self.show_exit_confirm = false;
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }

                        if stay.clicked() {
                            self.show_exit_confirm = false;
                        }
                    });
                });
            });
    }
}

impl eframe::App for CompressityApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_external_launches(ctx);
        Self::request_repaint_if_needed(ctx, self.video_engine.poll());
        Self::request_repaint_if_needed(ctx, self.document_engine.poll());
        Self::request_repaint_if_needed(
            ctx,
            self.compress_audio.poll_background(&mut self.video_engine),
        );
        self.apply_pending_launch_import();
        Self::request_repaint_if_needed(ctx, self.compress_documents.poll_background());
        Self::request_repaint_if_needed(ctx, self.compress_photos.poll_background());
        Self::request_repaint_if_needed(
            ctx,
            self.compress_videos.poll_background(
                &self.video_engine,
                self.app_settings.use_hardware_acceleration,
            ),
        );
        self.handle_close_request(ctx);

        if !self.compress_audio.is_compressing()
            && !self.compress_documents.is_compressing()
            && !self.compress_photos.is_compressing()
            && !self.compress_videos.is_compressing()
        {
            self.show_exit_confirm = false;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.theme.colors.bg_base))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.theme.paint_background(ui.painter(), rect);

                match self.active_module {
                    Some(ModuleKind::CompressAudio) => self.compress_audio.show(
                        ui,
                        ctx,
                        &self.theme,
                        &mut self.active_module,
                        &self.app_settings,
                        &mut self.video_engine,
                    ),
                    Some(ModuleKind::CompressDocuments) => self.compress_documents.show(
                        ui,
                        ctx,
                        &self.theme,
                        &mut self.active_module,
                        &self.app_settings,
                        &mut self.document_engine,
                    ),
                    Some(ModuleKind::CompressPhotos) => self.compress_photos.show(
                        ui,
                        ctx,
                        &self.theme,
                        &mut self.active_module,
                        &self.app_settings,
                    ),
                    Some(ModuleKind::CompressVideos) => self.compress_videos.show(
                        ui,
                        ctx,
                        &self.theme,
                        &mut self.active_module,
                        &self.app_settings,
                        &mut self.video_engine,
                    ),
                    Some(ModuleKind::Settings) => {
                        ui::settings_view::show(
                            ui,
                            ctx,
                            &self.theme,
                            &mut self.app_settings,
                            &mut self.settings_view_state,
                            &mut self.active_module,
                            &mut self.video_engine,
                            &mut self.document_engine,
                        );
                        // Persist settings whenever they change while in settings view.
                        let changed = self
                            .prev_settings_snapshot
                            .as_ref()
                            .map(|prev| prev != &self.app_settings)
                            .unwrap_or(true);
                        if changed {
                            self.app_settings.save();
                            self.prev_settings_snapshot = Some(self.app_settings.clone());
                        }
                    }
                    Some(module) => {
                        ui::module_view::show(ui, ctx, &self.theme, module, &mut self.active_module)
                    }
                    None => ui::main_menu::show(
                        ui,
                        ctx,
                        &self.theme,
                        self.app_icon.as_ref(),
                        &mut self.active_module,
                        &mut self.show_about,
                    ),
                }
            });

        self.render_exit_confirm(ctx);
    }
}
