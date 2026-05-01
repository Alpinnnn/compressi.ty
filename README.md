<p align="center">
  <img src="assets/icon/icon.svg" alt="Compressi.ty logo" width="96">
</p>

<h1 align="center">Compressi.ty</h1>

<p align="center">
  Local-first desktop compression toolkit for photos, videos, audio, and documents.
</p>

Compressi.ty is a native desktop application built with Rust and `eframe/egui`. It is designed around on-device processing, modular feature workspaces, and a packaging flow that can ship the video runtime with the app instead of requiring users to install FFmpeg manually.

## Why Compressi.ty

- Local-first by default. Compression runs on the user's machine with no cloud dependency.
- Native desktop UI. The application ships as a Rust desktop app with custom theming, fonts, and branding assets.
- Real workflows available today. The repository already includes working photo, video, audio, and document compression modules.
- Built to expand. Additional workspaces for file compression, folder compression, and archive/extract already have routed UI shells in place.
- Distribution ready. Windows and Linux packaging scripts produce portable bundles and installer-ready outputs.

## Project Status

| Workspace | Status | Notes |
| --- | --- | --- |
| Compress Photos | Available | Batch image compression, presets, advanced controls, output management, preview workspace |
| Compress Videos | Available | Batch video compression, metadata probing, thumbnails, live estimates, FFmpeg runtime management |
| Compress Documents | Available | Batch PDF and ZIP-package document compression with single-item and bulk workflows |
| Settings | Available | Default output folder, engine inventory, managed FFmpeg updates |
| Compress Files | Planned shell | Menu entry and routed shell exist, workflow not implemented yet |
| Compress Folder | Planned shell | Menu entry and routed shell exist, workflow not implemented yet |
| Archive / Extract | Planned shell | Menu entry and routed shell exist, workflow not implemented yet |

## Feature Overview

### Compress Photos

- Supported input formats: `JPG`, `JPEG`, `PNG`, `WebP`, `AVIF`
- Batch queue with drag-and-drop and file picker support
- Presets: `Maximum Quality`, `Balanced`, `High Compression`, `Ultra Compression`
- Advanced controls for quality, resize percentage, metadata stripping, and format conversion
- Output options: keep original format or convert to `JPEG`, `WebP`, or `AVIF`
- Before/after preview workspace with zoom, pan, and draggable split comparison
- Background processing with per-file progress reporting and cancellation
- Auto-generated run folders under `compressi.ty-output/photos/` when no custom destination is selected

### Compress Videos

- Supported input formats: `MP4`, `MOV`, `MKV`, `WEBM`, `AVI`, `M4V`
- Queue-based workflow with drag-and-drop, per-item probing, and thumbnail extraction
- Compression modes:
  - `Reduce Size`: target-size workflow with adaptive recommendations and two-pass encoding
  - `Good Quality`: quality-first workflow using codec-aware estimation
  - `Custom (Advanced)`: manual bitrate, codec, resolution, and FPS control
- Codec detection for `H.264`, `H.265/HEVC`, and `AV1`, with automatic fallback if an encoder is unavailable
- Sequential batch compression with live progress, ETA, and output summaries
- `MP4` output container with audio preserved when present

### Compress Documents

- Supported input formats: `PDF`, `DOCX`, `DOCM`, `DOTX`, `DOTM`, `XLSX`, `XLSM`, `XLTX`, `XLTM`, `XLAM`, `PPTX`, `PPTM`, `POTX`, `POTM`, `PPSX`, `PPSM`, `PPAM`, `SLDX`, `SLDM`, `ODT`, `OTT`, `OTH`, `ODM`, `ODS`, `OTS`, `ODP`, `OTP`, `ODG`, `OTG`, `ODF`, `ODC`, `ODI`, `ODB`, `EPUB`, `XPS`, `OXPS`, `VSDX`, `VSDM`, `VSSTX`, `VSSTM`, `VSSX`, `VSSM`, `VSTX`, `VSTM`
- Drag-and-drop and file picker queue with per-row single compression or full batch compression
- Presets: `Maximum Compatibility`, `Balanced`, `High Compression`, `Ultra Compression`
- PDF optimization uses Rust-native `lopdf` stream compression and optional object/xref streams
- Office, OpenDocument, EPUB, XPS, and Visio files use ZIP deflate repacking through `zip-rs`
- OpenDocument and EPUB packages preserve the required uncompressed first `mimetype` entry
- Engine discovery order: managed update, bundled runtime, then system `PATH`

## Architecture

Compressi.ty is organized around a shared application shell plus feature-specific modules.

