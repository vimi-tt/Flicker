#!/bin/bash
# ==============================================================================
# Flicker AppImage Packaging Script
# Generates a standalone, relocatable Linux AppImage for Flicker.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Target architecture (defaults to host architecture)
HOST_ARCH="$(uname -m)"
ARCH="${ARCH:-${HOST_ARCH}}"

# Version extraction from Cargo.toml or environment
VERSION="${VERSION:-$(grep -m 1 '^version = ' "${ROOT_DIR}/Cargo.toml" | cut -d '"' -f 2)}"

echo "=================================================="
echo "  Flicker AppImage Builder"
echo "  Target Architecture: ${ARCH}"
echo "  Version:             ${VERSION}"
echo "=================================================="

# Check if container build is requested
USE_CONTAINER="${USE_CONTAINER:-0}"
if [[ "${1:-}" == "--container" ]]; then
    USE_CONTAINER=1
fi

if [[ "${USE_CONTAINER}" == "1" ]]; then
    echo "🐳 Containerized build requested (Ubuntu 22.04 for GLIBC 2.35 compatibility)..."
    CONTAINER_ENGINE=""
    if command -v podman &>/dev/null; then
        CONTAINER_ENGINE="podman"
    elif command -v docker &>/dev/null; then
        CONTAINER_ENGINE="docker"
    else
        echo "❌ Error: Neither podman nor docker found for containerized build."
        exit 1
    fi

    echo "Using ${CONTAINER_ENGINE} to build in clean Ubuntu 22.04 container..."
    ${CONTAINER_ENGINE} run --rm \
        -v "${ROOT_DIR}:/workspace:Z" \
        -w /workspace \
        ubuntu:22.04 \
        bash -c "
            set -euo pipefail
            export DEBIAN_FRONTEND=noninteractive
            apt-get update -qq
            apt-get install -y -qq curl build-essential pkg-config libudev-dev
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
            source \$HOME/.cargo/env
            cargo build --release
        "
else
    echo "🔨 Building Flicker in release mode on host..."
    if ! command -v cargo &>/dev/null; then
        if [[ -f "${HOME}/.cargo/env" ]]; then
            # shellcheck source=/dev/null
            source "${HOME}/.cargo/env"
        else
            echo "❌ Error: cargo not found in PATH."
            exit 1
        fi
    fi
    (cd "${ROOT_DIR}" && cargo build --release)
fi

BINARY_PATH="${ROOT_DIR}/target/release/flicker"
if [[ ! -f "${BINARY_PATH}" ]]; then
    echo "❌ Error: Compiled binary not found at ${BINARY_PATH}"
    exit 1
fi

echo "✓ Binary compiled successfully ($(du -h "${BINARY_PATH}" | cut -f 1))"

# Check dynamic dependencies
echo "📋 Inspecting dynamic dependencies (ldd):"
ldd "${BINARY_PATH}" || true

# Prepare AppDir directory
APPDIR="${ROOT_DIR}/target/AppDir"
echo "📁 Preparing AppDir at ${APPDIR}..."
rm -rf "${APPDIR}"
mkdir -p \
    "${APPDIR}/usr/bin" \
    "${APPDIR}/usr/lib" \
    "${APPDIR}/usr/share/applications" \
    "${APPDIR}/usr/share/metainfo"

# Copy binary and AppRun
cp "${BINARY_PATH}" "${APPDIR}/usr/bin/flicker"
chmod +x "${APPDIR}/usr/bin/flicker"

cp "${SCRIPT_DIR}/AppRun" "${APPDIR}/AppRun"
chmod +x "${APPDIR}/AppRun"

# Copy desktop and metainfo files
cp "${SCRIPT_DIR}/flicker.desktop" "${APPDIR}/usr/share/applications/flicker.desktop"
cp "${SCRIPT_DIR}/flicker.desktop" "${APPDIR}/flicker.desktop"

