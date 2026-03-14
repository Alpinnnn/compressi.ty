use eframe::egui::{self, FontFamily, FontId, RichText};

pub const HELP: char = '\u{F142}';
pub const SETTINGS: char = '\u{F43C}';
pub const CLOSE: char = '\u{F404}';
pub const BACK: char = '\u{F3CF}';
pub const ADD: char = '\u{F2C7}';
pub const DOCUMENT: char = '\u{F12F}';
pub const FOLDER: char = '\u{F139}';
pub const IMAGE: char = '\u{F147}';
pub const IMAGES: char = '\u{F148}';
pub const VIDEO: char = '\u{F256}';
pub const ARCHIVE: char = '\u{F102}';
pub const PLAY: char = '\u{F488}';

pub fn font_family() -> FontFamily {
    FontFamily::Name("ionicons".into())
}

pub fn font_id(size: f32) -> FontId {
    FontId::new(size, font_family())
}

pub fn rich(glyph: char, size: f32, color: egui::Color32) -> RichText {
    RichText::new(glyph.to_string())
        .family(font_family())
        .size(size)
        .color(color)
}
