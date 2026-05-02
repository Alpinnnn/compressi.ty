use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::mpsc,
};

use zip::ZipArchive;

use crate::runtime;

use super::{
    inventory::WorkerEvent,
    process_utils::{background_command, run_status},
};

const GHOSTSCRIPT_TAG: &str = "gs10070";
const QPDF_VERSION: &str = "12.3.2";
const SEVEN_ZIP_TAG: &str = "26.01";

pub(super) fn install_latest_managed_document_engines(
    sender: &mpsc::Sender<WorkerEvent>,
) -> Result<(), String> {
    let pdf_dir = runtime::managed_pdf_engine_dir()
        .ok_or_else(|| "Could not resolve the managed PDF engine directory.".to_owned())?;
    let package_dir = runtime::managed_package_engine_dir()
        .ok_or_else(|| "Could not resolve the managed package engine directory.".to_owned())?;

    reset_dir(&pdf_dir)?;
    reset_dir(&package_dir)?;

    let mut errors = Vec::new();
    if let Err(error) = install_ghostscript(&pdf_dir, sender) {
        errors.push(error);
    }
    if let Err(error) = install_qpdf(&pdf_dir, sender) {
        errors.push(error);
    }
    if let Err(error) = install_seven_zip(&package_dir, sender) {
        errors.push(error);
    }

    if errors.is_empty() {
        let _ = sender.send(WorkerEvent::Progress {
            progress: 1.0,
            stage: "Document engines ready".to_owned(),
        });
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn install_ghostscript(
    destination: &Path,
    sender: &mpsc::Sender<WorkerEvent>,
) -> Result<(), String> {
    let url = ghostscript_download_url()?;
    let downloads = destination.join("downloads");
    fs::create_dir_all(&downloads).map_err(|error| {
        format!(
            "Could not create Ghostscript download folder {}: {error}",
            downloads.display()
        )
    })?;
    let archive_path = downloads.join(ghostscript_package_name()?);

    download_to_file(
        &url,
        &archive_path,
        sender,
        0.03,
        0.24,
        "Downloading Ghostscript",
    )?;

    let _ = sender.send(WorkerEvent::Progress {
        progress: 0.28,
        stage: "Installing Ghostscript".to_owned(),
    });

    #[cfg(target_os = "windows")]
    {
        run_silent_installer(&archive_path, destination, "Ghostscript installer")?;
    }

    #[cfg(target_os = "linux")]
    {
        extract_ghostscript_snap(&archive_path, destination)?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = archive_path;
        return Err("Managed Ghostscript downloads are not configured for this OS.".to_owned());
    }

    Ok(())
}

fn install_qpdf(destination: &Path, sender: &mpsc::Sender<WorkerEvent>) -> Result<(), String> {
    let url = qpdf_download_url()?;
    let downloads = destination.join("downloads");
    fs::create_dir_all(&downloads).map_err(|error| {
        format!(
            "Could not create qpdf download folder {}: {error}",
            downloads.display()
        )
    })?;
    let archive_path = downloads.join(qpdf_package_name()?);
    download_to_file(&url, &archive_path, sender, 0.34, 0.18, "Downloading qpdf")?;

    let qpdf_dir = destination.join("qpdf");
    reset_dir(&qpdf_dir)?;
    let _ = sender.send(WorkerEvent::Progress {
        progress: 0.54,
        stage: "Installing qpdf".to_owned(),
    });
    extract_zip_archive(&archive_path, &qpdf_dir)?;
    Ok(())
}

fn install_seven_zip(destination: &Path, sender: &mpsc::Sender<WorkerEvent>) -> Result<(), String> {
    let url = seven_zip_download_url()?;
    let downloads = destination.join("downloads");
    fs::create_dir_all(&downloads).map_err(|error| {
        format!(
            "Could not create 7-Zip download folder {}: {error}",
            downloads.display()
        )
    })?;
    let archive_path = downloads.join(seven_zip_package_name()?);
    download_to_file(&url, &archive_path, sender, 0.62, 0.22, "Downloading 7-Zip")?;

    let _ = sender.send(WorkerEvent::Progress {
        progress: 0.88,
        stage: "Installing 7-Zip".to_owned(),
    });

    #[cfg(target_os = "windows")]
    {
        run_silent_installer(&archive_path, destination, "7-Zip installer")?;
    }

    #[cfg(target_os = "linux")]
    {
        extract_tar_archive(&archive_path, destination, "7-Zip archive")?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = archive_path;
        return Err("Managed 7-Zip downloads are not configured for this OS.".to_owned());
    }

    Ok(())
}

fn download_to_file(
    url: &str,
    output_path: &Path,
    sender: &mpsc::Sender<WorkerEvent>,
    base_progress: f32,
    progress_span: f32,
    label: &str,
) -> Result<(), String> {
    let _ = sender.send(WorkerEvent::Progress {
        progress: base_progress,
        stage: label.to_owned(),
    });

    let mut response = ureq::get(url)
        .call()
        .map_err(|error| format!("{label} failed from {url}: {error}"))?;
    let total_bytes = response.body().content_length().unwrap_or(0);
    let mut reader = response.body_mut().as_reader();
    let mut file = File::create(output_path)
        .map_err(|error| format!("Could not create {}: {error}", output_path.display()))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut downloaded = 0_u64;

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("{label} stream failed: {error}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("Could not write {}: {error}", output_path.display()))?;
        downloaded = downloaded.saturating_add(read as u64);
        let fraction = if total_bytes == 0 {
            0.5
        } else {
            (downloaded as f32 / total_bytes as f32).clamp(0.0, 1.0)
        };
        let _ = sender.send(WorkerEvent::Progress {
            progress: (base_progress + progress_span * fraction).clamp(0.0, 0.99),
            stage: format!("{label} ({})", format_bytes(downloaded)),
        });
    }

    Ok(())
}

fn extract_zip_archive(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let archive_file = File::open(archive_path)
        .map_err(|error| format!("Could not open {}: {error}", archive_path.display()))?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| format!("Could not read {}: {error}", archive_path.display()))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not read zip entry {index}: {error}"))?;
        let enclosed_name = entry
            .enclosed_name()
            .ok_or_else(|| format!("Archive contains an unsafe path: {}", entry.name()))?;
        let output_path = destination.join(enclosed_name);
        if entry.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("Could not create {}: {error}", output_path.display()))?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
            }
            let mut output = File::create(&output_path).map_err(|error| {
                format!(
                    "Could not create extracted file {}: {error}",
                    output_path.display()
                )
            })?;
            io::copy(&mut entry, &mut output)
                .map_err(|error| format!("Could not extract {}: {error}", output_path.display()))?;
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&output_path, fs::Permissions::from_mode(mode)).map_err(
                    |error| {
                        format!(
                            "Could not set permissions on {}: {error}",
                            output_path.display()
                        )
                    },
                )?;
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn run_silent_installer(
    installer_path: &Path,
    destination: &Path,
    label: &str,
) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "Could not create installer destination {}: {error}",
            destination.display()
        )
    })?;
    let mut command = background_command(installer_path);
    command
        .arg("/S")
        .arg(format!("/D={}", destination.display()));
    run_status(command, label)
}

