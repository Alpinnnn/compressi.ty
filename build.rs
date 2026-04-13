#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = embed_windows_icon() {
        panic!("failed to embed windows icon: {error}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn embed_windows_icon() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs::File, path::PathBuf};

    use ico::{IconDir, IconImage, ResourceType};
    use resvg::{
        tiny_skia::{Pixmap, Transform},
        usvg,
    };

    println!("cargo:rerun-if-changed=assets/icon/icon.svg");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let svg_path = manifest_dir.join("assets").join("icon").join("icon.svg");
    let svg = std::fs::read_to_string(svg_path)?;

    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &options)?;
    let size = tree.size().to_int_size();

    let mut icon_dir = IconDir::new(ResourceType::Icon);
    for target_size in [16_u32, 24, 32, 48, 64, 128, 256] {
        let longest_edge = size.width().max(size.height()).max(1) as f32;
        let scale = target_size as f32 / longest_edge;
        let width = (size.width() as f32 * scale).round().max(1.0) as u32;
        let height = (size.height() as f32 * scale).round().max(1.0) as u32;

        let mut pixmap = Pixmap::new(target_size, target_size).ok_or("invalid icon pixmap")?;
        let offset_x = ((target_size - width) as f32 * 0.5).max(0.0);
        let offset_y = ((target_size - height) as f32 * 0.5).max(0.0);
        let transform = Transform::from_row(scale, 0.0, 0.0, scale, offset_x, offset_y);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let image = IconImage::from_rgba_data(target_size, target_size, pixmap.data().to_vec());
        icon_dir.add_entry(ico::IconDirEntry::encode(&image)?);
    }

    let icon_path = out_dir.join("compressi.ty.ico");
    let mut icon_file = File::create(&icon_path)?;
    icon_dir.write(&mut icon_file)?;

    winresource::WindowsResource::new()
        .set_icon(icon_path.to_str().ok_or("invalid icon path")?)
        .compile()?;

    Ok(())
}
