#!/bin/bash
# Build a portable Linux AppImage for Flicker.
# Usage:
#   ./packaging/appimage/build-appimage.sh
#
# Optional environment variables:
#   APPIMAGETOOL   Path to appimagetool binary (auto-detected if in PATH)
#   VERSION        Version string embedded in output filename (defaults to Cargo.toml version)
#   SKIP_BUILD     Set to 1 to reuse an existing target/release/flicker binary

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

ARCH="${ARCH:-$(uname -m)}"
VERSION="${VERSION:-1.0}"
APPDIR="$REPO_ROOT/target/AppDir"
OUT_DIR="$REPO_ROOT/dist"
OUT_FILE="$OUT_DIR/Flicker-${VERSION}-${ARCH}.AppImage"

echo "🔥 Building Flicker AppImage (v${VERSION}, ${ARCH})..."

if [ "${SKIP_BUILD:-0}" != "1" ]; then
    cargo build --release
fi

if [ ! -f "$REPO_ROOT/target/release/flicker" ]; then
    echo "Error: target/release/flicker not found." >&2
    exit 1
fi

rm -rf "$APPDIR"
mkdir -p \
    "$APPDIR/usr/bin" \
    "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/metainfo" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
    "$APPDIR/usr/share/icons/hicolor/512x512/apps" \
    "$OUT_DIR"

# Binary
install -m 755 "$REPO_ROOT/target/release/flicker" "$APPDIR/usr/bin/flicker"

# Desktop file & AppStream metadata
install -m 644 "$REPO_ROOT/packaging/flicker.desktop" "$APPDIR/flicker.desktop"
install -m 644 "$REPO_ROOT/packaging/flicker.desktop" "$APPDIR/usr/share/applications/flicker.desktop"
install -m 644 "$REPO_ROOT/packaging/io.github.vimi_tt.Flicker.metainfo.xml" \
    "$APPDIR/usr/share/metainfo/io.github.vimi_tt.Flicker.metainfo.xml"

# Icon (root + .DirIcon + hicolor)
install -m 644 "$REPO_ROOT/Flicker.png" "$APPDIR/flicker.png"
install -m 644 "$REPO_ROOT/Flicker.png" "$APPDIR/.DirIcon"
install -m 644 "$REPO_ROOT/Flicker.png" "$APPDIR/usr/share/icons/hicolor/512x512/apps/flicker.png"
if command -v convert &> /dev/null; then
    convert "$REPO_ROOT/Flicker.png" -resize 256x256 "$APPDIR/usr/share/icons/hicolor/256x256/apps/flicker.png"
else
    install -m 644 "$REPO_ROOT/Flicker.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/flicker.png"
fi

# AppRun launcher (passes CLI args through so both GUI and `pkexec $APPIMAGE internal-write` work!)
cat << 'EOF' > "$APPDIR/AppRun"
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="${HERE}/usr/bin:${PATH}"
exec "${HERE}/usr/bin/flicker" "$@"
EOF
chmod 755 "$APPDIR/AppRun"

# Locate appimagetool
APPIMAGETOOL="${APPIMAGETOOL:-}"
if [ -z "$APPIMAGETOOL" ]; then
    if command -v appimagetool &> /dev/null; then
        APPIMAGETOOL="$(command -v appimagetool)"
    elif [ -x "$REPO_ROOT/target/appimagetool-${ARCH}.AppImage" ]; then
        APPIMAGETOOL="$REPO_ROOT/target/appimagetool-${ARCH}.AppImage"
    else
        echo "appimagetool not found in PATH." >&2
        echo "Download it from https://github.com/AppImage/appimagetool/releases or set APPIMAGETOOL=/path/to/appimagetool." >&2
        echo "AppDir was staged successfully at: $APPDIR" >&2
        exit 1
    fi
fi

ARCH="$ARCH" "$APPIMAGETOOL" --no-appstream "$APPDIR" "$OUT_FILE"
sha256sum "$OUT_FILE" > "${OUT_FILE}.sha256"

echo "✅ AppImage created: $OUT_FILE"
echo "   SHA256: $(cat "${OUT_FILE}.sha256")"
