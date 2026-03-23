# Packaging

Panduan lengkap ada di [RELEASING.md](../RELEASING.md).

## Windows

Script:

```powershell
packaging\windows\build-installer.ps1
```

Output:

- `dist/windows/Compressity/`
- `dist/windows/installer/Compressity-Setup-<version>.exe`

Catatan:

- Installer final membutuhkan Inno Setup 6
- Bundle berisi aplikasi dan FFmpeg

## Linux

Script:

```bash
packaging/linux/build-bundle.sh
```

Output:

- `dist/linux/Compressity.AppDir/`
- `dist/linux/Compressity-<version>-<arch>.tar.gz`
- `dist/linux/Compressity-<version>-<arch>.AppImage`

Catatan:

- Jalankan dari Linux, WSL, atau CI Linux
- Output `.AppImage` dibuat hanya jika `appimagetool` tersedia
