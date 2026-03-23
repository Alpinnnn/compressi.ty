# Packaging

See [RELEASING.md](../RELEASING.md) for the full release process.

## Windows

Script:

```powershell
packaging\windows\build-installer.ps1
```

Output:

- `dist/windows/Compressity/`
- `dist/windows/installer/Compressity-Setup-<version>.exe`

Files:

| File | Purpose |
|---|---|
| `build-installer.ps1` | Orchestrates the full build: compile, stage, bundle FFmpeg, and invoke Inno Setup |
| `compressity.iss` | Inno Setup script with `WizardStyle=modern dark`, per-page background switching, and `assets/icon/icon.bmp` as the small wizard badge |
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

## Linux

Script:

```bash
packaging/linux/build-bundle.sh
```

Output:

- `dist/linux/Compressity.AppDir/`
- `dist/linux/Compressity-<version>-<arch>.tar.gz`
- `dist/linux/Compressity-<version>-<arch>.AppImage`

Notes:

- Run from Linux, WSL, or a Linux CI environment
- `.AppImage` output is generated only when `appimagetool` is available