cp "${SCRIPT_DIR}/io.github.vimi_tt.flicker.metainfo.xml" "${APPDIR}/usr/share/metainfo/io.github.vimi_tt.flicker.metainfo.xml"
cp "${SCRIPT_DIR}/io.github.vimi_tt.flicker.metainfo.xml" "${APPDIR}/usr/share/metainfo/flicker.metainfo.xml"

# Generate multi-resolution icons
echo "🎨 Generating multi-resolution icons..."
ICON_SRC="${ROOT_DIR}/Flicker.png"
ICON_SIZES=(16 32 48 64 128 256 512)

for size in "${ICON_SIZES[@]}"; do
    TARGET_DIR="${APPDIR}/usr/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "${TARGET_DIR}"
    if command -v magick &>/dev/null; then
        magick "${ICON_SRC}" -resize "${size}x${size}" "${TARGET_DIR}/flicker.png"
    elif command -v convert &>/dev/null; then
        convert "${ICON_SRC}" -resize "${size}x${size}" "${TARGET_DIR}/flicker.png"
    elif python3 -c "import PIL" &>/dev/null; then
        python3 -c "from PIL import Image; img = Image.open('${ICON_SRC}'); img.resize((${size}, ${size}), Image.Resampling.LANCZOS).save('${TARGET_DIR}/flicker.png')"
    else
        cp "${ICON_SRC}" "${TARGET_DIR}/flicker.png"
    fi
done

# AppDir root icon and .DirIcon
cp "${APPDIR}/usr/share/icons/hicolor/512x512/apps/flicker.png" "${APPDIR}/flicker.png"
cp "${APPDIR}/usr/share/icons/hicolor/512x512/apps/flicker.png" "${APPDIR}/.DirIcon"

# Obtain appimagetool if not present
TOOLS_DIR="${ROOT_DIR}/target/tools"
mkdir -p "${TOOLS_DIR}"

APPIMAGETOOL=""
if command -v appimagetool &>/dev/null; then
    APPIMAGETOOL="$(command -v appimagetool)"
    echo "✓ Using system appimagetool: ${APPIMAGETOOL}"
else
    APPIMAGETOOL="${TOOLS_DIR}/appimagetool-${ARCH}"
    if [[ ! -f "${APPIMAGETOOL}" ]]; then
        echo "⬇️ Downloading appimagetool for ${ARCH}..."
        TOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"
        curl -fsSL -o "${APPIMAGETOOL}" "${TOOL_URL}"
        chmod +x "${APPIMAGETOOL}"
    fi
    echo "✓ Using cached appimagetool: ${APPIMAGETOOL}"
fi

OUTPUT_NAME="Flicker-${VERSION}-${ARCH}.AppImage"
OUTPUT_PATH="${ROOT_DIR}/target/${OUTPUT_NAME}"
SYMLINK_PATH="${ROOT_DIR}/target/Flicker-${ARCH}.AppImage"

echo "📦 Packaging AppImage: ${OUTPUT_NAME}..."

# Export architecture for appimagetool
export ARCH="${ARCH}"

# Try running appimagetool directly or with --appimage-extract-and-run
if ! "${APPIMAGETOOL}" "${APPDIR}" "${OUTPUT_PATH}" 2>/dev/null; then
    echo "Retrying appimagetool with --appimage-extract-and-run..."
    "${APPIMAGETOOL}" --appimage-extract-and-run "${APPDIR}" "${OUTPUT_PATH}"
fi

ln -sf "${OUTPUT_NAME}" "${SYMLINK_PATH}"
chmod +x "${OUTPUT_PATH}"

echo "=================================================="
echo "🎉 AppImage generated successfully!"
echo "   Output:  ${OUTPUT_PATH}"
echo "   Size:    $(du -h "${OUTPUT_PATH}" | cut -f 1)"
echo "   Symlink: ${SYMLINK_PATH}"
echo "=================================================="
