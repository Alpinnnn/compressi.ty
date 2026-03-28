use eframe::egui::{self, RichText, Ui};

use crate::{
    settings::AppSettings,
    theme::AppTheme,
    ui::components::{hint, panel},
};

pub(super) fn render_processing_settings(
    ui: &mut Ui,
    theme: &AppTheme,
    settings: &mut AppSettings,
) {
    panel::card(theme)
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            hint::title(
                ui,
                theme,
                "Processing",
                16.0,
                Some(
                    "When enabled, compatible workflows use the detected GPU automatically. When disabled, processing falls back to CPU-only software encoding.",
                ),
            );
            ui.add_space(16.0);

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut settings.use_hardware_acceleration,
                    "Use Hardware Acceleration",
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(if settings.use_hardware_acceleration {
                        "Auto GPU detect"
                    } else {
                        "Software only"
                    })
                    .size(11.0)
                    .color(theme.colors.fg_dim),
                );
            });
        });
}
