use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, Grid, Layout, Rect, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, TextureHandle, Ui, pos2, vec2,
};

use crate::{icons, modules::ModuleKind, theme::AppTheme};

use super::menu_card;

const MAIN_MENU_MODULES: [ModuleKind; 5] = [
    ModuleKind::CompressPhotos,
    ModuleKind::CompressFiles,
    ModuleKind::CompressFolder,
    ModuleKind::CompressVideos,
    ModuleKind::ArchiveExtract,
];

pub fn show(
    ui: &mut Ui,
    ctx: &egui::Context,
    theme: &AppTheme,
    app_icon: Option<&TextureHandle>,
    active_module: &mut Option<ModuleKind>,
    show_about: &mut bool,
) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            let max_w = 860.0;
            let avail = ui.available_width();
            let side = ((avail - max_w) * 0.5).max(0.0);

            ui.add_space(18.0);

            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.allocate_ui_with_layout(
                    vec2(max_w.min(avail), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| render_header(ui, theme, app_icon, active_module, show_about),
                );
            });

            ui.add_space(20.0);

            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.allocate_ui_with_layout(
                    vec2(max_w.min(avail), 0.0),
                    Layout::top_down(Align::Min),
                    |ui| render_grid(ui, theme, active_module),
                );
            });

            ui.add_space(22.0);
        });

    render_about_window(ctx, theme, show_about);
}

fn render_header(
    ui: &mut Ui,
    theme: &AppTheme,
    app_icon: Option<&TextureHandle>,
    active_module: &mut Option<ModuleKind>,
    show_about: &mut bool,
) {
    ui.horizontal(|ui| {
        paint_logo(ui, theme, app_icon);
        ui.add_space(14.0);

        ui.label(
            RichText::new("Compressi.ty")
                .size(22.0)
                .strong()
                .color(theme.colors.fg),
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, theme, icons::HELP).clicked() {
                *show_about = true;
            }
            if icon_button(ui, theme, icons::SETTINGS).clicked() {
                *active_module = Some(ModuleKind::Settings);
                *show_about = false;
            }
        });
    });
}

fn render_grid(ui: &mut Ui, theme: &AppTheme, active_module: &mut Option<ModuleKind>) {
    let avail = ui.available_width();
    let cols: usize = if avail >= 780.0 {
        3
    } else if avail >= 500.0 {
        2
    } else {
        1
    };
    let gap = 10.0;
    let card_w =
        ((avail - gap * (cols.saturating_sub(1) as f32)) / cols as f32).clamp(200.0, 280.0);
    let card_h = 148.0;

    Grid::new("main_grid")
        .num_columns(cols)
        .spacing(vec2(gap, gap))
        .min_col_width(card_w)
        .show(ui, |ui| {
            for (i, module) in MAIN_MENU_MODULES.iter().enumerate() {
                if menu_card::show(ui, theme, module.spec(), vec2(card_w, card_h)).clicked() {
                    *active_module = Some(*module);
                }
                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
}

fn render_about_window(ctx: &egui::Context, theme: &AppTheme, show_about: &mut bool) {
    if !*show_about {
        return;
    }

    egui::Window::new("About")
        .id(egui::Id::new("about_window_unique"))
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .frame(
            egui::Frame::new()
                .fill(theme.colors.surface)
                .stroke(Stroke::new(1.0, theme.colors.border))
                .corner_radius(CornerRadius::ZERO)
                .inner_margin(egui::Margin::same(24)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(300.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Compressi.ty")
                        .size(20.0)
                        .strong()
                        .color(theme.colors.fg),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(icons::rich(icons::CLOSE, 15.0, theme.colors.fg_dim))
                                .fill(theme.colors.bg_raised)
                                .stroke(Stroke::new(1.0, theme.colors.border))
                                .corner_radius(CornerRadius::ZERO),
                        )
                        .clicked()
                    {
                        *show_about = false;
                    }
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .size(12.0)
                    .color(theme.colors.fg_dim),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("Local-first compression toolkit.")
                    .size(13.0)
                    .color(theme.colors.fg_dim),
            );
        });
}

fn icon_button(ui: &mut Ui, theme: &AppTheme, glyph: char) -> egui::Response {
    let size = vec2(32.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool(response.id, response.hovered());

    let fill = theme.mix(theme.colors.bg_raised, theme.colors.surface_hover, t);
    let stroke = Stroke::new(
        1.0,
        theme.mix(theme.colors.border, theme.colors.border_focus, t),
    );

    ui.painter().rect_filled(rect, theme.rounded(8), fill);
    ui.painter()
        .rect_stroke(rect, theme.rounded(8), stroke, StrokeKind::Middle);

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        icons::font_id(15.0),
        theme.mix(theme.colors.fg_dim, theme.colors.fg, t * 0.5),
    );

    response
}

fn paint_logo(ui: &mut Ui, theme: &AppTheme, app_icon: Option<&TextureHandle>) {
    let size = vec2(36.0, 36.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let accent = theme.colors.accent;

    ui.painter().rect_filled(
        rect,
        theme.rounded(10),
        theme.mix(theme.colors.surface, accent, 0.08),
    );
    ui.painter().rect_stroke(
        rect,
        theme.rounded(10),
        Stroke::new(1.0, theme.mix(theme.colors.border, accent, 0.25)),
        StrokeKind::Middle,
    );

    if let Some(app_icon) = app_icon {
        let inner = rect.shrink(5.0);
        ui.painter().image(
            app_icon.id(),
            inner,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let inner = rect.shrink(8.0);
        for (i, alpha) in [(0.0, 1.0_f32), (0.5, 0.7), (1.0, 1.0)] {
            let y_top = inner.top() + inner.height() * i - 2.0;
            let bar =
                Rect::from_min_max(pos2(inner.left(), y_top), pos2(inner.right(), y_top + 3.5));
            ui.painter().rect_filled(
                bar,
                CornerRadius::ZERO,
                theme.mix(accent, Color32::WHITE, 0.06 * alpha),
            );
        }
    }
}
