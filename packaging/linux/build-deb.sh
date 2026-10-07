#!/usr/bin/env bash
# ==============================================================================
# build-deb.sh - Build native Debian / Ubuntu (.deb) package for Uwu Log Viewer
# Supports: amd64 (x86_64), arm64 (aarch64)
# ==============================================================================

set -e

export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGING_DIR="$(dirname "$SCRIPT_DIR")"
ROOT_DIR="$(dirname "$PACKAGING_DIR")"
DIST_DIR="$ROOT_DIR/dist"

# Parse CLI arguments
ARCH=""
TARGET=""
SKIP_BUILD=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch)
            ARCH="$2"
            shift 2
            ;;
        --target)
            TARGET="$2"
            shift 2
            ;;
        --skip-build)
            SKIP_BUILD="1"
            shift
            ;;
        -h|--help)
            echo "Usage: ./build-deb.sh [--arch <amd64|arm64>] [--target <triple>] [--skip-build]"
            exit 0
            ;;
        *)
            if [ -z "$ARCH" ]; then
                ARCH="$1"
            fi
            shift
            ;;
    esac
done

# Detect or normalize architecture
if [ -z "$ARCH" ]; then
    if command -v dpkg >/dev/null 2>&1; then
        ARCH="$(dpkg --print-architecture)"
    else
        ARCH="$(uname -m)"
    fi
fi

case "$ARCH" in
    x86_64|amd64)
        ARCH="amd64"
        DEFAULT_TARGET="x86_64-unknown-linux-gnu"
        ;;
    aarch64|arm64)
        ARCH="arm64"
        DEFAULT_TARGET="aarch64-unknown-linux-gnu"
        ;;
    *)
        DEFAULT_TARGET=""
        ;;
esac

TARGET="${TARGET:-$DEFAULT_TARGET}"
VERSION=$(grep -m 1 '^version = ' "$ROOT_DIR/crates/ui_gui/Cargo.toml" | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"

echo "========================================================"
echo " [UWU-LOG] Linux Debian Package Builder (.deb)"
echo " Version: $VERSION | Arch: $ARCH | Target: ${TARGET:-host}"
echo "========================================================"

# 1. Determine binary directory & cargo flags
HOST_ARCH="$(uname -m)"
CARGO_FLAGS=()
BIN_DIR="$ROOT_DIR/target/release"

if [ -n "$TARGET" ] && { [ "$ARCH" = "arm64" ] && [ "$HOST_ARCH" = "x86_64" ]; }; then
    CARGO_FLAGS+=(--target "$TARGET")
    BIN_DIR="$ROOT_DIR/target/$TARGET/release"
elif [ -n "$TARGET" ] && [ -d "$ROOT_DIR/target/$TARGET/release" ]; then
    BIN_DIR="$ROOT_DIR/target/$TARGET/release"
fi

# 2. Build release binaries natively or cross-compiled if not skipped
if [ -z "$SKIP_BUILD" ]; then
    echo "-> Building release binaries with cargo (${CARGO_FLAGS[*]:-default host})..."
    cd "$ROOT_DIR"
    cargo build --release "${CARGO_FLAGS[@]}" --bin uwu-gui --bin uwu-agent --bin uwulog
else
    echo "-> Skipping cargo build (--skip-build specified)..."
    # Fallback to target/release if target triple dir not present
    if [ ! -f "$BIN_DIR/uwu-gui" ] && [ -f "$ROOT_DIR/target/release/uwu-gui" ]; then
        BIN_DIR="$ROOT_DIR/target/release"
    fi
fi

echo "   Using binaries from: $BIN_DIR"

if [ ! -f "$BIN_DIR/uwu-gui" ]; then
    echo "❌ Error: Binaries not found in $BIN_DIR!"
    exit 1
fi

# 3. Prepare staging directory (use /tmp for true POSIX filesystem permissions)
PKG_NAME="uwulog_${VERSION}_${ARCH}"
STAGE_DIR="${TMPDIR:-/tmp}/deb_staging/$PKG_NAME"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/DEBIAN"
mkdir -p "$STAGE_DIR/usr/bin"
mkdir -p "$STAGE_DIR/usr/share/applications"
mkdir -p "$STAGE_DIR/usr/share/pixmaps"
mkdir -p "$DIST_DIR"

# 4. Create DEBIAN/control file
cat <<EOF > "$STAGE_DIR/DEBIAN/control"
Package: uwulog
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: gavuhop <https://github.com/gavuhop/uwulog-rust>
Description: High-performance log viewer and workspace monitor
 A blazing-fast log viewer, live streaming monitor, and remote agent
 supporting structured search, filters, journald, and file tailing.
EOF
chmod 755 "$STAGE_DIR/DEBIAN"
chmod 644 "$STAGE_DIR/DEBIAN/control"

# 5. Create DEBIAN/postinst hook
cat << 'EOF' > "$STAGE_DIR/DEBIAN/postinst"
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
if command -v gtk4-update-icon-cache >/dev/null 2>&1; then
    gtk4-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental 2>/dev/null || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    kbuildsycoca5 --noincremental 2>/dev/null || true
fi
EOF
chmod 755 "$STAGE_DIR/DEBIAN/postinst"

# 6. Create DEBIAN/postrm hook
cat << 'EOF' > "$STAGE_DIR/DEBIAN/postrm"
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
if command -v gtk4-update-icon-cache >/dev/null 2>&1; then
    gtk4-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental 2>/dev/null || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    kbuildsycoca5 --noincremental 2>/dev/null || true
fi
EOF
chmod 755 "$STAGE_DIR/DEBIAN/postrm"

# 7. Copy binaries and assets
cp "$BIN_DIR/uwu-gui" "$STAGE_DIR/usr/bin/"
cp "$BIN_DIR/uwu-agent" "$STAGE_DIR/usr/bin/"
cp "$BIN_DIR/uwulog" "$STAGE_DIR/usr/bin/"
chmod 755 "$STAGE_DIR/usr/bin/"*

# Desktop entry
cp "$PACKAGING_DIR/assets/uwulog.desktop" "$STAGE_DIR/usr/share/applications/"
chmod 644 "$STAGE_DIR/usr/share/applications/uwulog.desktop"

# Install all icon resolutions into hicolor theme
for size in 16 32 48 64 128 256 512; do
    icon_dir="$STAGE_DIR/usr/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$icon_dir"
    if [ -f "$PACKAGING_DIR/assets/icon_${size}.png" ]; then
        cp "$PACKAGING_DIR/assets/icon_${size}.png" "$icon_dir/uwulog.png"
        chmod 644 "$icon_dir/uwulog.png"
    fi
done

# Pixmaps fallback
cp "$PACKAGING_DIR/assets/icon_256.png" "$STAGE_DIR/usr/share/pixmaps/uwulog.png"
chmod 644 "$STAGE_DIR/usr/share/pixmaps/uwulog.png"

# 8. Build .deb package
OUTPUT_DEB="$DIST_DIR/${PKG_NAME}.deb"
dpkg-deb --build --root-owner-group "$STAGE_DIR" "$OUTPUT_DEB"
rm -rf "$STAGE_DIR"

echo ""
echo "✅ Successfully built Debian package: $OUTPUT_DEB"
ls -lh "$OUTPUT_DEB"
