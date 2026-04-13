# Packaging

See [RELEASING.md](../RELEASING.md) for the full release process.

## Windows

Script:

```powershell
packaging\windows\build-installer.ps1
```

Output:

- `dist/windows/Compressi.ty/`
- `dist/windows/installer/Compressi.ty-Setup-<version>.exe`

Files:

| File | Purpose |
|---|---|
| `build-installer.ps1` | Orchestrates the full build, supports `bundled` and `no-engine` variants, reuses the cached FFmpeg runtime, and invokes Inno Setup |
| `compressi.ty.iss` | Inno Setup script with `WizardStyle=modern dark`, per-page background switching, and `assets/icon/icon.bmp` as the small wizard badge |
| `installer-bg-welcome.png` | Background artwork for the Welcome page |
| `installer-bg-license.png` | Background artwork for the License page |
| `installer-bg-select-dir.png` | Background artwork for the installation directory page |
| `installer-bg-select-tasks.png` | Background artwork for the additional tasks page |
| `installer-bg-ready.png` | Background artwork for the Ready to Install page |
| `installer-bg-installing.png` | Background artwork for the Preparing and Installing pages |
| `installer-bg-finished.png` | Background artwork for the completion page |

Notes:

- The final installer requires Inno Setup 6
- The bundle includes the application and FFmpeg
- Installer artwork is versioned in-repo and switched at runtime via Pascal script
- The Windows build reuses `dist/windows/engine-cache/` and only downloads FFmpeg again when `-RefreshEngine` is used

## Linux

Script:

```bash
packaging/linux/build-bundle.sh
```

Output:

- `dist/linux/Compressi.ty.AppDir/`
- `dist/linux/Compressi.ty-<version>-<arch>.tar.gz`
- `dist/linux/Compressi.ty-<version>-<arch>.AppImage`

Notes:

- Run from Linux, WSL, or a Linux CI environment
- Run the Linux build script as your normal user. Use `sudo` only for installing prerequisite packages.
- Linux bundle builds also need a C toolchain that provides `cc` (`sudo apt update && sudo apt install -y build-essential` on Ubuntu/Debian/WSL)
- The Linux build reuses `dist/linux/engine-cache/` and will redownload FFmpeg automatically if the cached archive is incomplete or corrupted
- `.AppImage` output is generated only when `appimagetool` is available