#[cfg(target_os = "linux")]
fn extract_ghostscript_snap(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let extract_dir = destination.join("ghostscript-archive");
    let snap_root = destination.join("ghostscript");
    reset_dir(&extract_dir)?;
    reset_dir(&snap_root)?;
    extract_tar_archive(archive_path, &extract_dir, "Ghostscript archive")?;

    let snap_path = find_file_with_extension(&extract_dir, "snap")
        .ok_or_else(|| "Ghostscript archive did not contain a snap package.".to_owned())?;
    let mut command = background_command(Path::new("unsquashfs"));
    command.arg("-f").arg("-d").arg(&snap_root).arg(&snap_path);
    run_status(command, "Ghostscript snap extraction")?;
    let _ = fs::remove_dir_all(&extract_dir);
    Ok(())
}

#[cfg(target_os = "linux")]
fn extract_tar_archive(archive_path: &Path, destination: &Path, label: &str) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("Could not create {}: {error}", destination.display()))?;
    let mut command = background_command(Path::new("tar"));
    command
        .arg("-xf")
        .arg(archive_path)
        .arg("-C")
        .arg(destination);
    run_status(command, label)
}

fn reset_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Could not clear {}: {error}", path.display()))?;
    }
    fs::create_dir_all(path)
        .map_err(|error| format!("Could not create {}: {error}", path.display()))
}

#[cfg(target_os = "linux")]
fn find_file_with_extension(root: &Path, extension: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_with_extension(&path, extension) {
                return Some(found);
            }
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case(extension))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

fn ghostscript_download_url() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        return Ok(format!(
            "https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/{GHOSTSCRIPT_TAG}/{}",
            ghostscript_package_name()?
        ));
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok(format!(
            "https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/{GHOSTSCRIPT_TAG}/{}",
            ghostscript_package_name()?
        ));
    }

    #[allow(unreachable_code)]
    Err("Managed Ghostscript download is not configured for this platform.".to_owned())
}

fn ghostscript_package_name() -> Result<&'static str, String> {
    #[cfg(target_os = "windows")]
    {
        return Ok("gs10070w64.exe");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("gs_10.07.0_amd64_snap.tgz");
    }

    #[allow(unreachable_code)]
    Err("No Ghostscript package name is configured for this platform.".to_owned())
}

fn qpdf_download_url() -> Result<String, String> {
    Ok(format!(
        "https://sourceforge.net/projects/qpdf/files/qpdf/{QPDF_VERSION}/{}/download",
        qpdf_package_name()?
    ))
}

fn qpdf_package_name() -> Result<&'static str, String> {
    #[cfg(target_os = "windows")]
    {
        return Ok("qpdf-12.3.2-mingw64.zip");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("qpdf-12.3.2-bin-linux-x86_64.zip");
    }

    #[allow(unreachable_code)]
    Err("Managed qpdf download is not configured for this platform.".to_owned())
}

fn seven_zip_download_url() -> Result<String, String> {
    Ok(format!(
        "https://github.com/ip7z/7zip/releases/download/{SEVEN_ZIP_TAG}/{}",
        seven_zip_package_name()?
    ))
}

fn seven_zip_package_name() -> Result<&'static str, String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Ok("7z2601-x64.exe");
    }

    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        return Ok("7z2601-arm64.exe");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("7z2601-linux-x64.tar.xz");
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Ok("7z2601-linux-arm64.tar.xz");
    }

    #[allow(unreachable_code)]
    Err("Managed 7-Zip download is not configured for this platform.".to_owned())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_index = 0usize;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}
