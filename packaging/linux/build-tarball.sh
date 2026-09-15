#!/usr/bin/env bash
# ==============================================================================
# build-tarball.sh - Build portable .tar.gz package for Linux (Arch, Fedora, etc.)
# ==============================================================================

set -e

export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGING_DIR="$(dirname "$SCRIPT_DIR")"
ROOT_DIR="$(dirname "$PACKAGING_DIR")"
DIST_DIR="$ROOT_DIR/dist"

ARCH="$(uname -m)"
VERSION=$(grep -m 1 '^version = ' "$ROOT_DIR/crates/ui_gui/Cargo.toml" | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"

echo "========================================================"
echo " [UWU-LOG] Linux Portable Tarball Builder (.tar.gz)"
echo " Version: $VERSION | Architecture: $ARCH"
echo "========================================================"

# 1. Build release binaries natively if not already built
if [ -z "$SKIP_BUILD" ]; then
    echo "-> Building release binaries with cargo..."
    cd "$ROOT_DIR"
    cargo build --release --bin uwu-gui --bin uwu-tui --bin uwu-agent --bin uwulog
fi

# 2. Prepare staging directory
PKG_NAME="uwulog-v${VERSION}-linux-${ARCH}"
STAGE_DIR="$ROOT_DIR/target/tarball_staging/$PKG_NAME"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/bin"
mkdir -p "$DIST_DIR"

# 3. Copy binaries
cp "$ROOT_DIR/target/release/uwu-gui" "$STAGE_DIR/bin/"
cp "$ROOT_DIR/target/release/uwu-tui" "$STAGE_DIR/bin/"
cp "$ROOT_DIR/target/release/uwu-agent" "$STAGE_DIR/bin/"
cp "$ROOT_DIR/target/release/uwulog" "$STAGE_DIR/bin/"
chmod 755 "$STAGE_DIR/bin/"*

# 4. Copy assets
cp "$PACKAGING_DIR/assets/uwulog.desktop" "$STAGE_DIR/"
cp "$PACKAGING_DIR/assets/icon_512.png" "$STAGE_DIR/uwulog.png"

# 5. Standalone installer script
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

# 6. Standalone uninstaller script
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

# 7. Compress tarball
OUTPUT_TARBALL="$DIST_DIR/${PKG_NAME}.tar.gz"
cd "$ROOT_DIR/target/tarball_staging"
tar -czf "$OUTPUT_TARBALL" "$PKG_NAME"

echo ""
echo "✅ Successfully built Linux portable tarball: $OUTPUT_TARBALL"
ls -lh "$OUTPUT_TARBALL"
