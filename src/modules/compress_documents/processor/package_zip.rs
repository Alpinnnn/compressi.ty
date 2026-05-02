use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::modules::compress_documents::models::{
    DocumentAsset, DocumentKind, PackageDocumentCompressionSettings,
};

use super::{DocumentBatchEvent, package_engine, package_media};

pub(super) fn recompress_zip_package(
    asset: &DocumentAsset,
    settings: &PackageDocumentCompressionSettings,
    output_path: &Path,
    cancel_flag: &AtomicBool,
    sender: &Sender<DocumentBatchEvent>,
) -> Result<(), String> {
    let native_output_path = build_intermediate_output_path(output_path, "native");
    recompress_zip_package_native(asset, settings, &native_output_path, cancel_flag, sender)?;

    if cancel_flag.load(Ordering::Relaxed) {
        let _ = fs::remove_file(&native_output_path);
        return Err("Compression cancelled.".to_owned());
    }

    if package_engine::supports_external_repack(asset.kind) {
        let engine_output_path = build_intermediate_output_path(output_path, "7zip");
        let _ = sender.send(DocumentBatchEvent::FileProgress {
            id: asset.id,
            progress: 0.94,
            stage: "Optimizing ZIP package".to_owned(),
        });

        match package_engine::repack_zip_with_7zip(
            &native_output_path,
            &engine_output_path,
            settings.zip_compression_level(),
        ) {
            Ok(()) => {
                return keep_smallest_candidate(
                    &native_output_path,
                    &engine_output_path,
                    output_path,
                );
            }
            Err(_) => {
                let _ = fs::remove_file(&engine_output_path);
            }
        }
    }

    move_file_replace(&native_output_path, output_path)
}

fn recompress_zip_package_native(
    asset: &DocumentAsset,
    settings: &PackageDocumentCompressionSettings,
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
                &PackageDocumentCompressionSettings::default(),
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
    settings: &PackageDocumentCompressionSettings,
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

fn keep_smallest_candidate(
    native_output_path: &Path,
    engine_output_path: &Path,
    final_output_path: &Path,
) -> Result<(), String> {
    let native_size = fs::metadata(native_output_path)
        .map_err(|error| format!("Could not read {}: {error}", native_output_path.display()))?
        .len();
    let engine_size = fs::metadata(engine_output_path)
        .map_err(|error| format!("Could not read {}: {error}", engine_output_path.display()))?
        .len();

    if engine_size < native_size {
        let _ = fs::remove_file(native_output_path);
        move_file_replace(engine_output_path, final_output_path)
    } else {
        let _ = fs::remove_file(engine_output_path);
        move_file_replace(native_output_path, final_output_path)
    }
}

fn move_file_replace(source_path: &Path, destination_path: &Path) -> Result<(), String> {
    if destination_path.exists() {
        fs::remove_file(destination_path).map_err(|error| {
            format!("Could not replace {}: {error}", destination_path.display())
        })?;
    }
    fs::rename(source_path, destination_path).map_err(|error| {
        format!(
            "Could not move {} to {}: {error}",
            source_path.display(),
            destination_path.display()
        )
    })
}

fn build_intermediate_output_path(final_output_path: &Path, suffix: &str) -> PathBuf {
    let parent = final_output_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = final_output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("document");
    let extension = final_output_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("tmp");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    parent.join(format!("{stem}-{suffix}-{timestamp}.{extension}"))
}
