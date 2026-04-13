#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_ROOT="${REPO_ROOT}/dist/linux"
APPDIR="${DIST_ROOT}/Compressi.ty.AppDir"
CACHE_DIR="${DIST_ROOT}/engine-cache"

reexec_as_invoking_user() {
  if [[ "${EUID}" -ne 0 || -z "${SUDO_USER:-}" || -n "${COMPRESSITY_BUILD_LINUX_REEXEC:-}" ]]; then
    return
  fi

  local sudo_home
  sudo_home="$(getent passwd "${SUDO_USER}" | cut -d: -f6)"

  if [[ -z "${sudo_home}" ]]; then
    echo "Could not resolve the home directory for sudo user '${SUDO_USER}'." >&2
    exit 1
  fi

  echo "Re-running the Linux bundle build as '${SUDO_USER}' so cargo stays available and build artifacts remain user-owned."
  exec sudo -H -u "${SUDO_USER}" env \
    "PATH=${sudo_home}/.cargo/bin:${PATH}" \
    "COMPRESSITY_BUILD_LINUX_REEXEC=1" \
    bash "$0" "$@"
}

require_cmd() {
  local cmd="$1"
  local help_text="$2"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing required command: ${cmd}. ${help_text}" >&2
    exit 1
  fi
}

download_engine_archive() {
  local archive_path="$1"
  local engine_url="$2"

  rm -f "${archive_path}"
  echo "Downloading bundled FFmpeg for Linux..."
  curl -fL --retry 3 --retry-delay 2 "${engine_url}" -o "${archive_path}"
}

clear_extracted_engine_cache() {
  local download_root="$1"
  local archive_path="$2"

  find "${download_root}" -mindepth 1 ! -path "${archive_path}" -exec rm -rf -- {} +
}

extract_engine_archive() {
  local download_root="$1"
  local archive_path="$2"

  clear_extracted_engine_cache "${download_root}" "${archive_path}"
  echo "Extracting cached FFmpeg runtime..."
  tar -xf "${archive_path}" -C "${download_root}"
}

ensure_engine_cache() {
  local download_root="$1"
  local archive_path="$2"
  local engine_url="$3"

  mkdir -p "${download_root}"

  local ffmpeg_bin
  local ffprobe_bin
  ffmpeg_bin="$(find "${download_root}" -type f -name ffmpeg | head -n 1)"
  ffprobe_bin="$(find "${download_root}" -type f -name ffprobe | head -n 1)"

  if [[ -z "${archive_path}" || -z "${engine_url}" ]]; then
    echo "Engine cache configuration is incomplete." >&2
    exit 1
  fi

  if [[ ! -f "${archive_path}" ]]; then
    download_engine_archive "${archive_path}" "${engine_url}"
  fi

  if [[ -z "${ffmpeg_bin}" || -z "${ffprobe_bin}" ]]; then
    if ! extract_engine_archive "${download_root}" "${archive_path}"; then
      echo "Cached FFmpeg archive is incomplete or corrupted. Redownloading..."
      download_engine_archive "${archive_path}" "${engine_url}"
      extract_engine_archive "${download_root}" "${archive_path}"
    fi

    ffmpeg_bin="$(find "${download_root}" -type f -name ffmpeg | head -n 1)"
    ffprobe_bin="$(find "${download_root}" -type f -name ffprobe | head -n 1)"
  fi

  if [[ -z "${ffmpeg_bin}" || -z "${ffprobe_bin}" ]]; then
    echo "The cached FFmpeg archive did not contain ffmpeg and ffprobe binaries." >&2
    exit 1
  fi

  ENGINE_FFMPEG_BIN="${ffmpeg_bin}"
  ENGINE_FFPROBE_BIN="${ffprobe_bin}"
}

reexec_as_invoking_user "$@"

require_cmd cargo "Install the Rust toolchain before building the Linux bundle."
require_cmd cc "Install a C toolchain first. On Ubuntu, Debian, or WSL this is usually: sudo apt update && sudo apt install -y build-essential"
require_cmd curl "Install curl to download the bundled FFmpeg runtime."
require_cmd tar "Install tar to unpack the bundled FFmpeg runtime."

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
  "${APPDIR}/usr/share/metainfo" \
  "${APPDIR}/usr/share/icons/hicolor/scalable/apps"

cp "${REPO_ROOT}/target/release/compressity" "${APPDIR}/usr/bin/compressity"
cp "${REPO_ROOT}/LICENSE" "${APPDIR}/LICENSE.txt"
cp "${SCRIPT_DIR}/AppRun" "${APPDIR}/AppRun"
cp "${SCRIPT_DIR}/io.github.Alpinnnn.Compressity.desktop" "${APPDIR}/io.github.Alpinnnn.Compressity.desktop"
cp "${SCRIPT_DIR}/io.github.Alpinnnn.Compressity.desktop" "${APPDIR}/usr/share/applications/io.github.Alpinnnn.Compressity.desktop"
cp "${SCRIPT_DIR}/io.github.Alpinnnn.Compressity.appdata.xml" "${APPDIR}/usr/share/metainfo/io.github.Alpinnnn.Compressity.appdata.xml"
cp "${REPO_ROOT}/assets/icon/icon.svg" "${APPDIR}/compressi.ty.svg"
cp "${REPO_ROOT}/assets/icon/icon.svg" "${APPDIR}/usr/share/icons/hicolor/scalable/apps/compressi.ty.svg"
chmod +x "${APPDIR}/AppRun" "${APPDIR}/usr/bin/compressity"

DOWNLOAD_ROOT="${CACHE_DIR}/download"
ARCHIVE_PATH="${DOWNLOAD_ROOT}/ffmpeg-static.tar.xz"
ENGINE_FFMPEG_BIN=""
ENGINE_FFPROBE_BIN=""
ensure_engine_cache "${DOWNLOAD_ROOT}" "${ARCHIVE_PATH}" "${ENGINE_URL}"

cp "${ENGINE_FFMPEG_BIN}" "${APPDIR}/usr/bin/ffmpeg"
cp "${ENGINE_FFPROBE_BIN}" "${APPDIR}/usr/bin/ffprobe"
chmod +x "${APPDIR}/usr/bin/ffmpeg" "${APPDIR}/usr/bin/ffprobe"

TARBALL="${DIST_ROOT}/Compressi.ty-${APP_VERSION}-${ARCH}.tar.gz"
rm -f "${TARBALL}"
tar -czf "${TARBALL}" -C "${DIST_ROOT}" "Compressi.ty.AppDir"

if command -v appimagetool >/dev/null 2>&1; then
  APPIMAGE="${DIST_ROOT}/Compressi.ty-${APP_VERSION}-${ARCH}.AppImage"
  rm -f "${APPIMAGE}"
  appimagetool "${APPDIR}" "${APPIMAGE}"
  echo "Linux AppImage created at ${APPIMAGE}"
else
  echo "appimagetool not found; created portable bundle at ${TARBALL}"
fi
