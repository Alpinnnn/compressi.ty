#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::sync::Arc;

mod app;
mod branding;
mod file_dialog;
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

const ROOT_MIN_INNER_SIZE: [f32; 2] = [820.0, 560.0];

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

    let mut viewport = build_root_viewport();

    if let Some(icon) = branding::load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = build_native_options(viewport);

    eframe::run_native(
        branding::APP_ID,
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

fn build_root_viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title(branding::APP_NAME)
        .with_app_id(branding::APP_ID)
        .with_maximized(true)
        .with_min_inner_size(ROOT_MIN_INNER_SIZE)
}

fn build_native_options(viewport: egui::ViewportBuilder) -> eframe::NativeOptions {
    let wgpu_options = egui_wgpu::WgpuConfiguration {
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: Some(1),
        on_surface_error: Arc::new(|error| match error {
            wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                egui_wgpu::SurfaceErrorAction::RecreateSurface
            }
            _ => egui_wgpu::SurfaceErrorAction::SkipFrame,
        }),
        ..Default::default()
    };

    eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_viewport_does_not_create_fixed_wayland_size_constraints() {
        let viewport = build_root_viewport();

        assert_eq!(
            viewport.min_inner_size,
            Some(egui::vec2(ROOT_MIN_INNER_SIZE[0], ROOT_MIN_INNER_SIZE[1]))
        );
        assert_eq!(viewport.max_inner_size, None);
        assert_ne!(viewport.resizable, Some(false));
    }

    #[test]
    fn root_viewport_sets_linux_desktop_identity() {
        let viewport = build_root_viewport();

        assert_eq!(viewport.title.as_deref(), Some(branding::APP_NAME));
        assert_eq!(viewport.app_id.as_deref(), Some(branding::APP_ID));
    }
}
