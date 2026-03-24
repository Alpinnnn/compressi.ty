use image::{
    ExtendedColorType, GenericImageView, ImageEncoder,
    codecs::{
        avif::AvifEncoder,
        png::{CompressionType, FilterType, PngEncoder},
    },
    imageops::FilterType as ResizeFilter,
};
use mozjpeg::{ColorSpace, Compress, ScanMode};
use webp::Encoder;

use crate::modules::compress_photos::models::{
    CompressionPreset, CompressionSettings, ConvertFormat, PhotoFormat,
};

use super::metadata::{SourceMetadata, apply_container_metadata};

pub(super) fn resize_image(image: image::DynamicImage, resize_percent: u8) -> image::DynamicImage {
    if resize_percent >= 100 {
        return image;
    }

    let (width, height) = image.dimensions();
    let scale = resize_percent as f32 / 100.0;
    let target_width = ((width as f32 * scale).round() as u32).max(1);
    let target_height = ((height as f32 * scale).round() as u32).max(1);

    image.resize(target_width, target_height, ResizeFilter::Lanczos3)
}

pub(super) fn resolve_output_format(
    source_format: PhotoFormat,
    settings: &CompressionSettings,
    _has_alpha: bool,
) -> PhotoFormat {
    match settings.format_choice {
        ConvertFormat::Original => source_format,
        ConvertFormat::Jpeg => PhotoFormat::Jpeg,
        ConvertFormat::WebP => PhotoFormat::WebP,
        ConvertFormat::Avif => PhotoFormat::Avif,
    }
}

pub(super) fn encode_image(
    image: &image::DynamicImage,
    output_format: PhotoFormat,
    settings: &CompressionSettings,
    metadata: &SourceMetadata,
) -> Result<Vec<u8>, String> {
    match output_format {
        PhotoFormat::Jpeg => encode_jpeg(image, settings, metadata),
        PhotoFormat::Png => encode_png(image, settings, metadata),
        PhotoFormat::WebP => encode_webp(image, settings, metadata),
        PhotoFormat::Avif => encode_avif(image, settings, metadata),
    }
}

fn encode_jpeg(
    image: &image::DynamicImage,
    settings: &CompressionSettings,
    metadata: &SourceMetadata,
) -> Result<Vec<u8>, String> {
    let rgb = rgba_to_rgb_over_white(&image.to_rgba8());
    let quality = settings.quality.max(1) as f32;

    let encoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> std::io::Result<Vec<u8>> {
            let mut compressor = Compress::new(ColorSpace::JCS_RGB);
            compressor.set_scan_optimization_mode(ScanMode::AllComponentsTogether);
            compressor.set_quality(quality);
            compressor.set_progressive_mode();
            compressor.set_chroma_sampling_pixel_sizes((1, 1), (1, 1));
            compressor.set_size(image.width() as usize, image.height() as usize);

            let mut started = compressor.start_compress(Vec::new())?;
            started.write_scanlines(rgb.as_raw())?;
            started.finish()
        },
    ))
    .map_err(|_| "MozJPEG failed while encoding the image.".to_owned())
    .and_then(|result| result.map_err(|error| format!("MozJPEG encode error: {error}")))?;

    if metadata.exif.is_some() || metadata.icc_profile.is_some() {
        apply_container_metadata(encoded, PhotoFormat::Jpeg, metadata)
    } else {
        Ok(encoded)
    }
}

fn encode_png(
    image: &image::DynamicImage,
    settings: &CompressionSettings,
    metadata: &SourceMetadata,
) -> Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let compression = if settings.preset == CompressionPreset::MaximumQuality {
        CompressionType::Fast
    } else {
        CompressionType::Best
    };

    let mut encoded = Vec::new();
    let encoder = PngEncoder::new_with_quality(&mut encoded, compression, FilterType::Adaptive);
    encoder
        .write_image(
            rgba.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("PNG encode error: {error}"))?;

    if metadata.exif.is_some() || metadata.icc_profile.is_some() {
        apply_container_metadata(encoded, PhotoFormat::Png, metadata)
    } else {
        Ok(encoded)
    }
}

fn encode_webp(
    image: &image::DynamicImage,
    settings: &CompressionSettings,
    metadata: &SourceMetadata,
) -> Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let encoded = Encoder::from_rgba(rgba.as_raw(), image.width(), image.height())
        .encode(settings.quality as f32)
        .to_vec();

    if metadata.exif.is_some() || metadata.icc_profile.is_some() {
        apply_container_metadata(encoded, PhotoFormat::WebP, metadata)
    } else {
        Ok(encoded)
    }
}

fn encode_avif(
    image: &image::DynamicImage,
    settings: &CompressionSettings,
    metadata: &SourceMetadata,
) -> Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let width = image.width();
    let height = image.height();
    let quality = settings.quality;
    let speed = match settings.preset {
        CompressionPreset::MaximumQuality => 3,
        CompressionPreset::Balanced => 5,
        CompressionPreset::HighCompression => 7,
        CompressionPreset::UltraCompression => 8,
    };
    let exif_data = metadata.exif.as_ref().map(|bytes| bytes.to_vec());
    let icc_data = metadata.icc_profile.as_ref().map(|bytes| bytes.to_vec());

    let mut encoded = Vec::new();
    let mut encoder =
        AvifEncoder::new_with_speed_quality(&mut encoded, speed, quality).with_num_threads(Some(1));

    if let Some(exif) = exif_data {
        let _ = encoder.set_exif_metadata(exif);
    }
    if let Some(icc) = icc_data {
        let _ = encoder.set_icc_profile(icc);
    }

    encoder
        .write_image(rgba.as_raw(), width, height, ExtendedColorType::Rgba8)
        .map_err(|error| format!("AVIF encode error: {error}"))?;

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
