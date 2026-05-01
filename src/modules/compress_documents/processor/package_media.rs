use std::{io::Cursor, path::Path};

use image::{
    ExtendedColorType, GenericImageView, ImageEncoder, ImageReader,
    codecs::png::{CompressionType, FilterType, PngEncoder},
    imageops::FilterType as ResizeFilter,
};
use mozjpeg::{ColorSpace, Compress, ScanMode};

use crate::modules::compress_documents::models::DocumentCompressionSettings;

const MIN_OPTIMIZE_IMAGE_BYTES: usize = 16 * 1024;
const MIN_RESIZE_DIMENSION: u32 = 512;

enum PackageImageFormat {
    Jpeg,
    Png,
}

pub(super) fn should_optimize_entry(name: &str, settings: &DocumentCompressionSettings) -> bool {
    settings.package_image_optimization_enabled() && package_image_format(name).is_some()
}

pub(super) fn optimize_entry_payload(
    name: &str,
    payload: Vec<u8>,
    settings: &DocumentCompressionSettings,
) -> Result<Vec<u8>, String> {
    if payload.len() < MIN_OPTIMIZE_IMAGE_BYTES {
        return Ok(payload);
    }

    let Some(format) = package_image_format(name) else {
        return Ok(payload);
    };
    let image = ImageReader::new(Cursor::new(payload.as_slice()))
        .with_guessed_format()
        .map_err(|error| format!("Could not inspect embedded image {name}: {error}"))?
        .decode()
        .map_err(|error| format!("Could not decode embedded image {name}: {error}"))?;
    let image = resize_package_image(image, settings.package_image_resize_percent());

    let encoded = match format {
        PackageImageFormat::Jpeg => encode_jpeg(&image, settings.package_image_quality())?,
        PackageImageFormat::Png => encode_png(&image)?,
    };

    if encoded.len() < payload.len() {
        Ok(encoded)
    } else {
        Ok(payload)
    }
}

fn package_image_format(name: &str) -> Option<PackageImageFormat> {
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => Some(PackageImageFormat::Jpeg),
        "png" => Some(PackageImageFormat::Png),
        _ => None,
    }
}

fn resize_package_image(image: image::DynamicImage, resize_percent: u8) -> image::DynamicImage {
    if resize_percent >= 100 {
        return image;
    }

    let (width, height) = image.dimensions();
    if width.max(height) < MIN_RESIZE_DIMENSION {
        return image;
    }

    let scale = resize_percent as f32 / 100.0;
    let target_width = ((width as f32 * scale).round() as u32).max(1);
    let target_height = ((height as f32 * scale).round() as u32).max(1);

    image.resize(target_width, target_height, ResizeFilter::Lanczos3)
}

fn encode_jpeg(image: &image::DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = rgba_to_rgb_over_white(&image.to_rgba8());
    let quality = quality.clamp(35, 100) as f32;

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> std::io::Result<Vec<u8>> {
            let mut compressor = Compress::new(ColorSpace::JCS_RGB);
            compressor.set_scan_optimization_mode(ScanMode::AllComponentsTogether);
            compressor.set_quality(quality);
            compressor.set_progressive_mode();
            if quality < 82.0 {
                compressor.set_chroma_sampling_pixel_sizes((2, 2), (1, 1));
            } else {
                compressor.set_chroma_sampling_pixel_sizes((1, 1), (1, 1));
            }
            compressor.set_size(image.width() as usize, image.height() as usize);

            let mut started = compressor.start_compress(Vec::new())?;
            started.write_scanlines(rgb.as_raw())?;
            started.finish()
        },
    ))
    .map_err(|_| "MozJPEG failed while encoding embedded document media.".to_owned())
    .and_then(|result| result.map_err(|error| format!("MozJPEG media encode error: {error}")))
}

fn encode_png(image: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let mut encoded = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut encoded, CompressionType::Best, FilterType::Adaptive);
    encoder
        .write_image(
            rgba.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("PNG media encode error: {error}"))?;

    Ok(encoded)
}

fn rgba_to_rgb_over_white(rgba: &image::RgbaImage) -> image::RgbImage {
    let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let [red, green, blue, alpha] = pixel.0;
        let alpha = alpha as f32 / 255.0;
        let blend = |channel: u8| -> u8 {
            ((channel as f32 * alpha) + (255.0 * (1.0 - alpha))).round() as u8
        };

        rgb.put_pixel(x, y, image::Rgb([blend(red), blend(green), blend(blue)]));
    }

    rgb
}
