use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::runtime;

pub(super) fn run_tool(binary: &Path, args: &[OsString], label: &str) -> Result<(), String> {
    let mut command = Command::new(binary);
    command.args(args);
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
    if cfg!(windows) {
        discover_binary(&["gswin64c.exe", "gswin32c.exe", "gs.exe", "gs"])
    } else {
        discover_binary(&["gs"])
    }
}

pub(super) fn discover_qpdf_binary() -> Option<PathBuf> {
    if cfg!(windows) {
        discover_binary(&["qpdf.exe", "qpdf"])
    } else {
        discover_binary(&["qpdf"])
    }
}

fn discover_binary(names: &[&str]) -> Option<PathBuf> {
    for (dir, depth) in document_engine_dirs() {
        if let Some(binary) = find_binary_in_dir(&dir, names, depth) {
            return Some(binary);
        }
    }

    system_binary_candidates(names)
        .into_iter()
        .find(|path| path.is_file())
}

fn document_engine_dirs() -> Vec<(PathBuf, usize)> {
    let mut dirs = Vec::new();
    for dir in [
        runtime::managed_document_engine_dir(),
        runtime::bundled_document_engine_dir(),
    ]
    .into_iter()
    .flatten()
    {
        dirs.push((dir.join("bin"), 2));
        dirs.push((dir, 4));
    }

    if let Some(exe_dir) = runtime::current_exe_dir() {
        dirs.push((exe_dir.join("bin"), 0));
        dirs.push((exe_dir, 0));
    }
    dirs
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

fn ghostscript_root(binary: &Path) -> Option<PathBuf> {
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
