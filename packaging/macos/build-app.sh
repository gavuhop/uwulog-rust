#!/usr/bin/env bash
# ==============================================================================
# build-app.sh - Package native macOS UwuLog.app Bundle
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGING_DIR="$(dirname "$SCRIPT_DIR")"
ROOT_DIR="$(dirname "$PACKAGING_DIR")"
DIST_DIR="$ROOT_DIR/dist"

VERSION=$(grep -m 1 '^version = ' "$ROOT_DIR/crates/ui_gui/Cargo.toml" | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"

echo "========================================================"
echo " [UWU-LOG] macOS App Bundle Builder (UwuLog.app)"
echo " Version: $VERSION"
echo "========================================================"

# 1. Build release binaries natively if not already built
if [ -z "$SKIP_BUILD" ]; then
    echo "-> Building release binaries with cargo..."
    cd "$ROOT_DIR"
    cargo build --release --bin uwu-gui --bin uwu-tui --bin uwu-agent --bin uwulog
fi

# 2. Prepare UwuLog.app structure
APP_BUNDLE="$DIST_DIR/UwuLog.app"
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources/bin"

# 3. Copy binaries
cp "$ROOT_DIR/target/release/uwu-gui" "$APP_BUNDLE/Contents/MacOS/"
cp "$ROOT_DIR/target/release/uwu-tui" "$APP_BUNDLE/Contents/Resources/bin/"
cp "$ROOT_DIR/target/release/uwu-agent" "$APP_BUNDLE/Contents/Resources/bin/"
cp "$ROOT_DIR/target/release/uwulog" "$APP_BUNDLE/Contents/Resources/bin/"
chmod 755 "$APP_BUNDLE/Contents/MacOS/"*
chmod 755 "$APP_BUNDLE/Contents/Resources/bin/"*

# 5. Copy Info.plist & Icon
sed "s/0.1.0/$VERSION/g" "$SCRIPT_DIR/Info.plist" > "$APP_BUNDLE/Contents/Info.plist"
if [ -f "$PACKAGING_DIR/assets/icon.icns" ]; then
    cp "$PACKAGING_DIR/assets/icon.icns" "$APP_BUNDLE/Contents/Resources/AppIcon.icns"
fi

# 6. Code Signing (Ad-hoc or Developer ID)
if command -v codesign >/dev/null 2>&1; then
    if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
        echo "-> Signing with Developer ID: $APPLE_SIGNING_IDENTITY..."
        codesign --force --deep --options runtime --sign "$APPLE_SIGNING_IDENTITY" "$APP_BUNDLE"
    else
        echo "-> Performing local ad-hoc code sign..."
        codesign --force --deep --sign - "$APP_BUNDLE"
    fi
else
    echo "-> Notice: codesign not available on this platform (cross-build or non-macOS host)."
fi

echo ""
echo "✅ Successfully built macOS bundle: $APP_BUNDLE"
