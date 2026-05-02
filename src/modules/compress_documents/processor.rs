use std::{
    ffi::OsStr,
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

use crate::{
    modules::compress_documents::models::{
        DocumentAsset, DocumentCompressionResult, DocumentCompressionSettings,
        DocumentCompressionState, DocumentKind, LoadedDocument, is_supported_document_path,
        supported_document_extensions,
    },
    runtime,
};

mod package_engine;
mod package_media;
mod package_zip;
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
        DocumentKind::Pdf => pdf::compress_pdf(
            asset,
            settings.pdf_settings(),
            &output_path,
            cancel_flag,
            sender,
        )?,
        kind if kind.is_zip_package() => {
            let Some(package_settings) = settings.package_settings(kind) else {
                return Err(format!("Unsupported document: {}", asset.file_name));
            };
            package_zip::recompress_zip_package(
                asset,
                package_settings,
                &output_path,
                cancel_flag,
                sender,
            )?
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
