#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod branding;
mod icons;
mod launch;
mod modules;
mod runtime;
mod settings;
mod theme;
mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let launch_import = launch::LaunchImport::collect_from_command_line();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Compressi.ty")
        .with_maximized(true)
        .with_min_inner_size([820.0, 560.0])
        .with_resizable(true);

    if let Some(icon) = branding::load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Compressi.ty",
        options,
        Box::new(move |cc| Ok(Box::new(app::CompressityApp::new(cc, launch_import)))),
    )
}
