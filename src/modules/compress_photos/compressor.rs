use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use image::{
    ExtendedColorType, GenericImageView, ImageEncoder, ImageReader,
    codecs::{
        avif::AvifEncoder,
        png::{CompressionType, FilterType, PngEncoder},
    },
    imageops::FilterType as ResizeFilter,
};
use img_parts::{ImageEXIF, ImageICC};
use mozjpeg::{ColorSpace, Compress, ScanMode};
use rayon::prelude::*;
use webp::Encoder;

use super::models::{
    CompressionPreset, CompressionResult, CompressionSettings, CompressionState, ConvertFormat,
    LoadedPhoto, PhotoAsset, PhotoFormat, PhotoPreview,
};

#[derive(Debug)]
pub enum CompressionEvent {
    FileStarted {
        id: u64,
    },
    FileProgress {
        id: u64,
        progress: f32,
        stage: String,
    },
    FileFinished {
        id: u64,
        result: CompressionResult,
    },
    FileFailed {
        id: u64,
        error: String,
    },
    BatchFinished {
        cancelled: bool,
    },
}

pub struct CompressionHandle {
    pub output_dir: PathBuf,
    pub receiver: Receiver<CompressionEvent>,
    cancel_flag: Arc<AtomicBool>,
}

impl CompressionHandle {
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone, Default)]
struct SourceMetadata {
    exif: Option<img_parts::Bytes>,
    icc_profile: Option<img_parts::Bytes>,
}

pub fn load_photo(path: PathBuf, id: u64) -> Result<LoadedPhoto, String> {
    let format = PhotoFormat::from_path(&path)
        .ok_or_else(|| format!("Unsupported format: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image")
        .to_owned();
    let original_size = fs::metadata(&path)
        .map_err(|error| {
            format!(
                "Could not read file metadata for {}: {error}",
                path.display()
            )
        })?
        .len();
    let image = ImageReader::open(&path)
        .map_err(|error| format!("Could not open {}: {error}", path.display()))?
        .decode()
        .map_err(|error| format!("Could not decode {}: {error}", path.display()))?;
    let dimensions = image.dimensions();
    let thumbnail = image.thumbnail(112, 112).to_rgba8();
    let preview_size = [thumbnail.width() as usize, thumbnail.height() as usize];

    Ok(LoadedPhoto {
        asset: PhotoAsset {
            id,
            path,
            file_name,
            original_size,
            format,
            dimensions,
        },
        preview: Some(PhotoPreview {
            rgba: thumbnail.into_raw(),
            size: preview_size,
        }),
    })
}

pub fn start_batch(
    files: Vec<PhotoAsset>,
    settings: CompressionSettings,
    custom_output_dir: Option<PathBuf>,
) -> Result<CompressionHandle, String> {
    if files.is_empty() {
        return Err("Add at least one supported image before compressing.".to_owned());
    }

    let output_dir = match custom_output_dir {
        Some(dir) => dir,
        None => create_output_dir()?,
    };
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "Could not create output folder {}: {error}",
            output_dir.display()
        )
    })?;

    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let thread_cancel = Arc::clone(&cancel_flag);
    let thread_output_dir = output_dir.clone();

    thread::spawn(move || {
        // Each file runs on the rayon pool so large batches stay responsive while the UI keeps polling progress.
        files
            .into_par_iter()
            .for_each_with(sender.clone(), |sender, asset| {
                if thread_cancel.load(Ordering::Relaxed) {
                    return;
                }

                let _ = sender.send(CompressionEvent::FileStarted { id: asset.id });
                match compress_one(
                    &asset,
                    &settings,
                    &thread_output_dir,
                    &thread_cancel,
                    sender,
                ) {
                    Ok(Some(result)) => {
                        let _ = sender.send(CompressionEvent::FileFinished {
                            id: asset.id,
                            result,
                        });
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = sender.send(CompressionEvent::FileFailed {
                            id: asset.id,
                            error,
                        });
                    }
                }
            });

        let _ = sender.send(CompressionEvent::BatchFinished {
            cancelled: thread_cancel.load(Ordering::Relaxed),
        });
    });

    Ok(CompressionHandle {
        output_dir,
        receiver,
        cancel_flag,
    })
}

fn compress_one(
    asset: &PhotoAsset,
    settings: &CompressionSettings,
    output_dir: &Path,
    cancel_flag: &AtomicBool,
    sender: &Sender<CompressionEvent>,
) -> Result<Option<CompressionResult>, String> {
    report_progress(sender, asset.id, 0.08, "Reading image");
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(None);
    }

    let metadata = if settings.strip_metadata {
        SourceMetadata::default()
    } else {
        read_source_metadata(asset)
    };

    let image = ImageReader::open(&asset.path)
        .map_err(|error| format!("Could not open {}: {error}", asset.file_name))?
        .decode()
        .map_err(|error| format!("Could not decode {}: {error}", asset.file_name))?;

    report_progress(sender, asset.id, 0.24, "Preparing pixels");
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(None);
    }

    let working_image = resize_image(image, settings.resize_percent);
    let output_format =
        resolve_output_format(asset.format, settings, working_image.color().has_alpha());

    report_progress(sender, asset.id, 0.48, "Encoding");
    let encoded = encode_image(&working_image, output_format, settings, &metadata)?;

    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(None);
    }

    report_progress(sender, asset.id, 0.84, "Writing output");
    let output_path = build_output_path(output_dir, asset, output_format);
    fs::write(&output_path, &encoded).map_err(|error| {
        format!(
            "Could not write compressed image for {}: {error}",
            asset.file_name
        )
    })?;

    let compressed_size = encoded.len() as u64;
    let reduction_percent = if asset.original_size == 0 {
        0.0
    } else {
        100.0 - ((compressed_size as f32 / asset.original_size as f32) * 100.0)
    };

    Ok(Some(CompressionResult {
        output_path,
        output_format,
        original_size: asset.original_size,
        compressed_size,
        reduction_percent,
    }))
}

