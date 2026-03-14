use std::{fs, path::PathBuf};

use eframe::egui::{ColorImage, Context, IconData, TextureHandle, TextureOptions};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg,
};

const APP_ICON_TEXTURE_NAME: &str = "compressity-app-icon";
const APP_ICON_PATH: &str = "assets/fonts/icon/icon.svg";

pub fn load_window_icon() -> Option<IconData> {
    let image = load_svg_image(256)?;
    Some(IconData {
        rgba: image.as_raw().to_vec(),
        width: image.width() as u32,
        height: image.height() as u32,
    })
}

pub fn load_app_icon_texture(ctx: &Context) -> Option<TextureHandle> {
    let image = load_svg_image(96)?;
    Some(ctx.load_texture(APP_ICON_TEXTURE_NAME, image, TextureOptions::LINEAR))
}

fn load_svg_image(target_size: u32) -> Option<ColorImage> {
    let svg = fs::read_to_string(find_app_icon_path()?).ok()?;
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &options).ok()?;
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

fn find_app_icon_path() -> Option<PathBuf> {
    let mut roots = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))];

    if let Ok(current_dir) = std::env::current_dir() {
        if !roots.contains(&current_dir) {
            roots.push(current_dir);
        }
    }

    roots
        .into_iter()
        .map(|root| root.join(APP_ICON_PATH))
        .find(|path| path.exists())
}
