use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    DocumentBatchEvent,
    pdf_tools::{
        discover_ghostscript_binary, discover_qpdf_binary, run_ghostscript_tool, run_tool,
    },
};
use crate::modules::compress_documents::models::{DocumentAsset, PdfDocumentCompressionSettings};

struct PdfCandidate {
    path: PathBuf,
    label: &'static str,
}

pub(super) fn compress_pdf(
    asset: &DocumentAsset,
    settings: &PdfDocumentCompressionSettings,
    output_path: &Path,
    cancel_flag: &AtomicBool,
    sender: &Sender<DocumentBatchEvent>,
) -> Result<(), String> {
    let ghostscript_binary = discover_ghostscript_binary().ok_or_else(|| {
        "Ghostscript engine is required for PDF compression. Bundle it under engine/pdf-engine or install Ghostscript on PATH.".to_owned()
    })?;
    let mut candidates = Vec::new();
    let mut errors = Vec::new();

    build_ghostscript_candidate(
        &ghostscript_binary,
        asset,
        settings,
        output_path,
        "ghostscript",
        "Ghostscript",
        sender,
        &mut candidates,
        &mut errors,
    );

    if cancel_flag.load(Ordering::Relaxed) {
        cleanup_candidates(&candidates);
        return Err("Compression cancelled.".to_owned());
    }

    if candidates.is_empty() && settings.pdf_image_optimization_enabled() {
        let mut compatibility_settings = settings.clone();
        compatibility_settings.pdf_image_quality = 100;
        compatibility_settings.pdf_image_resolution_dpi = 300;
        build_ghostscript_candidate(
            &ghostscript_binary,
            asset,
            &compatibility_settings,
            output_path,
            "ghostscript-compatibility",
            "Ghostscript compatibility",
            sender,
            &mut candidates,
            &mut errors,
        );
    }

    if cancel_flag.load(Ordering::Relaxed) {
        cleanup_candidates(&candidates);
        return Err("Compression cancelled.".to_owned());
    }

    finalize_smallest_candidate(asset, output_path, candidates, errors, sender)
}

fn build_ghostscript_candidate(
    ghostscript_binary: &Path,
    asset: &DocumentAsset,
    settings: &PdfDocumentCompressionSettings,
    output_path: &Path,
    path_label: &str,
    result_label: &'static str,
    sender: &Sender<DocumentBatchEvent>,
    candidates: &mut Vec<PdfCandidate>,
    errors: &mut Vec<String>,
) {
    let ghostscript_path = candidate_path(output_path, path_label);
    match run_ghostscript_pdf(
        ghostscript_binary,
        asset,
        settings,
        &ghostscript_path,
        sender,
    ) {
        Ok(()) => push_optional_qpdf_candidate(
            asset,
            settings,
            result_label,
            ghostscript_path,
            sender,
            candidates,
            errors,
        ),
        Err(error) => {
            remove_candidate(&ghostscript_path);
            errors.push(error);
        }
    }
}

fn push_optional_qpdf_candidate(
    asset: &DocumentAsset,
    settings: &PdfDocumentCompressionSettings,
    original_label: &'static str,
    original_path: PathBuf,
    sender: &Sender<DocumentBatchEvent>,
    candidates: &mut Vec<PdfCandidate>,
    errors: &mut Vec<String>,
) {
    let Some(qpdf_binary) = discover_qpdf_binary() else {
        candidates.push(PdfCandidate {
            path: original_path,
            label: original_label,
        });
        return;
    };

    let polished_path = candidate_path(&original_path, "qpdf");
    match run_qpdf_pdf(
        &qpdf_binary,
        &original_path,
        settings,
        &polished_path,
        sender,
        asset.id,
    ) {
        Ok(()) => {
            remove_candidate(&original_path);
            candidates.push(PdfCandidate {
                path: polished_path,
                label: "Ghostscript + qpdf",
            });
        }
        Err(error) => {
            remove_candidate(&polished_path);
            errors.push(error);
            candidates.push(PdfCandidate {
                path: original_path,
                label: original_label,
            });
        }
    }
}

fn run_ghostscript_pdf(
    binary: &Path,
    asset: &DocumentAsset,
    settings: &PdfDocumentCompressionSettings,
    output_path: &Path,
    sender: &Sender<DocumentBatchEvent>,
) -> Result<(), String> {
    let _ = sender.send(DocumentBatchEvent::FileProgress {
        id: asset.id,
        progress: 0.34,
        stage: "Running Ghostscript PDF engine".to_owned(),
    });

    let mut args = vec![
        OsString::from("-dSAFER"),
        OsString::from("-dBATCH"),
        OsString::from("-dNOPAUSE"),
        OsString::from("-dQUIET"),
        OsString::from("-sDEVICE=pdfwrite"),
        OsString::from("-dCompatibilityLevel=1.7"),
        OsString::from("-dAutoRotatePages=/None"),
        OsString::from("-sColorConversionStrategy=LeaveColorUnchanged"),
        OsString::from("-dDetectDuplicateImages=true"),
        OsString::from("-dCompressFonts=true"),
        OsString::from("-dSubsetFonts=true"),
        OsString::from("-dEmbedAllFonts=true"),
    ];

    if settings.pdf_image_optimization_enabled() {
        append_lossy_image_args(&mut args, settings);
    } else {
        append_compatibility_image_args(&mut args);
    }

    args.push(OsString::from(format!(
        "-sOutputFile={}",
        output_path.to_string_lossy()
    )));
    args.push(asset.path.as_os_str().to_os_string());

    run_ghostscript_tool(binary, &args, "Ghostscript PDF compression")
}

