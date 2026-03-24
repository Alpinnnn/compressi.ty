use eframe::egui::{Color32, CornerRadius, Frame, Margin, Stroke};

use crate::theme::AppTheme;

/// Full-width panel with subtle border – the workhorse container.
pub fn card(theme: &AppTheme) -> Frame {
    Frame::new()
        .fill(theme.colors.surface)
        .stroke(Stroke::new(1.0, theme.colors.border))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::same(20))
}

/// A quieter, inset panel for nested content.
pub fn inset(theme: &AppTheme) -> Frame {
    Frame::new()
        .fill(theme.colors.bg_raised)
        .stroke(Stroke::new(1.0, theme.colors.border))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::same(14))
}

/// Accent-tinted panel for highlighting context.
pub fn tinted(theme: &AppTheme, tint: Color32) -> Frame {
    Frame::new()
        .fill(theme.mix(theme.colors.surface, tint, 0.06))
        .stroke(Stroke::new(1.0, theme.mix(theme.colors.border, tint, 0.20)))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::same(20))
}

/// Accent-colored chip.
pub fn chip_accent(theme: &AppTheme, tint: Color32) -> Frame {
    Frame::new()
        .fill(theme.mix(theme.colors.bg_raised, tint, 0.10))
        .stroke(Stroke::new(1.0, theme.mix(theme.colors.border, tint, 0.24)))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(Margin::symmetric(12, 5))
}
