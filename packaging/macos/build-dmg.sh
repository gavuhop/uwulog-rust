#!/usr/bin/env bash
# ==============================================================================
# build-dmg.sh - Package native macOS Drag-and-Drop Disk Image (.dmg) (Multi-Arch)
# Supports: arm64 / aarch64 (Apple Silicon), x86_64 (Intel), and universal
# ==============================================================================

set -e

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
            echo "Usage: ./build-dmg.sh [--arch <arm64|x86_64|universal>] [--target <triple>] [--skip-build]"
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

# Detect host architecture if not specified
if [ -z "$ARCH" ]; then
    HOST_ARCH="$(uname -m)"
    case "$HOST_ARCH" in
        arm64|aarch64) ARCH="arm64" ;;
        x86_64) ARCH="x86_64" ;;
        *) ARCH="x86_64" ;;
    esac
fi

case "$ARCH" in
    arm64|aarch64) ARCH="arm64" ;;
    x86_64|x64) ARCH="x86_64" ;;
    universal) ARCH="universal" ;;
esac

VERSION=$(grep -m 1 '^version = ' "$ROOT_DIR/crates/ui_gui/Cargo.toml" | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"
APP_BUNDLE="$DIST_DIR/UwuLog.app"
OUTPUT_DMG="$DIST_DIR/UwuLog-v${VERSION}-macOS-${ARCH}.dmg"

echo "========================================================"
echo " [UWU-LOG] macOS Disk Image (.dmg) Builder"
echo " Version: $VERSION | Arch: $ARCH"
echo "========================================================"

# 1. Build App Bundle first
APP_ARGS=(--arch "$ARCH")
if [ -n "$TARGET" ]; then
    APP_ARGS+=(--target "$TARGET")
fi
if [ -n "$SKIP_BUILD" ]; then
    APP_ARGS+=(--skip-build)
fi

bash "$SCRIPT_DIR/build-app.sh" "${APP_ARGS[@]}"

# 2. Prepare staging folder for DMG
STAGING_DMG="$ROOT_DIR/target/dmg_staging"
rm -rf "$STAGING_DMG"
mkdir -p "$STAGING_DMG"

echo "-> Preparing DMG contents..."
cp -R "$APP_BUNDLE" "$STAGING_DMG/"
ln -s /Applications "$STAGING_DMG/Applications"

# 3. Create DMG using hdiutil (native macOS)
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

    # Create convenient non-versioned alias matching Zed style (e.g. UwuLog-aarch64.dmg)
    ALIAS_DMG="$DIST_DIR/UwuLog-${ARCH}.dmg"
    cp -f "$OUTPUT_DMG" "$ALIAS_DMG" 2>/dev/null || true

    echo ""
    echo "✅ Successfully created macOS DMG: $OUTPUT_DMG"
    ls -lh "$OUTPUT_DMG"
else
    echo "-> Notice: hdiutil is only available on native macOS."
    echo "   Compressing app bundle as portable tarball for macOS transfer..."
    FALLBACK_TAR="$DIST_DIR/UwuLog-v${VERSION}-macOS-${ARCH}.tar.gz"
    cd "$DIST_DIR"
    tar -czf "$FALLBACK_TAR" "UwuLog.app"
    echo "✅ Created macOS tarball: $FALLBACK_TAR"
fi

rm -rf "$STAGING_DMG"
