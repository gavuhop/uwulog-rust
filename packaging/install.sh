#!/bin/sh
# ==============================================================================
# Uwu Log Universal Web Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/gavuhop/uwulog-rust/main/packaging/install.sh | sh
# ==============================================================================

set -e

REPO="gavuhop/uwulog-rust"
GITHUB_URL="https://github.com/${REPO}"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux*)     PLATFORM="linux" ;;
    Darwin*)    PLATFORM="macos" ;;
    *)
        echo "Error: Unsupported operating system: $OS"
        echo "For Windows, please download the installer (.exe) or portable (.zip) from:"
        echo "  $GITHUB_URL/releases/latest"
        exit 1
        ;;
esac

# Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)
        ARCH_LINUX="x86_64"
        ARCH_MACOS="x86_64"
        ;;
    aarch64|arm64)
        ARCH_LINUX="aarch64"
        ARCH_MACOS="arm64"
        ;;
    *)
        echo "Error: Unsupported CPU architecture: $ARCH"
        exit 1
        ;;
esac

echo "==> Installing Uwu Log for $PLATFORM ($ARCH)..."

# Fetch latest release version from GitHub API or tag
VERSION="${UWULOG_VERSION:-}"
if [ -z "$VERSION" ]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | sed -E 's/.*"v?([^"]+)".*/\1/' || true)
fi
VERSION="${VERSION:-0.1.0}"
echo "==> Target Version: v${VERSION}"

# Determine target directory
if [ "$(id -u)" -eq 0 ]; then
    INSTALL_DIR="${UWULOG_INSTALL_DIR:-/usr/local}"
else
    INSTALL_DIR="${UWULOG_INSTALL_DIR:-$HOME/.local}"
fi

TMP_DIR="$(mktemp -d -t uwulog-install-XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ "$PLATFORM" = "linux" ]; then
    TARBALL_NAME="uwulog-v${VERSION}-linux-${ARCH_LINUX}.tar.gz"
    DOWNLOAD_URL="${GITHUB_URL}/releases/download/v${VERSION}/${TARBALL_NAME}"

    echo "==> Downloading $DOWNLOAD_URL..."
    if ! curl -fL "$DOWNLOAD_URL" -o "$TMP_DIR/$TARBALL_NAME" 2>/dev/null; then
        echo "==> Release asset not found online, attempting local repository copy if present..."
        # Fallback if run locally inside repo
        SCRIPT_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)"
        if [ -f "$SCRIPT_DIR/../dist/$TARBALL_NAME" ]; then
            cp "$SCRIPT_DIR/../dist/$TARBALL_NAME" "$TMP_DIR/$TARBALL_NAME"
        else
            echo "Error: Could not download $DOWNLOAD_URL"
            exit 1
        fi
    fi

    echo "==> Extracting archive..."
    tar -xzf "$TMP_DIR/$TARBALL_NAME" -C "$TMP_DIR"
    EXTRACTED_DIR="$TMP_DIR/uwulog-v${VERSION}-linux-${ARCH_LINUX}"

    echo "==> Installing to $INSTALL_DIR..."
    mkdir -p "$INSTALL_DIR/bin"
    cp -f "$EXTRACTED_DIR/bin/"* "$INSTALL_DIR/bin/"
    chmod 755 "$INSTALL_DIR/bin/uwulog" \
              "$INSTALL_DIR/bin/uwu-gui" \
              "$INSTALL_DIR/bin/uwu-tui" \
              "$INSTALL_DIR/bin/uwu-agent"

    # Install desktop entry and icons
    mkdir -p "$INSTALL_DIR/share/applications"
    mkdir -p "$INSTALL_DIR/share/icons/hicolor/512x512/apps"
    cp -f "$EXTRACTED_DIR/uwulog.desktop" "$INSTALL_DIR/share/applications/"
    cp -f "$EXTRACTED_DIR/uwulog.png" "$INSTALL_DIR/share/icons/hicolor/512x512/apps/uwulog.png"

    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "$INSTALL_DIR/share/applications" || true
    fi

elif [ "$PLATFORM" = "macos" ]; then
    DMG_NAME="UwuLog-v${VERSION}-macOS-${ARCH_MACOS}.dmg"
    DOWNLOAD_URL="${GITHUB_URL}/releases/download/v${VERSION}/${DMG_NAME}"

    echo "==> Downloading $DOWNLOAD_URL..."
    if curl -fL "$DOWNLOAD_URL" -o "$TMP_DIR/$DMG_NAME" 2>/dev/null; then
        echo "==> Mounting DMG..."
        MOUNT_DIR="$TMP_DIR/mount"
        mkdir -p "$MOUNT_DIR"
        hdiutil attach "$TMP_DIR/$DMG_NAME" -mountpoint "$MOUNT_DIR" -nobrowse -quiet

        echo "==> Copying UwuLog.app to /Applications..."
        rm -rf "/Applications/UwuLog.app"
        cp -R "$MOUNT_DIR/UwuLog.app" /Applications/

        hdiutil detach "$MOUNT_DIR" -quiet
    else
        echo "==> DMG download failed or non-macOS environment. Checking fallback tarball..."
        TARBALL_NAME="UwuLog-v${VERSION}-macOS-${ARCH_MACOS}.tar.gz"
        FALLBACK_URL="${GITHUB_URL}/releases/download/v${VERSION}/${TARBALL_NAME}"
        curl -fL "$FALLBACK_URL" -o "$TMP_DIR/$TARBALL_NAME"
        tar -xzf "$TMP_DIR/$TARBALL_NAME" -C /Applications/
    fi

    # CLI symlink
    mkdir -p "$INSTALL_DIR/bin"
    ln -sf "/Applications/UwuLog.app/Contents/Resources/bin/uwulog" "$INSTALL_DIR/bin/uwulog"
fi

echo ""
echo "========================================================"
echo " ✅ Uwu Log v${VERSION} was successfully installed!"
echo "========================================================"
echo " Installed binaries at: $INSTALL_DIR/bin"
echo ""
case ":$PATH:" in
    *":$INSTALL_DIR/bin:"*) ;;
    *)
        echo "⚠️ Note: $INSTALL_DIR/bin is not in your PATH."
        echo "   Add it by running:  export PATH=\"$INSTALL_DIR/bin:\$PATH\""
        echo ""
        ;;
esac
echo " Run 'uwulog --help' or 'uwulog' to get started!"
