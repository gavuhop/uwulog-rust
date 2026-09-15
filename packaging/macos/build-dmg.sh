#!/usr/bin/env bash
# ==============================================================================
# build-dmg.sh - Package native macOS Drag-and-Drop Disk Image (.dmg)
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGING_DIR="$(dirname "$SCRIPT_DIR")"
ROOT_DIR="$(dirname "$PACKAGING_DIR")"
DIST_DIR="$ROOT_DIR/dist"

VERSION=$(grep -m 1 '^version = ' "$ROOT_DIR/crates/ui_gui/Cargo.toml" | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"
APP_BUNDLE="$DIST_DIR/UwuLog.app"
OUTPUT_DMG="$DIST_DIR/UwuLog-v${VERSION}-macOS.dmg"

echo "========================================================"
echo " [UWU-LOG] macOS Disk Image (.dmg) Builder"
echo " Version: $VERSION"
echo "========================================================"

# 1. Build App Bundle first if missing
if [ ! -d "$APP_BUNDLE" ]; then
    echo "-> UwuLog.app missing in dist/, building bundle..."
    bash "$SCRIPT_DIR/build-app.sh"
fi

# 2. Prepare staging folder for DMG
STAGING_DMG="$ROOT_DIR/target/dmg_staging"
rm -rf "$STAGING_DMG"
mkdir -p "$STAGING_DMG"

echo "-> Preparing DMG contents..."
cp -R "$APP_BUNDLE" "$STAGING_DMG/"
ln -s /Applications "$STAGING_DMG/Applications"

# 3. Create DMG using hdiutil (native macOS) or create-dmg
if command -v hdiutil >/dev/null 2>&1; then
    echo "-> Creating DMG with hdiutil..."
    rm -f "$OUTPUT_DMG"
    hdiutil create -volname "Uwu Log" \
                   -srcfolder "$STAGING_DMG" \
                   -ov \
                   -format UDZO \
                   "$OUTPUT_DMG"
    
    # Sign DMG if certificate provided
    if [ -n "$APPLE_SIGNING_IDENTITY" ] && command -v codesign >/dev/null 2>&1; then
        echo "-> Signing DMG..."
        codesign --force --sign "$APPLE_SIGNING_IDENTITY" "$OUTPUT_DMG"
    fi

    echo ""
    echo "✅ Successfully created macOS DMG: $OUTPUT_DMG"
    ls -lh "$OUTPUT_DMG"
else
    echo "-> Notice: hdiutil is only available on native macOS."
    echo "   On Linux/Windows, compressing app bundle as tar.gz for macOS transfer..."
    cd "$DIST_DIR"
    tar -czf "UwuLog-v${VERSION}-macOS.tar.gz" "UwuLog.app"
    echo "✅ Created macOS tarball: $DIST_DIR/UwuLog-v${VERSION}-macOS.tar.gz"
fi

rm -rf "$STAGING_DMG"
