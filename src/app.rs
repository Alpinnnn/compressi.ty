use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Layout, RichText, Stroke, TextureHandle, vec2,
};

use crate::{
    branding,
    modules::{ModuleKind, compress_photos::CompressPhotosPage},
    settings::AppSettings,
    theme::AppTheme,
    ui,
};

pub struct CompressityApp {
    active_module: Option<ModuleKind>,
    compress_photos: CompressPhotosPage,
    show_about: bool,
    show_exit_confirm: bool,
    allow_close: bool,
    theme: AppTheme,
    app_icon: Option<TextureHandle>,
    app_settings: AppSettings,
    /// Snapshot of settings from previous frame to detect changes and save.
    prev_settings_snapshot: Option<AppSettings>,
}

impl CompressityApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = AppTheme::default();
        theme.apply(&cc.egui_ctx);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Maximized(true));

        let app_settings = AppSettings::load();

        Self {
            active_module: None,
            compress_photos: CompressPhotosPage::default(),
            show_about: false,
            show_exit_confirm: false,
            allow_close: false,
            theme,
            app_icon: branding::load_app_icon_texture(&cc.egui_ctx),
            prev_settings_snapshot: Some(app_settings.clone()),
            app_settings,
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

        if self.compress_photos.is_compressing() {
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
                            self.compress_photos.cancel_compression();
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
        self.compress_photos.poll_background(ctx);
        self.handle_close_request(ctx);

        if !self.compress_photos.is_compressing() {
            self.show_exit_confirm = false;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.theme.colors.bg_base))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.theme.paint_background(ui.painter(), rect);

                match self.active_module {
                    Some(ModuleKind::CompressPhotos) => self.compress_photos.show(
                        ui,
                        ctx,
                        &self.theme,
                        &mut self.active_module,
                        &self.app_settings,
                    ),
                    Some(ModuleKind::Settings) => {
                        ui::settings_view::show(
                            ui,
                            ctx,
                            &self.theme,
                            &mut self.app_settings,
                            &mut self.active_module,
                        );
                        // Persist settings whenever they change while in settings view.
                        let changed = self
                            .prev_settings_snapshot
                            .as_ref()
                            .map(|prev| {
                                prev.default_output_folder
                                    != self.app_settings.default_output_folder
                            })
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
