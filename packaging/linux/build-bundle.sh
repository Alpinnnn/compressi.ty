#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_ROOT="${REPO_ROOT}/dist/linux"
APPDIR="${DIST_ROOT}/Compressi.ty.AppDir"
CACHE_DIR="${DIST_ROOT}/engine-cache"
PDF_ENGINE_CACHE_DIR="${DIST_ROOT}/pdf-engine-cache"
PACKAGE_ENGINE_CACHE_DIR="${DIST_ROOT}/package-engine-cache"
GHOSTSCRIPT_VERSION="10.07.0"
GHOSTSCRIPT_TAG="gs10070"
GHOSTSCRIPT_AMD64_SNAP_URL="https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/${GHOSTSCRIPT_TAG}/gs_${GHOSTSCRIPT_VERSION}_amd64_snap.tgz"
QPDF_VERSION="12.3.2"
QPDF_AMD64_URL="https://sourceforge.net/projects/qpdf/files/qpdf/${QPDF_VERSION}/qpdf-${QPDF_VERSION}-bin-linux-x86_64.zip/download"
SEVEN_ZIP_VERSION="26.01"
SEVEN_ZIP_AMD64_URL="https://github.com/ip7z/7zip/releases/download/${SEVEN_ZIP_VERSION}/7z2601-linux-x64.tar.xz"
SEVEN_ZIP_ARM64_URL="https://github.com/ip7z/7zip/releases/download/${SEVEN_ZIP_VERSION}/7z2601-linux-arm64.tar.xz"

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

download_document_engine_archive() {
  local archive_path="$1"
  local engine_url="$2"
  local engine_label="$3"

  rm -f "${archive_path}"
  echo "Downloading bundled ${engine_label} for Linux..."
  curl -fL --retry 3 --retry-delay 2 "${engine_url}" -o "${archive_path}"
}

ensure_document_engine_cache() {
  local download_root="$1"
  local archive_path="$2"
  local engine_url="$3"

  mkdir -p "${download_root}"

  if [[ -z "${engine_url}" ]]; then
    local system_gs
    system_gs="$(command -v gs || true)"
    if [[ -z "${system_gs}" ]]; then
      echo "This architecture has no configured bundled Ghostscript archive and no system gs was found." >&2
      exit 1
    fi

    local system_root="${download_root}/system"
    rm -rf "${system_root}"
    mkdir -p "${system_root}/bin"
    cp "${system_gs}" "${system_root}/bin/gs"
    DOCUMENT_ENGINE_ROOT="${system_root}"
    return
  fi

  require_cmd unsquashfs "Install squashfs-tools to unpack the bundled Ghostscript snap."

  if [[ ! -f "${archive_path}" ]]; then
    download_document_engine_archive "${archive_path}" "${engine_url}" "Ghostscript ${GHOSTSCRIPT_VERSION}"
  fi

  local extract_root="${download_root}/extract"
  local snap_root="${download_root}/snap-root"
  local gs_bin
  gs_bin="$(find "${snap_root}" -type f -name gs 2>/dev/null | head -n 1)"

  if [[ -z "${gs_bin}" ]]; then
    rm -rf "${extract_root}" "${snap_root}"
    mkdir -p "${extract_root}"
    echo "Extracting cached Ghostscript runtime..."
    tar -xf "${archive_path}" -C "${extract_root}"

    local snap_path
    snap_path="$(find "${extract_root}" -type f -name "*.snap" | head -n 1)"
    if [[ -z "${snap_path}" ]]; then
      echo "The cached Ghostscript archive did not contain a snap package." >&2
      exit 1
    fi

    unsquashfs -f -d "${snap_root}" "${snap_path}" >/dev/null
    gs_bin="$(find "${snap_root}" -type f -name gs | head -n 1)"
  fi

  if [[ -z "${gs_bin}" ]]; then
    echo "The cached Ghostscript archive did not contain a gs binary." >&2
    exit 1
  fi

  DOCUMENT_ENGINE_ROOT="${snap_root}"
}

ensure_qpdf_cache() {
  local download_root="$1"
  local archive_path="$2"
  local engine_url="$3"

  mkdir -p "${download_root}"

  if [[ -z "${engine_url}" ]]; then
    local system_qpdf
    system_qpdf="$(command -v qpdf || true)"
    if [[ -z "${system_qpdf}" ]]; then
      echo "This architecture has no configured bundled qpdf archive and no system qpdf was found." >&2
      exit 1
    fi

    local system_root="${download_root}/qpdf-system"
    rm -rf "${system_root}"
    mkdir -p "${system_root}/bin"
    cp "${system_qpdf}" "${system_root}/bin/qpdf"
    QPDF_ENGINE_ROOT="${system_root}"
    return
  fi

  require_cmd unzip "Install unzip to unpack the bundled qpdf runtime."

  if [[ ! -f "${archive_path}" ]]; then
    download_document_engine_archive "${archive_path}" "${engine_url}" "qpdf ${QPDF_VERSION}"
  fi

  local qpdf_root="${download_root}/qpdf"
  local qpdf_bin
  qpdf_bin="$(find "${qpdf_root}" -type f -name qpdf 2>/dev/null | head -n 1)"

  if [[ -z "${qpdf_bin}" ]]; then
    rm -rf "${qpdf_root}"
    mkdir -p "${qpdf_root}"
    echo "Extracting cached qpdf runtime..."
    unzip -q "${archive_path}" -d "${qpdf_root}"
    qpdf_bin="$(find "${qpdf_root}" -type f -name qpdf | head -n 1)"
  fi

  if [[ -z "${qpdf_bin}" ]]; then
    echo "The cached qpdf archive did not contain a qpdf binary." >&2
    exit 1
  fi

  QPDF_ENGINE_ROOT="${qpdf_root}"
}

