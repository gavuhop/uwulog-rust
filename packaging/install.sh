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
    TARBALL_NAME="uwulog-v${VERSION}-linux-${ARCH_LINUX}-portable.tar.gz"
    DOWNLOAD_URL="${GITHUB_URL}/releases/download/v${VERSION}/${TARBALL_NAME}"
    EXTRACTED_DIR="$TMP_DIR/uwulog-v${VERSION}-linux-${ARCH_LINUX}-portable"

    # Determine potential repository root if run locally
    SCRIPT_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)"
    REPO_DIR=""
    if [ -f "$SCRIPT_DIR/../Cargo.toml" ] && [ -d "$SCRIPT_DIR/../packaging/assets" ]; then
        REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
    elif [ -f "Cargo.toml" ] && [ -d "packaging/assets" ]; then
        REPO_DIR="$(pwd)"
    fi

    echo "==> Downloading $DOWNLOAD_URL..."
    if curl -fL "$DOWNLOAD_URL" -o "$TMP_DIR/$TARBALL_NAME" 2>/dev/null; then
        echo "==> Extracting archive..."
        tar -xzf "$TMP_DIR/$TARBALL_NAME" -C "$TMP_DIR"
    else
        echo "==> Online release asset not found ($DOWNLOAD_URL)."
        echo "==> Checking for local build or archive..."

        if [ -n "$REPO_DIR" ] && [ -f "$REPO_DIR/dist/$TARBALL_NAME" ]; then
            echo "  -> Found local tarball at $REPO_DIR/dist/$TARBALL_NAME"
            tar -xzf "$REPO_DIR/dist/$TARBALL_NAME" -C "$TMP_DIR"
        elif [ -n "$REPO_DIR" ] && { [ -f "$REPO_DIR/target/release/uwu-gui" ] || [ -f "$REPO_DIR/target/debug/uwu-gui" ]; }; then
            BIN_SOURCE="$REPO_DIR/target/release"
            if [ ! -f "$BIN_SOURCE/uwu-gui" ]; then
                BIN_SOURCE="$REPO_DIR/target/debug"
            fi
            echo "  -> Found compiled binaries in $BIN_SOURCE, installing directly from local build..."
            mkdir -p "$EXTRACTED_DIR/bin"
            cp -f "$BIN_SOURCE/uwu-gui" "$EXTRACTED_DIR/bin/" 2>/dev/null || true
            cp -f "$BIN_SOURCE/uwu-agent" "$EXTRACTED_DIR/bin/" 2>/dev/null || true
            cp -f "$BIN_SOURCE/uwulog" "$EXTRACTED_DIR/bin/" 2>/dev/null || true

            # Copy packaging assets
            cp -f "$REPO_DIR/packaging/assets/uwulog.desktop" "$EXTRACTED_DIR/"
            cp -f "$REPO_DIR/packaging/assets/icon_512.png" "$EXTRACTED_DIR/uwulog.png"
            mkdir -p "$EXTRACTED_DIR/icons/hicolor"
            for size in 16 32 48 64 128 256 512; do
                if [ -f "$REPO_DIR/packaging/assets/icon_${size}.png" ]; then
                    mkdir -p "$EXTRACTED_DIR/icons/hicolor/${size}x${size}/apps"
                    cp -f "$REPO_DIR/packaging/assets/icon_${size}.png" "$EXTRACTED_DIR/icons/hicolor/${size}x${size}/apps/uwulog.png"
                fi
            done
        else
            echo "❌ Error: Could not download $DOWNLOAD_URL and no local binary found."
            echo "   If building from source, run: cargo build --release"
            exit 1
        fi
    fi

    echo "==> Installing to $INSTALL_DIR..."
    mkdir -p "$INSTALL_DIR/bin"
    cp -f "$EXTRACTED_DIR/bin/"* "$INSTALL_DIR/bin/"
    chmod 755 "$INSTALL_DIR/bin/uwulog" \
              "$INSTALL_DIR/bin/uwu-gui" \
              "$INSTALL_DIR/bin/uwu-agent"

    # Install desktop entry and icons
    mkdir -p "$INSTALL_DIR/share/applications"
    mkdir -p "$INSTALL_DIR/share/pixmaps"
    cp -f "$EXTRACTED_DIR/uwulog.desktop" "$INSTALL_DIR/share/applications/"
    chmod 644 "$INSTALL_DIR/share/applications/uwulog.desktop"

    # If installed into user/custom prefix (like ~/.local), resolve absolute paths for desktop Exec & Icon
    if [ "$INSTALL_DIR" != "/usr" ]; then
        sed -i "s|^Exec=uwu-gui|Exec=$INSTALL_DIR/bin/uwu-gui|g" "$INSTALL_DIR/share/applications/uwulog.desktop" 2>/dev/null || true
        sed -i "s|^Icon=uwulog|Icon=$INSTALL_DIR/share/icons/hicolor/512x512/apps/uwulog.png|g" "$INSTALL_DIR/share/applications/uwulog.desktop" 2>/dev/null || true
    fi

    if [ -d "$EXTRACTED_DIR/icons/hicolor" ]; then
        mkdir -p "$INSTALL_DIR/share/icons/hicolor"
        cp -r "$EXTRACTED_DIR/icons/hicolor/"* "$INSTALL_DIR/share/icons/hicolor/"
    fi
    mkdir -p "$INSTALL_DIR/share/icons/hicolor/512x512/apps"
    cp -f "$EXTRACTED_DIR/uwulog.png" "$INSTALL_DIR/share/icons/hicolor/512x512/apps/uwulog.png"
    chmod 644 "$INSTALL_DIR/share/icons/hicolor/512x512/apps/uwulog.png"

    if [ -f "$EXTRACTED_DIR/icons/hicolor/256x256/apps/uwulog.png" ]; then
        cp -f "$EXTRACTED_DIR/icons/hicolor/256x256/apps/uwulog.png" "$INSTALL_DIR/share/pixmaps/uwulog.png"
    else
        cp -f "$EXTRACTED_DIR/uwulog.png" "$INSTALL_DIR/share/pixmaps/uwulog.png"
    fi
    chmod 644 "$INSTALL_DIR/share/pixmaps/uwulog.png"

    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "$INSTALL_DIR/share/applications" || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f "$INSTALL_DIR/share/icons/hicolor" || true
    fi
    if command -v gtk4-update-icon-cache >/dev/null 2>&1; then
        gtk4-update-icon-cache -q -t -f "$INSTALL_DIR/share/icons/hicolor" || true
    fi
    if command -v kbuildsycoca6 >/dev/null 2>&1; then
        kbuildsycoca6 --noincremental 2>/dev/null || true
    elif command -v kbuildsycoca5 >/dev/null 2>&1; then
        kbuildsycoca5 --noincremental 2>/dev/null || true
    fi

