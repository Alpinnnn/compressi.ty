use eframe::egui::{ColorImage, Context, IconData, TextureHandle, TextureOptions};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg,
};

/// Desktop application id used for Wayland and Linux desktop metadata.
pub const APP_ID: &str = "io.github.Alpinnnn.Compressity";
/// Human-readable application name shown in window titles and dialogs.
pub const APP_NAME: &str = "Compressi.ty";

const APP_ICON_TEXTURE_NAME: &str = "compressity-app-icon";
const APP_ICON_SVG: &str = include_str!("../assets/icon/icon.svg");

pub fn load_window_icon() -> Option<IconData> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(APP_ICON_SVG, &options).ok()?;
    let size = tree.size().to_int_size();

    let target_size = 256;
    let longest_edge = size.width().max(size.height()).max(1);
    let scale = target_size as f32 / longest_edge as f32;
    let width = (size.width() as f32 * scale).round().max(1.0) as u32;
    let height = (size.height() as f32 * scale).round().max(1.0) as u32;

    let mut pixmap = Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    Some(IconData {
        rgba: pixmap.data().to_vec(),
        width,
        height,
    })
}

pub fn load_app_icon_texture(ctx: &Context) -> Option<TextureHandle> {
    let image = load_svg_image(96)?;
    Some(ctx.load_texture(APP_ICON_TEXTURE_NAME, image, TextureOptions::LINEAR))
}

fn load_svg_image(target_size: u32) -> Option<ColorImage> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(APP_ICON_SVG, &options).ok()?;
    let size = tree.size().to_int_size();

    let longest_edge = size.width().max(size.height()).max(1);
    let scale = target_size as f32 / longest_edge as f32;
    let width = (size.width() as f32 * scale).round().max(1.0) as u32;
    let height = (size.height() as f32 * scale).round().max(1.0) as u32;

    let mut pixmap = Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    Some(ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        pixmap.data(),
    ))
}
