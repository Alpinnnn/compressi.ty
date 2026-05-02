use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::modules::compress_documents::{engine, models::DocumentKind};

pub(super) fn supports_external_repack(kind: DocumentKind) -> bool {
    matches!(
        kind,
        DocumentKind::MicrosoftOpenXml | DocumentKind::OpenPackaging
    )
}

pub(super) fn repack_zip_with_7zip(
    input_path: &Path,
    output_path: &Path,
    compression_level: i64,
) -> Result<(), String> {
    let seven_zip = engine::discover_seven_zip_binary()
        .ok_or_else(|| "7-Zip package engine is not available.".to_owned())?;
    let temp_dir = create_temp_dir(output_path)?;
    let result = repack_zip_inner(
        &seven_zip,
        input_path,
        output_path,
        compression_level,
        &temp_dir,
    );
    let _ = fs::remove_dir_all(&temp_dir);
    result
}

fn repack_zip_inner(
    seven_zip: &Path,
    input_path: &Path,
    output_path: &Path,
    compression_level: i64,
    temp_dir: &Path,
) -> Result<(), String> {
    let mut extract = background_command(seven_zip);
    extract
        .arg("x")
        .arg(input_path)
        .arg(format!("-o{}", temp_dir.display()))
        .arg("-y")
        .arg("-bd")
        .arg("-bb0");
    run_command(extract, "7-Zip package extraction")?;

    if output_path.exists() {
        fs::remove_file(output_path)
            .map_err(|error| format!("Could not replace {}: {error}", output_path.display()))?;
    }

    let mut archive = background_command(seven_zip);
    archive
        .current_dir(temp_dir)
        .arg("a")
        .arg("-tzip")
        .arg("-mm=Deflate")
        .arg(format!("-mx={}", compression_level.clamp(0, 9)))
        .arg("-mfb=258")
        .arg("-mpass=15")
        .arg("-mcu=on")
        .arg("-r")
        .arg(output_path)
        .arg(".")
        .arg("-y")
        .arg("-bd")
        .arg("-bb0");
    run_command(archive, "7-Zip package compression")
}

fn create_temp_dir(output_path: &Path) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Clock error: {error}"))?
        .as_millis();
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("package");
    let temp_dir = std::env::temp_dir()
        .join("compressi.ty")
        .join("package-engine")
        .join(format!("{stem}-{timestamp}"));
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Could not create {}: {error}", temp_dir.display()))?;
    Ok(temp_dir)
}

fn background_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn run_command(mut command: Command, label: &str) -> Result<(), String> {
    let output = crate::process_lifecycle::output(&mut command)
        .map_err(|error| format!("{label} could not start: {error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = stderr
        .lines()
        .chain(stdout.lines())
        .find(|line| !line.trim().is_empty())
        .unwrap_or("7-Zip exited unexpectedly.");

    Err(format!("{label} failed: {detail}"))
}