elif [ "$PLATFORM" = "macos" ]; then
    DMG_NAME="UwuLog-v${VERSION}-macOS-${ARCH_MACOS}.dmg"
    DOWNLOAD_URL="${GITHUB_URL}/releases/download/v${VERSION}/${DMG_NAME}"

    SCRIPT_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)"
    REPO_DIR=""
    if [ -f "$SCRIPT_DIR/../Cargo.toml" ] && [ -d "$SCRIPT_DIR/../packaging/assets" ]; then
        REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
    elif [ -f "Cargo.toml" ] && [ -d "packaging/assets" ]; then
        REPO_DIR="$(pwd)"
    fi

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
    elif [ -n "$REPO_DIR" ] && [ -d "$REPO_DIR/dist/UwuLog.app" ]; then
        echo "==> Found local UwuLog.app in dist/, copying to /Applications..."
        rm -rf "/Applications/UwuLog.app"
        cp -R "$REPO_DIR/dist/UwuLog.app" /Applications/
    elif [ -n "$REPO_DIR" ] && [ -f "$REPO_DIR/dist/$DMG_NAME" ]; then
        echo "==> Found local DMG in dist/, mounting..."
        MOUNT_DIR="$TMP_DIR/mount"
        mkdir -p "$MOUNT_DIR"
        hdiutil attach "$REPO_DIR/dist/$DMG_NAME" -mountpoint "$MOUNT_DIR" -nobrowse -quiet
        rm -rf "/Applications/UwuLog.app"
        cp -R "$MOUNT_DIR/UwuLog.app" /Applications/
        hdiutil detach "$MOUNT_DIR" -quiet
    else
        echo "==> Checking fallback tarball..."
        TARBALL_NAME="UwuLog-v${VERSION}-macOS-${ARCH_MACOS}.tar.gz"
        FALLBACK_URL="${GITHUB_URL}/releases/download/v${VERSION}/${TARBALL_NAME}"
        if curl -fL "$FALLBACK_URL" -o "$TMP_DIR/$TARBALL_NAME" 2>/dev/null; then
            tar -xzf "$TMP_DIR/$TARBALL_NAME" -C /Applications/
        elif [ -n "$REPO_DIR" ] && [ -f "$REPO_DIR/dist/$TARBALL_NAME" ]; then
            tar -xzf "$REPO_DIR/dist/$TARBALL_NAME" -C /Applications/
        else
            echo "❌ Error: Could not download macOS package and no local asset found in dist/."
            exit 1
        fi
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
