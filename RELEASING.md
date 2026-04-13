# Release Guide

## Version Update

1. Update `version = "..."` in [Cargo.toml](Cargo.toml).
2. Commit the release changes.

## Output

- Windows staging bundle: `dist/windows/Compressi.ty/`
- Windows installer: `dist/windows/installer/Compressi.ty-Setup-<version>.exe`
- Linux AppDir: `dist/linux/Compressi.ty.AppDir/`
- Linux tarball: `dist/linux/Compressi.ty-<version>-<arch>.tar.gz`
- Linux AppImage: `dist/linux/Compressi.ty-<version>-<arch>.AppImage`

## Windows

Requirements:

- Rust toolchain
- Internet access only when the bundled FFmpeg cache is missing or when `-RefreshEngine` is used
- Inno Setup 6 to generate the final `.exe` installer

Command:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1
```

Options:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -SkipTests
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -RefreshEngine
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -Variant no-engine
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -Variant all
```

Result:

- `-Variant bundled` creates `dist/windows/Compressi.ty/` and the default installer
- `-Variant no-engine` creates `dist/windows/Compressi.ty-no-engine/` and a `NoEngine` installer
- `-Variant all` creates both staged bundles and both installer variants
- When Inno Setup is available, the generated installers are written to `dist/windows/installer/`

## Linux

Run from Linux, WSL, or a Linux CI environment.

Requirements:

- Rust toolchain
- `bash`, `curl`, `tar`
- A C toolchain that provides `cc` (`sudo apt update && sudo apt install -y build-essential` on Ubuntu/Debian/WSL)
- Internet access to download the bundled FFmpeg runtime
- `appimagetool` if `.AppImage` output is required

Preparation:

```bash
sudo apt update && sudo apt install -y build-essential
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

- Creates the AppDir in `dist/linux/Compressi.ty.AppDir/`
- Creates the tarball in `dist/linux/`
- Creates the AppImage in `dist/linux/` when `appimagetool` is available

## Engine Bundle

- The packaging scripts include a bundled FFmpeg runtime
- The Windows build now reuses the cached FFmpeg archive in `dist/windows/engine-cache/`
- Use `-RefreshEngine` on the Windows build to fetch the latest bundled runtime
- The `no-engine` variant skips copying FFmpeg into the package entirely
- After installation, engine versions can still be checked and updated from Settings
