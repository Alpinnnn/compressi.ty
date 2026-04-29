#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::sync::Arc;

mod app;
mod branding;
mod icons;
mod launch;
mod modules;
mod process_lifecycle;
mod runtime;
mod settings;
mod single_instance;
mod theme;
mod ui;

use eframe::{egui, egui_wgpu, wgpu};

fn main() -> eframe::Result<()> {
    let launch_import = launch::LaunchImport::collect_from_command_line();
    let primary_instance = match single_instance::initialize(&launch_import) {
        Ok(single_instance::InstanceState::Primary(primary_instance)) => Some(primary_instance),
        #[cfg(target_os = "windows")]
        Ok(single_instance::InstanceState::SecondaryForwarded) => return Ok(()),
        Err(error) => {
            eprintln!("single-instance handoff unavailable: {error}");
            None
        }
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Compressi.ty")
        .with_maximized(true)
        .with_min_inner_size([820.0, 560.0])
        .with_resizable(false);

    if let Some(icon) = branding::load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = build_native_options(viewport);

    eframe::run_native(
        "Compressi.ty",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::CompressityApp::new(
                cc,
                launch_import,
                primary_instance,
            )))
        }),
    )
}

fn build_native_options(viewport: egui::ViewportBuilder) -> eframe::NativeOptions {
    let mut wgpu_options = egui_wgpu::WgpuConfiguration::default();
    wgpu_options.present_mode = wgpu::PresentMode::AutoVsync;
    wgpu_options.desired_maximum_frame_latency = Some(1);
    wgpu_options.on_surface_error = Arc::new(|error| match error {
        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
            egui_wgpu::SurfaceErrorAction::RecreateSurface
        }
        _ => egui_wgpu::SurfaceErrorAction::SkipFrame,
    });

    eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    }
}
