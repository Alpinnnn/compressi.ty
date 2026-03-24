mod chrome;
mod controls;
mod layout;
mod queue;
mod settings_advanced;
mod settings_panel;
mod widgets;
mod workspace;

use eframe::egui::{self, Ui, vec2};

use crate::modules::compress_videos::{
    engine::VideoEngineController, models::VideoCompressionState,
};

pub(super) use super::{BannerMessage, BannerTone, CompressVideosPage};

pub(super) fn flush(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
}

pub(super) fn compact(ui: &mut Ui) {
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
}

pub(super) fn truncate_filename(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        return name.to_owned();
    }

    if let Some(dot_pos) = name.rfind('.') {
        let ext = &name[dot_pos..];
        let stem_budget = max_chars.saturating_sub(ext.len()).saturating_sub(1);
        if stem_budget >= 4 {
            return format!("{}...{}", &name[..stem_budget], ext);
        }
    }

    format!("{}...", &name[..max_chars.saturating_sub(1)])
}

pub(super) fn is_video_settings_editable(state: &VideoCompressionState) -> bool {
    !matches!(
        state,
        VideoCompressionState::Compressing(_) | VideoCompressionState::Completed(_)
    )
}

impl CompressVideosPage {
    fn pick_videos(&mut self, engine: &VideoEngineController) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Videos", &["mp4", "mov", "mkv", "webm", "avi", "m4v"])
            .pick_files()
        {
            self.add_paths(paths, engine);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context, engine: &VideoEngineController) {
        let paths = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });

        if !paths.is_empty() {
            self.add_paths(paths, engine);
        }
    }
}
