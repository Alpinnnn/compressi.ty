# Release Guide

## Version Update

1. Update `version = "..."` in [Cargo.toml](Cargo.toml).
2. Commit the release changes.

## Output

- Windows staging bundle: `dist/windows/Compressity/`
- Windows installer: `dist/windows/installer/Compressity-Setup-<version>.exe`
- Linux AppDir: `dist/linux/Compressity.AppDir/`
- Linux tarball: `dist/linux/Compressity-<version>-<arch>.tar.gz`
- Linux AppImage: `dist/linux/Compressity-<version>-<arch>.AppImage`

## Windows

Requirements:

- Rust toolchain
- Internet access to download the bundled FFmpeg runtime
- Inno Setup 6 to generate the final `.exe` installer

Command:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1
```

Options:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -SkipTests
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -RefreshEngine
```

Result:

- Always creates a staging bundle in `dist/windows/Compressity/`
- Creates the installer in `dist/windows/installer/` when Inno Setup is available

## Linux

Run from Linux, WSL, or a Linux CI environment.

Requirements:

- Rust toolchain
- `bash`, `curl`, `tar`
- Internet access to download the bundled FFmpeg runtime
- `appimagetool` if `.AppImage` output is required

Preparation:

```bash
chmod +x packaging/linux/build-bundle.sh
chmod +x packaging/linux/AppRun
```

Command:

```bash
./packaging/linux/build-bundle.sh
```

Options:

```bash
./packaging/linux/build-bundle.sh --skip-tests
```

Result:

- Creates the AppDir in `dist/linux/Compressity.AppDir/`
- Creates the tarball in `dist/linux/`
- Creates the AppImage in `dist/linux/` when `appimagetool` is available

## Engine Bundle

- The packaging scripts include a bundled FFmpeg runtime
- Use `-RefreshEngine` on the Windows build to fetch the latest bundled runtime
- After installation, engine versions can still be checked and updated from Settings
