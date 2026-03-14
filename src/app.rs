use eframe::egui::{self, TextureHandle};

use crate::{
    branding,
    modules::{ModuleKind, compress_photos::CompressPhotosPage},
    theme::AppTheme,
    ui,
};

pub struct CompressityApp {
    active_module: Option<ModuleKind>,
    compress_photos: CompressPhotosPage,
    show_about: bool,
    theme: AppTheme,
    app_icon: Option<TextureHandle>,
}

impl CompressityApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = AppTheme::default();
        theme.apply(&cc.egui_ctx);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Maximized(true));

        Self {
            active_module: None,
            compress_photos: CompressPhotosPage::default(),
            show_about: false,
            theme,
            app_icon: branding::load_app_icon_texture(&cc.egui_ctx),
        }
    }
}

impl eframe::App for CompressityApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.compress_photos.poll_background(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.theme.colors.bg_base))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.theme.paint_background(ui.painter(), rect);

                match self.active_module {
                    Some(ModuleKind::CompressPhotos) => {
                        self.compress_photos
                            .show(ui, ctx, &self.theme, &mut self.active_module)
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
    }
}
