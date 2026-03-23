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

Notes:

- The final installer requires Inno Setup 6
- The bundle includes the application and FFmpeg

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
