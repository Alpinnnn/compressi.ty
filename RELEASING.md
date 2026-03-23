# Release Guide

## Update versi

1. Ubah `version = "..."` di [Cargo.toml](Cargo.toml).
2. Commit perubahan rilis.

## Output

- Windows staging bundle: `dist/windows/Compressity/`
- Windows installer: `dist/windows/installer/Compressity-Setup-<version>.exe`
- Linux AppDir: `dist/linux/Compressity.AppDir/`
- Linux tarball: `dist/linux/Compressity-<version>-<arch>.tar.gz`
- Linux AppImage: `dist/linux/Compressity-<version>-<arch>.AppImage`

## Windows

Prasyarat:

- Rust toolchain
- Internet untuk download FFmpeg bundle
- Inno Setup 6 untuk membuat installer `.exe`

Command:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1
```

Opsi:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -SkipTests
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -RefreshEngine
```

Hasil:

- Selalu membuat staging bundle di `dist/windows/Compressity/`
- Membuat installer di `dist/windows/installer/` jika Inno Setup tersedia

## Linux

Jalankan dari Linux, WSL, atau CI Linux.

Prasyarat:

- Rust toolchain
- `bash`, `curl`, `tar`
- Internet untuk download FFmpeg bundle
- `appimagetool` jika ingin output `.AppImage`

Persiapan:

```bash
chmod +x packaging/linux/build-bundle.sh
chmod +x packaging/linux/AppRun
```

Command:

```bash
./packaging/linux/build-bundle.sh
```

Opsi:

```bash
./packaging/linux/build-bundle.sh --skip-tests
```

Hasil:

- Membuat AppDir di `dist/linux/Compressity.AppDir/`
- Membuat tarball di `dist/linux/`
- Membuat AppImage di `dist/linux/` jika `appimagetool` tersedia

## Engine bundle

- Script packaging menyertakan FFmpeg bundle
- Gunakan `-RefreshEngine` pada build Windows untuk mengambil bundle terbaru
- Setelah instalasi, versi engine tetap bisa dicek dan diupdate dari Settings
