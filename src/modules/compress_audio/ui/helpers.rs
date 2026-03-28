use eframe::egui::{self, Button, CornerRadius, RichText, Stroke, Ui};

use crate::{
    modules::{
        compress_audio::models::{AudioCompressionState, AudioFormat, AudioQueueItem},
        compress_videos::models::EncoderAvailability,
    },
    theme::AppTheme,
};

pub(super) fn toggle_button(
    ui: &mut Ui,
    theme: &AppTheme,
    selected: bool,
    label: &str,
) -> egui::Response {
    ui.add(
        Button::new(RichText::new(label).size(12.0).color(theme.colors.fg))
            .fill(if selected {
                theme.mix(theme.colors.bg_raised, theme.colors.accent, 0.16)
            } else {
                theme.colors.bg_raised
            })
            .stroke(Stroke::new(
                1.0,
                if selected {
                    theme.mix(theme.colors.border, theme.colors.accent, 0.32)
                } else {
                    theme.colors.border
                },
            ))
            .corner_radius(CornerRadius::ZERO),
    )
}

pub(super) fn selectable_option_row<T: Copy + PartialEq>(
    ui: &mut Ui,
    theme: &AppTheme,
    value: &mut Option<T>,
    options: &[(Option<T>, &str)],
) {
    ui.horizontal_wrapped(|ui| {
        for (candidate, label) in options {
            if toggle_button(ui, theme, *value == *candidate, label).clicked() {
                *value = *candidate;
            }
        }
    });
}

pub(super) fn format_available(format: AudioFormat, encoders: &EncoderAvailability) -> bool {
    match format {
        AudioFormat::Aac => encoders.supports_aac(),
        AudioFormat::Opus => encoders.supports_opus(),
        AudioFormat::Mp3 => encoders.supports_mp3(),
        AudioFormat::Flac => encoders.supports_flac(),
    }
}

pub(super) fn queue_subtitle(item: &AudioQueueItem) -> String {
    match &item.state {
        AudioCompressionState::Analyzing => "Analyzing audio...".to_owned(),
        AudioCompressionState::Ready => item
            .analysis
            .as_ref()
            .map(|analysis| analysis.headline.clone())
            .unwrap_or_else(|| "Ready".to_owned()),
        AudioCompressionState::Compressing(progress) => {
            format!("{} | {:.0}%", progress.stage, progress.progress * 100.0)
        }
        AudioCompressionState::Completed(result) => {
            format!("Done | saved {:.0}%", result.reduction_percent.max(0.0))
        }
        AudioCompressionState::Skipped(reason) => format!("Skipped | {}", reason),
        AudioCompressionState::Failed(error) => format!("Failed | {}", error),
        AudioCompressionState::Cancelled => "Cancelled".to_owned(),
    }
}

pub(super) fn overall_progress(queue: &[AudioQueueItem]) -> f32 {
    let mut total = 0.0;
    let mut count = 0.0;

    for item in queue {
        let progress = match &item.state {
            AudioCompressionState::Compressing(progress) => progress.progress,
            AudioCompressionState::Completed(_)
            | AudioCompressionState::Skipped(_)
            | AudioCompressionState::Cancelled => 1.0,
            _ => 0.0,
        };
        total += progress;
        count += 1.0;
    }

    if count <= 0.0 {
        0.0
    } else {
        (total / count).clamp(0.0, 1.0)
    }
}

pub(super) fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < units.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{bytes} {}", units[unit_index])
    } else {
        format!("{value:.1} {}", units[unit_index])
    }
}
