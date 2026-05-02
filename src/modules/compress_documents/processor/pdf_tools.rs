use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::modules::compress_documents::engine;

pub(super) fn run_tool(binary: &Path, args: &[OsString], label: &str) -> Result<(), String> {
    let mut command = Command::new(binary);
    command.args(args);
    configure_document_tool_environment(binary, &mut command);
    run_command(command, binary, label)
}

pub(super) fn run_ghostscript_tool(
    binary: &Path,
    args: &[OsString],
    label: &str,
) -> Result<(), String> {
    let mut command = Command::new(binary);
    command.args(args);
    configure_ghostscript_environment(binary, &mut command);
    run_command(command, binary, label)
}

fn run_command(mut command: Command, binary: &Path, label: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("{label} could not start at {}: {error}", binary.display()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };

    Err(format!(
        "{label} failed with status {}{}",
        output.status,
        if detail.is_empty() {
            String::new()
        } else {
            format!(": {}", detail.chars().take(600).collect::<String>())
        }
    ))
}

pub(super) fn discover_ghostscript_binary() -> Option<PathBuf> {
    engine::discover_ghostscript_binary()
}

pub(super) fn discover_qpdf_binary() -> Option<PathBuf> {
    engine::discover_qpdf_binary()
}

fn configure_ghostscript_environment(binary: &Path, command: &mut Command) {
    let Some(root) = ghostscript_root(binary) else {
        return;
    };

    let mut resource_dirs = Vec::new();
    collect_existing_dirs(&root.join("lib"), 2, &mut resource_dirs);
    collect_existing_dirs(&root.join("Resource"), 3, &mut resource_dirs);
    collect_existing_dirs(&root.join("fonts"), 1, &mut resource_dirs);
    collect_existing_dirs(
        &root.join("share").join("ghostscript"),
        4,
        &mut resource_dirs,
    );
    append_env_paths(command, "GS_LIB", resource_dirs);

    if !cfg!(windows) {
        let mut library_dirs = Vec::new();
        collect_existing_dirs(&root.join("lib"), 3, &mut library_dirs);
        collect_existing_dirs(&root.join("usr").join("lib"), 3, &mut library_dirs);
        let key = if cfg!(target_os = "macos") {
            "DYLD_LIBRARY_PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        append_env_paths(command, key, library_dirs);
    }
}

fn configure_document_tool_environment(binary: &Path, command: &mut Command) {
    if cfg!(windows) {
        return;
    }

    let Some(root) = tool_root(binary) else {
        return;
    };
    let mut library_dirs = Vec::new();
    collect_existing_dirs(&root.join("lib"), 2, &mut library_dirs);
    collect_existing_dirs(&root.join("lib64"), 1, &mut library_dirs);
    append_env_paths(command, "LD_LIBRARY_PATH", library_dirs);
}

fn ghostscript_root(binary: &Path) -> Option<PathBuf> {
    tool_root(binary)
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

fn append_env_paths(command: &mut Command, key: &str, mut paths: Vec<PathBuf>) {
    if let Some(existing) = env::var_os(key) {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(joined) = env::join_paths(paths) {
        command.env(key, joined);
    }
}
