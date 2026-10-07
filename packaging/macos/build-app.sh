#!/usr/bin/env bash
# ==============================================================================
# build-app.sh - Package native macOS UwuLog.app Bundle (Multi-Arch)
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
            echo "Usage: ./build-app.sh [--arch <arm64|x86_64|universal>] [--target <triple>] [--skip-build]"
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

# Normalize architecture and default target triple
case "$ARCH" in
    arm64|aarch64)
        ARCH="arm64"
        DEFAULT_TARGET="aarch64-apple-darwin"
        ;;
    x86_64|x64)
        ARCH="x86_64"
        DEFAULT_TARGET="x86_64-apple-darwin"
        ;;
    universal)
        ARCH="universal"
        DEFAULT_TARGET=""
        ;;
    *)
        DEFAULT_TARGET=""
        ;;
esac

TARGET="${TARGET:-$DEFAULT_TARGET}"
VERSION=$(grep -m 1 '^version = ' "$ROOT_DIR/crates/ui_gui/Cargo.toml" | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"

echo "========================================================"
echo " [UWU-LOG] macOS App Bundle Builder (UwuLog.app)"
echo " Version: $VERSION | Arch: $ARCH | Target: ${TARGET:-universal}"
echo "========================================================"

# 1. Build release binaries
if [ -z "$SKIP_BUILD" ]; then
    cd "$ROOT_DIR"
    if [ "$ARCH" = "universal" ]; then
        echo "-> Building dual architectures for universal binary..."
        cargo build --release --target aarch64-apple-darwin --bin uwu-gui --bin uwu-agent --bin uwulog
        cargo build --release --target x86_64-apple-darwin --bin uwu-gui --bin uwu-agent --bin uwulog
    elif [ -n "$TARGET" ]; then
        echo "-> Building release binaries for target $TARGET..."
        cargo build --release --target "$TARGET" --bin uwu-gui --bin uwu-agent --bin uwulog
    else
        echo "-> Building release binaries natively..."
        cargo build --release --bin uwu-gui --bin uwu-agent --bin uwulog
    fi
else
    echo "-> Skipping cargo build (--skip-build specified)..."
fi

# 2. Determine binary sources
BIN_DIR="$ROOT_DIR/target/release"
if [ "$ARCH" != "universal" ]; then
    if [ -n "$TARGET" ] && [ -d "$ROOT_DIR/target/$TARGET/release" ]; then
        BIN_DIR="$ROOT_DIR/target/$TARGET/release"
    elif [ -d "$ROOT_DIR/target/release" ]; then
        BIN_DIR="$ROOT_DIR/target/release"
    fi
fi

# 3. Prepare UwuLog.app structure
APP_BUNDLE="$DIST_DIR/UwuLog.app"
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources/bin"
mkdir -p "$DIST_DIR"

# 4. Copy or combine binaries into bundle
if [ "$ARCH" = "universal" ] && command -v lipo >/dev/null 2>&1; then
    echo "-> Creating Universal macOS binaries using lipo..."
    ARM_DIR="$ROOT_DIR/target/aarch64-apple-darwin/release"
    X86_DIR="$ROOT_DIR/target/x86_64-apple-darwin/release"

    lipo -create "$ARM_DIR/uwu-gui" "$X86_DIR/uwu-gui" -output "$APP_BUNDLE/Contents/MacOS/uwu-gui"
    lipo -create "$ARM_DIR/uwu-agent" "$X86_DIR/uwu-agent" -output "$APP_BUNDLE/Contents/Resources/bin/uwu-agent"
    lipo -create "$ARM_DIR/uwulog" "$X86_DIR/uwulog" -output "$APP_BUNDLE/Contents/Resources/bin/uwulog"
else
    echo "-> Copying binaries from: $BIN_DIR"
    cp "$BIN_DIR/uwu-gui" "$APP_BUNDLE/Contents/MacOS/"
    cp "$BIN_DIR/uwu-agent" "$APP_BUNDLE/Contents/Resources/bin/"
    cp "$BIN_DIR/uwulog" "$APP_BUNDLE/Contents/Resources/bin/"
fi

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
echo "✅ Successfully built macOS bundle: $APP_BUNDLE (Arch: $ARCH)"