```text
src/
  main.rs                      Native app bootstrap
  app.rs                       Global state, routing, dialogs, and module orchestration
  theme.rs                     Typography, palette, and egui styling
  branding.rs                  SVG app icon loading for window and in-app surfaces
  runtime.rs                   Config, data, engine, and output directory helpers
  settings.rs                  Persistent JSON application settings
  modules/
    compress_photos/           Image models, compressor, and egui workspace
    compress_documents/        Document models, processor, and egui workspace
    compress_videos/           FFmpeg engine, processor, models, and egui workspace
  ui/                          Main menu, settings screen, and placeholder module shell
packaging/
  windows/                     Installer staging and Inno Setup workflow
  linux/                       AppDir/AppImage bundling workflow
assets/
  icon/                        Application icon assets
  fonts/                       Google Sans and Ionicons
```

### Module Contract

The repository already follows a clear split that is easy to extend:

- `models.rs` contains user-facing state and domain types
- `compressor.rs`, `processor.rs`, and `engine.rs` contain heavy logic and background work
- `ui.rs` owns egui rendering, layout, and interaction mapping

## Runtime Behavior

- Global settings are persisted as `compressi.ty/settings.json` inside the user's config directory
- If no default output folder is configured, Compressi.ty resolves an output root in this order:
  `Downloads -> Pictures -> Documents -> Home -> temp fallback`
- Default generated output folders:
  - Photos: `compressi.ty-output/photos/run-<timestamp>/`
  - Documents: `compressi.ty-output/documents/run-<timestamp>/`
  - Videos: `compressi.ty-output/videos/run-<timestamp>/`
- Managed FFmpeg updates are stored in local app data so installed application folders can remain read-only

## Getting Started

### Prerequisites

- Rust toolchain
- Internet access if you plan to use video compression from source on a machine without an existing FFmpeg runtime
- Optional release prerequisites:
  - Windows installer builds: Inno Setup 6
  - Linux bundles: `bash`, `curl`, `tar`, and a C toolchain that provides `cc` (for Ubuntu/Debian/WSL: `sudo apt install build-essential`), optional `appimagetool`

### Run From Source

```bash
cargo run
```

### Test

```bash
cargo test
```

### First-Run Note for Video Compression

The video workspace can use any of the following FFmpeg sources:

1. Managed FFmpeg stored in app data
2. Bundled FFmpeg shipped with the app
3. System FFmpeg available on `PATH`

If no engine is available, the application attempts to prepare a managed FFmpeg runtime automatically.

## Packaging and Release

The detailed release process lives in [RELEASING.md](RELEASING.md). The commands below cover the main workflow.

### Windows

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1
```

Optional flags:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -SkipTests
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -RefreshEngine
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -Variant no-engine
powershell -ExecutionPolicy Bypass -File packaging\windows\build-installer.ps1 -Variant all
```

Outputs:

- `dist/windows/Compressi.ty/` for the default bundled variant
- `dist/windows/Compressi.ty-no-engine/` for the no-engine variant
- `dist/windows/Compressi.ty-bundled/` as the bundled staging folder when `-Variant all` is used
- `dist/windows/installer/Compressi.ty-Setup-<version>.exe` for the default bundled installer
- `dist/windows/installer/Compressi.ty-Setup-<version>-NoEngine.exe` for the no-engine installer
- `dist/windows/installer/Compressi.ty-Setup-<version>-Bundled.exe` when `-Variant all` is used

### Linux

```bash
sudo apt update && sudo apt install -y build-essential
chmod +x packaging/linux/build-bundle.sh
chmod +x packaging/linux/AppRun
./packaging/linux/build-bundle.sh
```

Optional flag:

```bash
./packaging/linux/build-bundle.sh --skip-tests
```

Outputs:

- `dist/linux/Compressi.ty.AppDir/`
- `dist/linux/Compressi.ty-<version>-<arch>.tar.gz`
- `dist/linux/Compressi.ty-<version>-<arch>.AppImage` when `appimagetool` is available

## Repository Docs

- [AI_PLAYBOOK.md](AI_PLAYBOOK.md): architectural rules and contributor guidance for expanding the module system
- [RELEASING.md](RELEASING.md): release process and artifact expectations
- [packaging/README.md](packaging/README.md): packaging overview
- [IONICONS-CHEATSHEET.md](IONICONS-CHEATSHEET.md): verified Ionicons mappings used by the UI

## Development Notes

- Current automated tests are concentrated in the video processing module, especially parser and estimation logic
- Windows and Linux packaging are implemented in-repo
- macOS-specific packaging is not documented in the current repository

## License

This project is distributed under the `Compressi.ty Personal Source-Share Attribution NonCommercial License 1.0`. See [LICENSE](LICENSE).
