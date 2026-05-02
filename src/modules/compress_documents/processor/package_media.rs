use std::{io::Cursor, path::Path};

use image::{
    ExtendedColorType, GenericImageView, ImageEncoder, ImageReader,
    codecs::png::{CompressionType, FilterType, PngEncoder},
    imageops::FilterType as ResizeFilter,
};
use mozjpeg::{ColorSpace, Compress, ScanMode};

use crate::modules::compress_documents::models::PackageDocumentCompressionSettings;

const MIN_OPTIMIZE_IMAGE_BYTES: usize = 16 * 1024;
const MIN_RESIZE_DIMENSION: u32 = 512;

#[derive(Clone, Copy)]
enum PackageImageFormat {
    Jpeg,
    Png,
}

pub(super) fn should_optimize_entry(
    name: &str,
    settings: &PackageDocumentCompressionSettings,
) -> bool {
    settings.package_image_optimization_enabled() && package_image_format(name).is_some()
}

pub(super) fn optimize_entry_payload(
    name: &str,
    payload: Vec<u8>,
    settings: &PackageDocumentCompressionSettings,
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

    let mut best_payload = payload;
    let mut best_size = best_payload.len();
    for candidate in optimized_payload_candidates(format, image, settings)? {
        if candidate.len() < best_size {
            best_size = candidate.len();
            best_payload = candidate;
        }
    }

    Ok(best_payload)
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

fn optimized_payload_candidates(
    format: PackageImageFormat,
    image: image::DynamicImage,
    settings: &PackageDocumentCompressionSettings,
) -> Result<Vec<Vec<u8>>, String> {
    let resize_percent = settings.package_image_resize_percent();
    let quality = settings.package_image_quality();
    let original_width = image.width();
    let original_height = image.height();
    let mut candidates = vec![encode_package_image(format, &image, quality)?];

    if resize_percent < 100 {
        let resized = resize_package_image(image, resize_percent);
        if resized.width() != original_width || resized.height() != original_height {
            candidates.push(encode_package_image(format, &resized, quality)?);
        }
    }

    Ok(candidates)
}

fn encode_package_image(
    format: PackageImageFormat,
    image: &image::DynamicImage,
    quality: u8,
) -> Result<Vec<u8>, String> {
    match format {
        PackageImageFormat::Jpeg => encode_jpeg(image, quality),
        PackageImageFormat::Png => encode_png(image),
    }
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

#[cfg(test)]
mod tests {
    use image::{
        ExtendedColorType, ImageBuffer, ImageEncoder, Rgba,
        codecs::png::{CompressionType, FilterType, PngEncoder},
    };

    use crate::modules::compress_documents::models::{
        DocumentCompressionPreset, PackageDocumentCompressionSettings,
    };

    use super::{encode_png, optimize_entry_payload, resize_package_image};

    #[test]
    fn png_optimization_keeps_smallest_candidate() {
        let mut image = ImageBuffer::new(768, 768);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let value = if ((x / 8) + (y / 8)) % 2 == 0 {
                20
            } else {
                235
            };
            *pixel = Rgba([value, value, value, 255]);
        }

        let image = image::DynamicImage::ImageRgba8(image);
        let source_payload = encode_fast_source_png(&image);
        let mut settings = PackageDocumentCompressionSettings::default();
        settings.apply_preset(DocumentCompressionPreset::UltraCompression);

        let optimized =
            optimize_entry_payload("word/media/image1.png", source_payload.clone(), &settings)
                .unwrap();
        let lossless_payload = encode_png(&image).unwrap();
        let resized_image = resize_package_image(image, settings.package_image_resize_percent());
        let resized_payload = encode_png(&resized_image).unwrap();
        let expected_size = [
            source_payload.len(),
            lossless_payload.len(),
            resized_payload.len(),
        ]
        .into_iter()
        .min()
        .unwrap();

        assert!(
            lossless_payload.len() < resized_payload.len(),
            "fixture should make the lossless candidate smaller than the resized PNG"
        );
        assert_eq!(optimized.len(), expected_size);
    }

    fn encode_fast_source_png(image: &image::DynamicImage) -> Vec<u8> {
        let rgba = image.to_rgba8();
        let mut encoded = Vec::new();
        let encoder =
            PngEncoder::new_with_quality(&mut encoded, CompressionType::Fast, FilterType::NoFilter);
        encoder
            .write_image(
                rgba.as_raw(),
                image.width(),
                image.height(),
                ExtendedColorType::Rgba8,
            )
            .unwrap();
        encoded
    }
}
