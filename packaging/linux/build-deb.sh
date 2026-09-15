#!/usr/bin/env bash
# ==============================================================================
# build-deb.sh - Build native Debian / Ubuntu (.deb) package for Uwu Log Viewer
# ==============================================================================

set -e

export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGING_DIR="$(dirname "$SCRIPT_DIR")"
ROOT_DIR="$(dirname "$PACKAGING_DIR")"
DIST_DIR="$ROOT_DIR/dist"

ARCH="${1:-amd64}"
VERSION=$(grep -m 1 '^version = ' "$ROOT_DIR/crates/ui_gui/Cargo.toml" | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"

echo "========================================================"
echo " [UWU-LOG] Linux Debian Package Builder (.deb)"
echo " Version: $VERSION | Architecture: $ARCH"
echo "========================================================"

# 1. Build release binaries natively if not already built
if [ -z "$SKIP_BUILD" ]; then
    echo "-> Building release binaries with cargo..."
    cd "$ROOT_DIR"
    cargo build --release --bin uwu-gui --bin uwu-tui --bin uwu-agent --bin uwulog
fi

# 2. Prepare staging directory (use /tmp for true POSIX filesystem permissions)
PKG_NAME="uwulog_${VERSION}_${ARCH}"
STAGE_DIR="${TMPDIR:-/tmp}/deb_staging/$PKG_NAME"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/DEBIAN"
mkdir -p "$STAGE_DIR/usr/bin"
mkdir -p "$STAGE_DIR/usr/share/applications"
mkdir -p "$STAGE_DIR/usr/share/icons/hicolor/512x512/apps"
mkdir -p "$DIST_DIR"

# 3. Create DEBIAN/control file
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

# 4. Create DEBIAN/postinst hook
cat << 'EOF' > "$STAGE_DIR/DEBIAN/postinst"
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q /usr/share/icons/hicolor || true
fi
EOF
chmod 755 "$STAGE_DIR/DEBIAN/postinst"

# 5. Create DEBIAN/postrm hook
cat << 'EOF' > "$STAGE_DIR/DEBIAN/postrm"
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q /usr/share/icons/hicolor || true
fi
EOF
chmod 755 "$STAGE_DIR/DEBIAN/postrm"

# 6. Copy binaries and assets
cp "$ROOT_DIR/target/release/uwu-gui" "$STAGE_DIR/usr/bin/"
cp "$ROOT_DIR/target/release/uwu-tui" "$STAGE_DIR/usr/bin/"
cp "$ROOT_DIR/target/release/uwu-agent" "$STAGE_DIR/usr/bin/"
cp "$ROOT_DIR/target/release/uwulog" "$STAGE_DIR/usr/bin/"
chmod 755 "$STAGE_DIR/usr/bin/"*

# Desktop entry & icon
cp "$PACKAGING_DIR/assets/uwulog.desktop" "$STAGE_DIR/usr/share/applications/"
cp "$PACKAGING_DIR/assets/icon_512.png" "$STAGE_DIR/usr/share/icons/hicolor/512x512/apps/uwulog.png"
chmod 644 "$STAGE_DIR/usr/share/applications/uwulog.desktop"
chmod 644 "$STAGE_DIR/usr/share/icons/hicolor/512x512/apps/uwulog.png"

# 7. Build .deb package
OUTPUT_DEB="$DIST_DIR/${PKG_NAME}.deb"
dpkg-deb --build --root-owner-group "$STAGE_DIR" "$OUTPUT_DEB"
rm -rf "$STAGE_DIR"

echo ""
echo "✅ Successfully built Debian package: $OUTPUT_DEB"
ls -lh "$OUTPUT_DEB"
