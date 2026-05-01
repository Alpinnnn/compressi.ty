use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    modules::compress_documents::models::{
        DocumentAsset, DocumentCompressionResult, DocumentCompressionSettings,
        DocumentCompressionState, DocumentKind, LoadedDocument, is_supported_document_path,
        supported_document_extensions,
    },
    runtime,
};

mod package_media;
mod pdf;
mod pdf_tools;

/// Events emitted by the background document compression worker.
#[derive(Debug)]
pub enum DocumentBatchEvent {
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
        result: DocumentCompressionResult,
    },
    FileFailed {
        id: u64,
        error: String,
    },
    BatchFinished {
        cancelled: bool,
    },
}

/// Handle for a running document compression batch.
pub struct DocumentBatchHandle {
    pub output_dir: PathBuf,
    pub receiver: Receiver<DocumentBatchEvent>,
    cancel_flag: Arc<AtomicBool>,
}

impl DocumentBatchHandle {
    /// Signals the background worker to stop after the current file boundary.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

/// Returns true when the path is supported by the document compressor.
pub fn is_supported_path(path: &Path) -> bool {
    is_supported_document_path(path)
}

/// Returns the file extensions accepted by the document compressor.
pub fn supported_extensions() -> &'static [&'static str] {
    supported_document_extensions()
}

/// Loads source document metadata for queue insertion.
pub fn load_document(path: PathBuf, id: u64) -> Result<LoadedDocument, String> {
    let kind = DocumentKind::from_path(&path)
        .ok_or_else(|| format!("Unsupported document: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("document")
        .to_owned();
    let original_size = fs::metadata(&path)
        .map_err(|error| {
            format!(
                "Could not read document metadata for {}: {error}",
                path.display()
            )
        })?
        .len();

    Ok(LoadedDocument {
        asset: DocumentAsset {
            id,
            path,
            file_name,
            original_size,
            kind,
        },
    })
}

/// Starts a background document compression batch.
pub fn start_batch(
    documents: Vec<DocumentAsset>,
    settings: DocumentCompressionSettings,
    custom_output_dir: Option<PathBuf>,
) -> Result<DocumentBatchHandle, String> {
    if documents.is_empty() {
        return Err("Add at least one supported document before compressing.".to_owned());
    }

    let output_dir = resolve_output_dir(custom_output_dir)?;
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
        let mut cancelled = false;
        for document in documents {
            if thread_cancel.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }

            let id = document.id;
            let _ = sender.send(DocumentBatchEvent::FileStarted { id });
            match compress_one(
                &document,
                &settings,
                &thread_output_dir,
                &thread_cancel,
                &sender,
            ) {
                Ok(result) => {
                    let _ = sender.send(DocumentBatchEvent::FileFinished { id, result });
                }
                Err(error) => {
                    if thread_cancel.load(Ordering::Relaxed) {
                        cancelled = true;
                    }
                    let _ = sender.send(DocumentBatchEvent::FileFailed { id, error });
                }
            }
        }

        let _ = sender.send(DocumentBatchEvent::BatchFinished { cancelled });
    });

    Ok(DocumentBatchHandle {
        output_dir,
        receiver,
        cancel_flag,
    })
}

/// Marks any non-final queue state as cancelled after a stop request.
pub fn mark_cancelled(state: &mut DocumentCompressionState) {
    if matches!(
        state,
        DocumentCompressionState::Ready | DocumentCompressionState::Compressing(_)
    ) {
        *state = DocumentCompressionState::Cancelled;
    }
}

fn compress_one(
    asset: &DocumentAsset,
    settings: &DocumentCompressionSettings,
    output_dir: &Path,
    cancel_flag: &AtomicBool,
    sender: &Sender<DocumentBatchEvent>,
) -> Result<DocumentCompressionResult, String> {
    let extension = asset
        .path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("document");
    let output_path = build_unique_output_path(output_dir, &asset.path, "compressed", extension);

    let _ = sender.send(DocumentBatchEvent::FileProgress {
        id: asset.id,
        progress: 0.12,
        stage: "Preparing".to_owned(),
    });

    match asset.kind {
        DocumentKind::Pdf => pdf::compress_pdf(asset, settings, &output_path, cancel_flag, sender)?,
        kind if kind.is_zip_package() => {
            recompress_zip_package(asset, settings, &output_path, cancel_flag, sender)?
        }
        _ => return Err(format!("Unsupported document: {}", asset.file_name)),
    }

    let compressed_size = fs::metadata(&output_path)
        .map_err(|error| {
            format!(
                "Could not read compressed output {}: {error}",
                output_path.display()
            )
        })?
        .len();
    let reduction_percent = if asset.original_size > 0 {
        100.0 - ((compressed_size as f32 / asset.original_size as f32) * 100.0)
    } else {
        0.0
    };

    Ok(DocumentCompressionResult {
        output_path,
        original_size: asset.original_size,
        compressed_size,
        reduction_percent,
    })
}

