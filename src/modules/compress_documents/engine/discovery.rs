use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    modules::compress_documents::engine::{
        DocumentEngineInfo, DocumentEngineInventory, DocumentEngineKind, DocumentEngineSource,
    },
    runtime,
};

use super::process_utils::{background_command, run_capture};

pub(super) fn discover_inventory_in_managed_dirs() -> DocumentEngineInventory {
    scan_inventory(
        runtime::managed_pdf_engine_dir(),
        runtime::managed_package_engine_dir(),
        DocumentEngineSource::ManagedUpdate,
    )
}

pub(super) fn discover_inventory_in_bundled_dirs() -> DocumentEngineInventory {
    scan_inventory(
        runtime::bundled_pdf_engine_dir(),
        runtime::bundled_package_engine_dir(),
        DocumentEngineSource::Bundled,
    )
}

pub(super) fn discover_system_inventory() -> DocumentEngineInventory {
    DocumentEngineInventory {
        ghostscript: discover_system_ghostscript(DocumentEngineSource::SystemPath),
        qpdf: discover_system_qpdf(DocumentEngineSource::SystemPath),
        seven_zip: discover_system_seven_zip(DocumentEngineSource::SystemPath),
    }
}

pub(crate) fn discover_ghostscript_binary() -> Option<PathBuf> {
    discover_engine_binary(
        &ghostscript_binary_names(),
        &[
            runtime::managed_pdf_engine_dir(),
            runtime::bundled_pdf_engine_dir(),
        ],
    )
}

pub(crate) fn discover_qpdf_binary() -> Option<PathBuf> {
    discover_engine_binary(
        &qpdf_binary_names(),
        &[
            runtime::managed_pdf_engine_dir(),
            runtime::bundled_pdf_engine_dir(),
        ],
    )
}

pub(crate) fn discover_seven_zip_binary() -> Option<PathBuf> {
    discover_engine_binary(
        &seven_zip_binary_names(),
        &[
            runtime::managed_package_engine_dir(),
            runtime::bundled_package_engine_dir(),
        ],
    )
}

fn scan_inventory(
    pdf_dir: Option<PathBuf>,
    package_dir: Option<PathBuf>,
    source: DocumentEngineSource,
) -> DocumentEngineInventory {
    DocumentEngineInventory {
        ghostscript: discover_in_optional_dir(
            pdf_dir.as_deref(),
            &ghostscript_binary_names(),
            5,
            source,
            DocumentEngineKind::Pdf,
        ),
        qpdf: discover_in_optional_dir(
            pdf_dir.as_deref(),
            &qpdf_binary_names(),
            5,
            source,
            DocumentEngineKind::PdfPolish,
        ),
        seven_zip: discover_in_optional_dir(
            package_dir.as_deref(),
            &seven_zip_binary_names(),
            4,
            source,
            DocumentEngineKind::PackageZip,
        ),
    }
}

fn discover_engine_binary(names: &[&str], engine_dirs: &[Option<PathBuf>]) -> Option<PathBuf> {
    for dir in engine_dirs.iter().flatten() {
        for search_dir in [dir.join("bin"), dir.to_path_buf()] {
            if let Some(binary) = find_binary_in_dir(&search_dir, names, 5) {
                return Some(binary);
            }
        }
    }

    system_binary_candidates(names)
        .into_iter()
        .find(|path| path.is_file())
}

fn discover_in_optional_dir(
    dir: Option<&Path>,
    names: &[&str],
    remaining_depth: usize,
    source: DocumentEngineSource,
    kind: DocumentEngineKind,
) -> Option<DocumentEngineInfo> {
    let dir = dir?;
    for search_dir in [dir.join("bin"), dir.to_path_buf()] {
        if let Some(binary) = find_binary_in_dir(&search_dir, names, remaining_depth) {
            return inspect_engine(binary, source, kind).ok();
        }
    }
    None
}

fn discover_system_ghostscript(source: DocumentEngineSource) -> Option<DocumentEngineInfo> {
    discover_system_binary(&ghostscript_binary_names(), source, DocumentEngineKind::Pdf)
}