fn resize_image(image: image::DynamicImage, resize_percent: u8) -> image::DynamicImage {
    if resize_percent >= 100 {
        return image;
    }

    let (width, height) = image.dimensions();
    let scale = resize_percent as f32 / 100.0;
    let target_width = ((width as f32 * scale).round() as u32).max(1);
    let target_height = ((height as f32 * scale).round() as u32).max(1);

    image.resize(target_width, target_height, ResizeFilter::Lanczos3)
}

fn resolve_output_format(
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

fn encode_image(
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
    // AVIF encoding goes through ravif/rav1e. By default that stack borrows
    // rayon's global thread pool, which is the same pool the batch runner
    // already saturates with `into_par_iter()`. The result is a deadlock-like
    // stall where progress never moves past "Encoding".
    //
    // Fix: force ravif/rav1e to use a tiny dedicated pool per encode instead of
    // the shared global pool. A single worker avoids the stall and keeps thread
    // fan-out under control when multiple files are processed together.

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
    let exif_data = metadata.exif.as_ref().map(|b| b.to_vec());
    let icc_data = metadata.icc_profile.as_ref().map(|b| b.to_vec());

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

fn apply_container_metadata(
    encoded: Vec<u8>,
    output_format: PhotoFormat,
    metadata: &SourceMetadata,
) -> Result<Vec<u8>, String> {
    match output_format {
        PhotoFormat::Jpeg => {
            let mut image = img_parts::jpeg::Jpeg::from_bytes(encoded.into())
                .map_err(|error| format!("Could not restore JPEG metadata: {error}"))?;
            image.set_exif(metadata.exif.clone());
            image.set_icc_profile(metadata.icc_profile.clone());
            let mut output = Vec::new();
            image
                .encoder()
                .write_to(&mut output)
                .map_err(|error| format!("Could not write JPEG metadata: {error}"))?;
            Ok(output)
        }
        PhotoFormat::Png => {
            let mut image = img_parts::png::Png::from_bytes(encoded.into())
                .map_err(|error| format!("Could not restore PNG metadata: {error}"))?;
            image.set_exif(metadata.exif.clone());
            image.set_icc_profile(metadata.icc_profile.clone());
            let mut output = Vec::new();
            image
                .encoder()
                .write_to(&mut output)
                .map_err(|error| format!("Could not write PNG metadata: {error}"))?;
            Ok(output)
        }
        PhotoFormat::WebP => {
            let mut image = img_parts::webp::WebP::from_bytes(encoded.into())
                .map_err(|error| format!("Could not restore WebP metadata: {error}"))?;
            image.set_exif(metadata.exif.clone());
            image.set_icc_profile(metadata.icc_profile.clone());
            let mut output = Vec::new();
            image
                .encoder()
                .write_to(&mut output)
                .map_err(|error| format!("Could not write WebP metadata: {error}"))?;
            Ok(output)
        }
        PhotoFormat::Avif => Ok(encoded),
    }
}

fn read_source_metadata(asset: &PhotoAsset) -> SourceMetadata {
    let bytes = match fs::read(&asset.path) {
        Ok(bytes) => bytes,
        Err(_) => return SourceMetadata::default(),
    };

    match asset.format {
        PhotoFormat::Jpeg => img_parts::jpeg::Jpeg::from_bytes(bytes.into())
            .map(|image| SourceMetadata {
                exif: image.exif(),
                icc_profile: image.icc_profile(),
            })
            .unwrap_or_default(),
        PhotoFormat::Png => img_parts::png::Png::from_bytes(bytes.into())
            .map(|image| SourceMetadata {
                exif: image.exif(),
                icc_profile: image.icc_profile(),
            })
            .unwrap_or_default(),
        PhotoFormat::WebP => img_parts::webp::WebP::from_bytes(bytes.into())
            .map(|image| SourceMetadata {
                exif: image.exif(),
                icc_profile: image.icc_profile(),
            })
            .unwrap_or_default(),
        PhotoFormat::Avif => SourceMetadata::default(),
    }
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

fn build_output_path(output_dir: &Path, asset: &PhotoAsset, output_format: PhotoFormat) -> PathBuf {
    let stem = asset
        .path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let safe_stem = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    output_dir.join(format!(
        "{}-compressed-{}.{}",
        safe_stem,
        asset.id,
        output_format.extension()
    ))
}

fn create_output_dir() -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Clock error: {error}"))?
        .as_secs();
    let base = std::env::current_dir()
        .map_err(|error| format!("Could not read working directory: {error}"))?;

    Ok(base
        .join("compressity-output")
        .join("photos")
        .join(format!("run-{timestamp}")))
}

fn report_progress(sender: &Sender<CompressionEvent>, id: u64, progress: f32, stage: &str) {
    let _ = sender.send(CompressionEvent::FileProgress {
        id,
        progress,
        stage: stage.to_owned(),
    });
}

pub fn mark_cancelled(state: &mut CompressionState) {
    if matches!(
        state,
        CompressionState::Ready | CompressionState::Compressing(_)
    ) {
        *state = CompressionState::Cancelled;
    }
}