fn recompress_zip_package(
    asset: &DocumentAsset,
    settings: &DocumentCompressionSettings,
    output_path: &Path,
    cancel_flag: &AtomicBool,
    sender: &Sender<DocumentBatchEvent>,
) -> Result<(), String> {
    let input = File::open(&asset.path)
        .map_err(|error| format!("Could not open {}: {error}", asset.path.display()))?;
    let mut archive = ZipArchive::new(input)
        .map_err(|error| format!("Could not read package {}: {error}", asset.path.display()))?;
    let output = File::create(output_path)
        .map_err(|error| format!("Could not create {}: {error}", output_path.display()))?;
    let mut writer = ZipWriter::new(output);
    let total_entries = archive.len().max(1);

    if requires_stored_mimetype(asset.kind) {
        write_stored_mimetype_first(&mut archive, &mut writer)?;
    }

    for index in 0..archive.len() {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Compression cancelled.".to_owned());
        }

        let mut entry = archive.by_index(index).map_err(|error| {
            format!(
                "Could not read package entry {index} in {}: {error}",
                asset.file_name
            )
        })?;
        let name = entry.name().to_owned();
        if requires_stored_mimetype(asset.kind) && name == "mimetype" {
            continue;
        }
        if entry.enclosed_name().is_none() {
            return Err(format!("Package entry has an unsafe path: {name}"));
        }

        let progress = 0.18 + ((index + 1) as f32 / total_entries as f32) * 0.68;
        let _ = sender.send(DocumentBatchEvent::FileProgress {
            id: asset.id,
            progress,
            stage: format!("Packing {}/{}", index + 1, total_entries),
        });

        let unix_mode = entry.unix_mode();
        if entry.is_dir() {
            let options = zip_entry_options(settings, false, unix_mode, false);
            writer
                .add_directory(name, options)
                .map_err(|error| format!("Could not add directory to package: {error}"))?;
        } else if package_media::should_optimize_entry(&name, settings) {
            let mut payload = Vec::new();
            entry
                .read_to_end(&mut payload)
                .map_err(|error| format!("Could not read package media entry: {error}"))?;
            let payload = package_media::optimize_entry_payload(&name, payload, settings)?;
            let large_file = payload.len() > u32::MAX as usize;
            let options = zip_entry_options(settings, false, unix_mode, large_file);
            writer
                .start_file(name, options)
                .map_err(|error| format!("Could not add file to package: {error}"))?;
            writer
                .write_all(&payload)
                .map_err(|error| format!("Could not write optimized package media: {error}"))?;
        } else {
            let large_file = entry.size() > u64::from(u32::MAX);
            let options = zip_entry_options(settings, false, unix_mode, large_file);
            writer
                .start_file(name, options)
                .map_err(|error| format!("Could not add file to package: {error}"))?;
            io::copy(&mut entry, &mut writer)
                .map_err(|error| format!("Could not recompress package entry: {error}"))?;
        }
    }

    let _ = sender.send(DocumentBatchEvent::FileProgress {
        id: asset.id,
        progress: 0.92,
        stage: "Finalizing package".to_owned(),
    });
    writer
        .finish()
        .map_err(|error| format!("Could not finalize package: {error}"))?;

    Ok(())
}

fn write_stored_mimetype_first<W: io::Write + io::Seek>(
    archive: &mut ZipArchive<File>,
    writer: &mut ZipWriter<W>,
) -> Result<(), String> {
    let Ok(mut mimetype) = archive.by_name("mimetype") else {
        return Ok(());
    };
    writer
        .start_file(
            "mimetype",
            zip_entry_options(
                &DocumentCompressionSettings::default(),
                true,
                mimetype.unix_mode(),
                false,
            ),
        )
        .map_err(|error| format!("Could not preserve stored mimetype entry: {error}"))?;
    io::copy(&mut mimetype, writer)
        .map_err(|error| format!("Could not copy stored mimetype entry: {error}"))?;
    Ok(())
}

fn requires_stored_mimetype(kind: DocumentKind) -> bool {
    matches!(kind, DocumentKind::OpenDocument | DocumentKind::Epub)
}

fn zip_entry_options(
    settings: &DocumentCompressionSettings,
    stored: bool,
    unix_mode: Option<u32>,
    large_file: bool,
) -> SimpleFileOptions {
    let method = if stored {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    let level = if stored {
        None
    } else {
        Some(settings.zip_compression_level())
    };
    let mut options = SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .large_file(large_file);

    if let Some(mode) = unix_mode {
        options = options.unix_permissions(mode);
    }

    options
}

fn resolve_output_dir(base_output_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match base_output_dir {
        Some(path) => Ok(path),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("Clock error: {error}"))?
                .as_secs();
            Ok(runtime::default_document_output_root().join(format!("run-{timestamp}")))
        }
    }
}

fn build_unique_output_path(
    output_dir: &Path,
    source: &Path,
    suffix: &str,
    extension: &str,
) -> PathBuf {
    let candidate = output_dir.join(build_output_name(source, suffix, extension));
    if !candidate.exists() {
        return candidate;
    }

    let safe_stem = safe_stem(source, "document");
    for counter in 1..=999 {
        let path = output_dir.join(format!("{safe_stem}-{suffix}-{counter}.{extension}"));
        if !path.exists() {
            return path;
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    output_dir.join(format!("{safe_stem}-{suffix}-{timestamp}.{extension}"))
}

fn build_output_name(source: &Path, suffix: &str, extension: &str) -> String {
    format!("{}-{suffix}.{extension}", safe_stem(source, "document"))
}

fn safe_stem(source: &Path, fallback: &str) -> String {
    source
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or(fallback)
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}