fn discover_system_qpdf(source: DocumentEngineSource) -> Option<DocumentEngineInfo> {
    discover_system_binary(&qpdf_binary_names(), source, DocumentEngineKind::PdfPolish)
}

fn discover_system_seven_zip(source: DocumentEngineSource) -> Option<DocumentEngineInfo> {
    discover_system_binary(
        &seven_zip_binary_names(),
        source,
        DocumentEngineKind::PackageZip,
    )
}

fn discover_system_binary(
    names: &[&str],
    source: DocumentEngineSource,
    kind: DocumentEngineKind,
) -> Option<DocumentEngineInfo> {
    system_binary_candidates(names)
        .into_iter()
        .find(|path| path.is_file())
        .and_then(|binary| inspect_engine(binary, source, kind).ok())
}

fn inspect_engine(
    path: PathBuf,
    source: DocumentEngineSource,
    kind: DocumentEngineKind,
) -> Result<DocumentEngineInfo, String> {
    let mut command = background_command(&path);
    match kind {
        DocumentEngineKind::Pdf => {
            command.arg("-version");
        }
        DocumentEngineKind::PdfPolish => {
            command.arg("--version");
            configure_document_tool_environment(&path, &mut command);
        }
        DocumentEngineKind::PackageZip => {
            command.arg("i");
        }
    }

    let output = run_capture(command)?;
    Ok(DocumentEngineInfo {
        kind,
        source,
        version: parse_version(&output, kind),
        path,
    })
}

fn parse_version(output: &str, kind: DocumentEngineKind) -> String {
    let first_line = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| kind.binary_label());

    match kind {
        DocumentEngineKind::Pdf => format!("Ghostscript {}", first_line),
        DocumentEngineKind::PdfPolish => first_line.to_owned(),
        DocumentEngineKind::PackageZip => first_line.to_owned(),
    }
}

fn system_binary_candidates(names: &[&str]) -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path_value| {
            env::split_paths(&path_value)
                .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
                .collect()
        })
        .unwrap_or_default()
}

fn find_binary_in_dir(dir: &Path, names: &[&str], remaining_depth: usize) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }

    for name in names {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if remaining_depth == 0 {
        return None;
    }

    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && let Some(binary) = find_binary_in_dir(&path, names, remaining_depth - 1)
        {
            return Some(binary);
        }
    }

    None
}

fn configure_document_tool_environment(binary: &Path, command: &mut Command) {
    if cfg!(windows) {
        return;
    }

    let Some(root) = tool_root(binary) else {
        return;
    };
    let mut paths = Vec::new();
    collect_existing_dirs(&root.join("lib"), 2, &mut paths);
    collect_existing_dirs(&root.join("lib64"), 1, &mut paths);
    if let Some(existing) = env::var_os("LD_LIBRARY_PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(joined) = env::join_paths(paths) {
        command.env("LD_LIBRARY_PATH", joined);
    }
}

fn tool_root(binary: &Path) -> Option<PathBuf> {
    let parent = binary.parent()?;
    if parent
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case("bin"))
        .unwrap_or(false)
    {
        parent.parent().map(Path::to_path_buf)
    } else {
        Some(parent.to_path_buf())
    }
}

fn collect_existing_dirs(path: &Path, remaining_depth: usize, dirs: &mut Vec<PathBuf>) {
    if !path.is_dir() {
        return;
    }

    dirs.push(path.to_path_buf());
    if remaining_depth == 0 {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_existing_dirs(&entry.path(), remaining_depth - 1, dirs);
    }
}

fn ghostscript_binary_names() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["gswin64c.exe", "gswin32c.exe", "gs.exe", "gs"]
    } else {
        vec!["gs"]
    }
}

fn qpdf_binary_names() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["qpdf.exe", "qpdf"]
    } else {
        vec!["qpdf"]
    }
}

fn seven_zip_binary_names() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["7z.exe", "7zz.exe", "7za.exe", "7z", "7zz", "7za"]
    } else {
        vec!["7zz", "7z", "7za"]
    }
}