ensure_package_engine_cache() {
  local download_root="$1"
  local archive_path="$2"
  local engine_url="$3"

  mkdir -p "${download_root}"

  if [[ -z "${engine_url}" ]]; then
    echo "This architecture has no configured bundled 7-Zip archive." >&2
    exit 1
  fi

  if [[ ! -f "${archive_path}" ]]; then
    download_document_engine_archive "${archive_path}" "${engine_url}" "7-Zip ${SEVEN_ZIP_VERSION}"
  fi

  local seven_zip_root="${download_root}/7zip"
  local seven_zip_bin
  seven_zip_bin="$(find "${seven_zip_root}" -type f \( -name 7zz -o -name 7z \) 2>/dev/null | head -n 1)"

  if [[ -z "${seven_zip_bin}" ]]; then
    rm -rf "${seven_zip_root}"
    mkdir -p "${seven_zip_root}"
    echo "Extracting cached 7-Zip runtime..."
    tar -xf "${archive_path}" -C "${seven_zip_root}"
    seven_zip_bin="$(find "${seven_zip_root}" -type f \( -name 7zz -o -name 7z \) | head -n 1)"
  fi

  if [[ -z "${seven_zip_bin}" ]]; then
    echo "The cached 7-Zip archive did not contain a 7z/7zz binary." >&2
    exit 1
  fi

  PACKAGE_ENGINE_ROOT="${seven_zip_root}"
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
    DOCUMENT_ENGINE_URL="${GHOSTSCRIPT_AMD64_SNAP_URL}"
    QPDF_ENGINE_URL="${QPDF_AMD64_URL}"
    PACKAGE_ENGINE_URL="${SEVEN_ZIP_AMD64_URL}"
    ;;
  aarch64|arm64)
    ENGINE_URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"
    DOCUMENT_ENGINE_URL=""
    QPDF_ENGINE_URL=""
    PACKAGE_ENGINE_URL="${SEVEN_ZIP_ARM64_URL}"
    ;;
  *)
    echo "Unsupported Linux architecture: ${ARCH}" >&2
    exit 1
    ;;
esac

rm -rf "${APPDIR}"
mkdir -p \
  "${APPDIR}/usr/bin" \
  "${APPDIR}/usr/bin/engine/video-engine" \
  "${APPDIR}/usr/bin/engine/pdf-engine" \
  "${APPDIR}/usr/bin/engine/package-engine" \
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

cp "${ENGINE_FFMPEG_BIN}" "${APPDIR}/usr/bin/engine/video-engine/ffmpeg"
cp "${ENGINE_FFPROBE_BIN}" "${APPDIR}/usr/bin/engine/video-engine/ffprobe"
chmod +x "${APPDIR}/usr/bin/engine/video-engine/ffmpeg" "${APPDIR}/usr/bin/engine/video-engine/ffprobe"

DOCUMENT_DOWNLOAD_ROOT="${PDF_ENGINE_CACHE_DIR}/download"
DOCUMENT_ARCHIVE_PATH="${DOCUMENT_DOWNLOAD_ROOT}/ghostscript-snap.tgz"
DOCUMENT_ENGINE_ROOT=""
ensure_document_engine_cache "${DOCUMENT_DOWNLOAD_ROOT}" "${DOCUMENT_ARCHIVE_PATH}" "${DOCUMENT_ENGINE_URL}"
cp -a "${DOCUMENT_ENGINE_ROOT}/." "${APPDIR}/usr/bin/engine/pdf-engine/"
find "${APPDIR}/usr/bin/engine/pdf-engine" -type f -name gs -exec chmod +x {} +

QPDF_ARCHIVE_PATH="${DOCUMENT_DOWNLOAD_ROOT}/qpdf.zip"
QPDF_ENGINE_ROOT=""
ensure_qpdf_cache "${DOCUMENT_DOWNLOAD_ROOT}" "${QPDF_ARCHIVE_PATH}" "${QPDF_ENGINE_URL}"
mkdir -p "${APPDIR}/usr/bin/engine/pdf-engine/qpdf"
cp -a "${QPDF_ENGINE_ROOT}/." "${APPDIR}/usr/bin/engine/pdf-engine/qpdf/"
find "${APPDIR}/usr/bin/engine/pdf-engine/qpdf" -type f -name qpdf -exec chmod +x {} +

PACKAGE_DOWNLOAD_ROOT="${PACKAGE_ENGINE_CACHE_DIR}/download"
PACKAGE_ARCHIVE_PATH="${PACKAGE_DOWNLOAD_ROOT}/7zip.tar.xz"
PACKAGE_ENGINE_ROOT=""
ensure_package_engine_cache "${PACKAGE_DOWNLOAD_ROOT}" "${PACKAGE_ARCHIVE_PATH}" "${PACKAGE_ENGINE_URL}"
cp -a "${PACKAGE_ENGINE_ROOT}/." "${APPDIR}/usr/bin/engine/package-engine/"
find "${APPDIR}/usr/bin/engine/package-engine" -type f \( -name 7zz -o -name 7z \) -exec chmod +x {} +

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
