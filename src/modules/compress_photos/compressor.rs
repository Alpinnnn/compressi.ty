mod encoding;
mod metadata;
mod output;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use image::{GenericImageView, ImageReader};
use rayon::prelude::*;

use super::models::{
    CompressionResult, CompressionSettings, CompressionState, LoadedPhoto, PhotoAsset, PhotoFormat,
    PhotoPreview,
};

use self::{
    encoding::{encode_image, resize_image, resolve_output_format},
    metadata::{SourceMetadata, read_source_metadata},
    output::{build_output_path, create_output_dir},
};

/// Events emitted by the background photo compression worker.
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

/// Handle for a running photo compression batch.
pub struct CompressionHandle {
    pub output_dir: PathBuf,
    pub receiver: Receiver<CompressionEvent>,
    cancel_flag: Arc<AtomicBool>,
}

impl CompressionHandle {
    /// Signals the worker pool to stop processing remaining photos.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

/// Loads a source image, validates its format, and prepares a small preview thumbnail.
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

/// Starts a background photo compression batch for the provided queue items.
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

fn report_progress(sender: &Sender<CompressionEvent>, id: u64, progress: f32, stage: &str) {
    let _ = sender.send(CompressionEvent::FileProgress {
        id,
        progress,
        stage: stage.to_owned(),
    });
}

/// Marks any non-final queue state as cancelled after a batch stop request.
pub fn mark_cancelled(state: &mut CompressionState) {
    if matches!(
        state,
        CompressionState::Ready | CompressionState::Compressing(_)
    ) {
        *state = CompressionState::Cancelled;
    }
}