fn append_lossy_image_args(args: &mut Vec<OsString>, settings: &PdfDocumentCompressionSettings) {
    let resolution = settings.pdf_image_resolution_dpi();
    let quality = settings.pdf_image_quality();
    let profile = if resolution <= 100 {
        "/screen"
    } else if resolution <= 170 {
        "/ebook"
    } else {
        "/printer"
    };
    let mono_resolution = if resolution <= 100 { 300 } else { 600 };

    args.extend([
        OsString::from(format!("-dPDFSETTINGS={profile}")),
        OsString::from("-dDownsampleColorImages=true"),
        OsString::from("-dDownsampleGrayImages=true"),
        OsString::from("-dDownsampleMonoImages=true"),
        OsString::from("-dColorImageDownsampleType=/Bicubic"),
        OsString::from("-dGrayImageDownsampleType=/Bicubic"),
        OsString::from("-dMonoImageDownsampleType=/Subsample"),
        OsString::from(format!("-dColorImageResolution={resolution}")),
        OsString::from(format!("-dGrayImageResolution={resolution}")),
        OsString::from(format!("-dMonoImageResolution={mono_resolution}")),
        OsString::from("-dColorImageDownsampleThreshold=1.1"),
        OsString::from("-dGrayImageDownsampleThreshold=1.1"),
        OsString::from("-dMonoImageDownsampleThreshold=1.1"),
        OsString::from("-dAutoFilterColorImages=false"),
        OsString::from("-dAutoFilterGrayImages=false"),
        OsString::from("-dColorImageFilter=/DCTEncode"),
        OsString::from("-dGrayImageFilter=/DCTEncode"),
        OsString::from("-dPassThroughJPEGImages=false"),
        OsString::from("-dPassThroughJPXImages=false"),
        OsString::from(format!("-dJPEGQ={quality}")),
    ]);
}

fn append_compatibility_image_args(args: &mut Vec<OsString>) {
    args.extend([
        OsString::from("-dPDFSETTINGS=/prepress"),
        OsString::from("-dDownsampleColorImages=false"),
        OsString::from("-dDownsampleGrayImages=false"),
        OsString::from("-dDownsampleMonoImages=false"),
        OsString::from("-dPassThroughJPEGImages=true"),
        OsString::from("-dPassThroughJPXImages=true"),
    ]);
}

fn run_qpdf_pdf(
    binary: &Path,
    input_path: &Path,
    settings: &PdfDocumentCompressionSettings,
    output_path: &Path,
    sender: &Sender<DocumentBatchEvent>,
    id: u64,
) -> Result<(), String> {
    let _ = sender.send(DocumentBatchEvent::FileProgress {
        id,
        progress: 0.68,
        stage: "Polishing PDF structure".to_owned(),
    });

    let args = vec![
        OsString::from("--warning-exit-0"),
        input_path.as_os_str().to_os_string(),
        OsString::from("--compress-streams=y"),
        OsString::from("--decode-level=generalized"),
        OsString::from("--recompress-flate"),
        OsString::from(format!(
            "--compression-level={}",
            settings.pdf_compression_level().max(1)
        )),
        OsString::from(if settings.pdf_object_streams {
            "--object-streams=generate"
        } else {
            "--object-streams=preserve"
        }),
        output_path.as_os_str().to_os_string(),
    ];

    run_tool(binary, &args, "qpdf PDF optimization")
}

fn finalize_smallest_candidate(
    asset: &DocumentAsset,
    output_path: &Path,
    candidates: Vec<PdfCandidate>,
    errors: Vec<String>,
    sender: &Sender<DocumentBatchEvent>,
) -> Result<(), String> {
    let _ = sender.send(DocumentBatchEvent::FileProgress {
        id: asset.id,
        progress: 0.92,
        stage: "Selecting smallest PDF".to_owned(),
    });

    let best = candidates
        .iter()
        .filter_map(|candidate| {
            fs::metadata(&candidate.path)
                .ok()
                .map(|metadata| (candidate, metadata.len()))
        })
        .filter(|(_, size)| *size > 0)
        .min_by_key(|(_, size)| *size);

    if let Some((candidate, size)) = best
        && size < asset.original_size
    {
        replace_file(&candidate.path, output_path)?;
        for extra in &candidates {
            if extra.path != candidate.path {
                remove_candidate(&extra.path);
            }
        }
        let _ = sender.send(DocumentBatchEvent::FileProgress {
            id: asset.id,
            progress: 0.98,
            stage: format!("Selected {}", candidate.label),
        });
        return Ok(());
    }

    cleanup_candidates(&candidates);
    if candidates.is_empty() {
        return Err(if errors.is_empty() {
            "Ghostscript could not produce an optimized PDF.".to_owned()
        } else {
            format!(
                "Ghostscript could not produce an optimized PDF: {}",
                errors.join(" | ")
            )
        });
    }

    fs::copy(&asset.path, output_path).map_err(|error| {
        format!("Ghostscript candidates were larger; could not preserve original bytes: {error}")
    })?;
    Ok(())
}

fn candidate_path(output_path: &Path, label: &str) -> PathBuf {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = output_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("document");
    let extension = output_path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("pdf");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros())
        .unwrap_or(0);

    parent.join(format!("{stem}-{label}-{timestamp}.{extension}"))
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination)
        .or_else(|_| {
            fs::copy(source, destination)?;
            fs::remove_file(source)?;
            Ok::<(), std::io::Error>(())
        })
        .map_err(|error| {
            format!(
                "Could not move optimized PDF to {}: {error}",
                destination.display()
            )
        })
}

fn cleanup_candidates(candidates: &[PdfCandidate]) {
    for candidate in candidates {
        remove_candidate(&candidate.path);
    }
}

fn remove_candidate(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}
