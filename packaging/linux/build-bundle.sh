#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_ROOT="${REPO_ROOT}/dist/linux"
APPDIR="${DIST_ROOT}/Compressity.AppDir"
CACHE_DIR="${DIST_ROOT}/engine-cache"

if [[ "${1:-}" != "--skip-tests" ]]; then
  (cd "${REPO_ROOT}" && cargo test)
fi

(cd "${REPO_ROOT}" && cargo build --release)

APP_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "${REPO_ROOT}/Cargo.toml" | head -n 1)"
ARCH="$(uname -m)"

case "${ARCH}" in
  x86_64)
    ENGINE_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
    ;;
  aarch64|arm64)
    ENGINE_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"
    ;;
  *)
    echo "Unsupported Linux architecture: ${ARCH}" >&2
    exit 1
    ;;
esac

rm -rf "${APPDIR}"
mkdir -p \
  "${APPDIR}/usr/bin" \
  "${APPDIR}/usr/share/applications" \
  "${APPDIR}/usr/share/icons/hicolor/scalable/apps"

cp "${REPO_ROOT}/target/release/compressity" "${APPDIR}/usr/bin/compressity"
cp "${REPO_ROOT}/LICENSE" "${APPDIR}/LICENSE.txt"
cp "${SCRIPT_DIR}/AppRun" "${APPDIR}/AppRun"
cp "${SCRIPT_DIR}/compressity.desktop" "${APPDIR}/compressity.desktop"
cp "${SCRIPT_DIR}/compressity.desktop" "${APPDIR}/usr/share/applications/compressity.desktop"
cp "${REPO_ROOT}/assets/icon/icon.svg" "${APPDIR}/compressity.svg"
cp "${REPO_ROOT}/assets/icon/icon.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/compressity.svg"
chmod +x "${APPDIR}/AppRun" "${APPDIR}/usr/bin/compressity"

DOWNLOAD_ROOT="${CACHE_DIR}/download"
ARCHIVE_PATH="${DOWNLOAD_ROOT}/ffmpeg-static.tar.xz"

rm -rf "${DOWNLOAD_ROOT}"
mkdir -p "${DOWNLOAD_ROOT}"

echo "Downloading bundled FFmpeg for Linux..."
curl -L "${ENGINE_URL}" -o "${ARCHIVE_PATH}"
tar -xf "${ARCHIVE_PATH}" -C "${DOWNLOAD_ROOT}"

FFMPEG_BIN="$(find "${DOWNLOAD_ROOT}" -type f -name ffmpeg | head -n 1)"
FFPROBE_BIN="$(find "${DOWNLOAD_ROOT}" -type f -name ffprobe | head -n 1)"

if [[ -z "${FFMPEG_BIN}" || -z "${FFPROBE_BIN}" ]]; then
  echo "The downloaded FFmpeg archive did not contain ffmpeg and ffprobe binaries." >&2
  exit 1
fi

cp "${FFMPEG_BIN}" "${APPDIR}/usr/bin/ffmpeg"
cp "${FFPROBE_BIN}" "${APPDIR}/usr/bin/ffprobe"
chmod +x "${APPDIR}/usr/bin/ffmpeg" "${APPDIR}/usr/bin/ffprobe"

TARBALL="${DIST_ROOT}/Compressity-${APP_VERSION}-${ARCH}.tar.gz"
rm -f "${TARBALL}"
tar -czf "${TARBALL}" -C "${DIST_ROOT}" "Compressity.AppDir"

if command -v appimagetool >/dev/null 2>&1; then
  APPIMAGE="${DIST_ROOT}/Compressity-${APP_VERSION}-${ARCH}.AppImage"
  rm -f "${APPIMAGE}"
  appimagetool "${APPDIR}" "${APPIMAGE}"
  echo "Linux AppImage created at ${APPIMAGE}"
else
  echo "appimagetool not found; created portable bundle at ${TARBALL}"
fi
