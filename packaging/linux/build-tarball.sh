#!/usr/bin/env bash
# ==============================================================================
# build-tarball.sh - Build portable .tar.gz package for Linux (Multi-Arch)
# Supports: x86_64, aarch64 (ARM64)
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
            echo "Usage: ./build-tarball.sh [--arch <x86_64|aarch64>] [--target <triple>] [--skip-build]"
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
    ARCH="$(uname -m)"
fi

case "$ARCH" in
    x86_64|amd64)
        ARCH="x86_64"
        DEFAULT_TARGET="x86_64-unknown-linux-gnu"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
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
echo " [UWU-LOG] Linux Portable Tarball Builder (.tar.gz)"
echo " Version: $VERSION | Arch: $ARCH | Target: ${TARGET:-host}"
echo "========================================================"

# 1. Determine binary directory & cargo flags
HOST_ARCH="$(uname -m)"
CARGO_FLAGS=()
BIN_DIR="$ROOT_DIR/target/release"

if [ -n "$TARGET" ] && [ "$ARCH" != "$HOST_ARCH" ]; then
    CARGO_FLAGS+=(--target "$TARGET")
    BIN_DIR="$ROOT_DIR/target/$TARGET/release"
elif [ -n "$TARGET" ] && [ -d "$ROOT_DIR/target/$TARGET/release" ]; then
    BIN_DIR="$ROOT_DIR/target/$TARGET/release"
fi

# 2. Build release binaries natively or cross-compiled if not skipped
if [ -z "$SKIP_BUILD" ]; then
    echo "-> Building release binaries with cargo (${CARGO_FLAGS[*]:-default host})..."
    cd "$ROOT_DIR"
    cargo build --release "${CARGO_FLAGS[@]}" --bin uwu-gui --bin uwu-tui --bin uwu-agent --bin uwulog
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

# 3. Prepare staging directory
PKG_NAME="uwulog-v${VERSION}-linux-${ARCH}"
STAGE_DIR="$ROOT_DIR/target/tarball_staging/$PKG_NAME"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/bin"
mkdir -p "$DIST_DIR"

# 4. Copy binaries
cp "$BIN_DIR/uwu-gui" "$STAGE_DIR/bin/"
cp "$BIN_DIR/uwu-tui" "$STAGE_DIR/bin/"
cp "$BIN_DIR/uwu-agent" "$STAGE_DIR/bin/"
cp "$BIN_DIR/uwulog" "$STAGE_DIR/bin/"
chmod 755 "$STAGE_DIR/bin/"*

# 5. Copy assets
cp "$PACKAGING_DIR/assets/uwulog.desktop" "$STAGE_DIR/"
cp "$PACKAGING_DIR/assets/icon_512.png" "$STAGE_DIR/uwulog.png"

# 6. Standalone installer script
cat << 'EOF' > "$STAGE_DIR/install.sh"
#!/usr/bin/env bash
set -e

INSTALL_PREFIX="${1:-/usr/local}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Installing Uwu Log to $INSTALL_PREFIX..."

mkdir -p "$INSTALL_PREFIX/bin"
cp "$SCRIPT_DIR/bin/uwu-gui" "$INSTALL_PREFIX/bin/"
cp "$SCRIPT_DIR/bin/uwu-tui" "$INSTALL_PREFIX/bin/"
cp "$SCRIPT_DIR/bin/uwu-agent" "$INSTALL_PREFIX/bin/"
cp "$SCRIPT_DIR/bin/uwulog" "$INSTALL_PREFIX/bin/"
chmod 755 "$INSTALL_PREFIX/bin/"*

# Desktop Entry if installing to system
if [ "$INSTALL_PREFIX" = "/usr" ] || [ "$INSTALL_PREFIX" = "/usr/local" ]; then
    mkdir -p "$INSTALL_PREFIX/share/applications"
    mkdir -p "$INSTALL_PREFIX/share/icons/hicolor/512x512/apps"
    cp "$SCRIPT_DIR/uwulog.desktop" "$INSTALL_PREFIX/share/applications/"
    cp "$SCRIPT_DIR/uwulog.png" "$INSTALL_PREFIX/share/icons/hicolor/512x512/apps/uwulog.png"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "$INSTALL_PREFIX/share/applications" || true
    fi
fi

echo "✅ Uwu Log successfully installed to $INSTALL_PREFIX/bin/uwulog!"
EOF
chmod 755 "$STAGE_DIR/install.sh"

# 7. Standalone uninstaller script
cat << 'EOF' > "$STAGE_DIR/uninstall.sh"
#!/usr/bin/env bash
set -e

INSTALL_PREFIX="${1:-/usr/local}"
echo "Uninstalling Uwu Log from $INSTALL_PREFIX..."

rm -f "$INSTALL_PREFIX/bin/uwu-gui"
rm -f "$INSTALL_PREFIX/bin/uwu-tui"
rm -f "$INSTALL_PREFIX/bin/uwu-agent"
rm -f "$INSTALL_PREFIX/bin/uwulog"
rm -f "$INSTALL_PREFIX/share/applications/uwulog.desktop"
rm -f "$INSTALL_PREFIX/share/icons/hicolor/512x512/apps/uwulog.png"

echo "✅ Uwu Log successfully uninstalled."
EOF
chmod 755 "$STAGE_DIR/uninstall.sh"

# 8. Compress tarball
OUTPUT_TARBALL="$DIST_DIR/${PKG_NAME}.tar.gz"
cd "$ROOT_DIR/target/tarball_staging"
tar -czf "$OUTPUT_TARBALL" "$PKG_NAME"

echo ""
echo "✅ Successfully built Linux portable tarball: $OUTPUT_TARBALL"
ls -lh "$OUTPUT_TARBALL"
