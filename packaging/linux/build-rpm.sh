#!/usr/bin/env bash
# ==============================================================================
# build-rpm.sh - Build native Fedora / RHEL / openSUSE (.rpm) package for Uwu Log Viewer
# Supports: x86_64, aarch64
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
            echo "Usage: ./build-rpm.sh [--arch <x86_64|aarch64>] [--target <triple>] [--skip-build]"
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
RELEASE="1"

echo "========================================================"
echo " [UWU-LOG] Linux RPM Package Builder (.rpm - Fedora/RHEL)"
echo " Version: $VERSION | Arch: $ARCH | Target: ${TARGET:-host}"
echo "========================================================"

if ! command -v rpmbuild >/dev/null 2>&1; then
    echo "❌ Error: 'rpmbuild' is not installed."
    echo "   Install it on Fedora with: sudo dnf install -y rpm-build"
    exit 1
fi

# 1. Determine binary directory & cargo flags
HOST_ARCH="$(uname -m)"
CARGO_FLAGS=()
BIN_DIR="$ROOT_DIR/target/release"

if [ -n "$TARGET" ] && { [ "$ARCH" = "aarch64" ] && [ "$HOST_ARCH" = "x86_64" ]; }; then
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
    if [ ! -f "$BIN_DIR/uwu-gui" ] && [ -f "$ROOT_DIR/target/release/uwu-gui" ]; then
        BIN_DIR="$ROOT_DIR/target/release"
    fi
fi

echo "   Using binaries from: $BIN_DIR"

if [ ! -f "$BIN_DIR/uwu-gui" ]; then
    echo "❌ Error: Binaries not found in $BIN_DIR!"
    exit 1
fi

# 3. Prepare RPM build tree
STAGE_DIR="${TMPDIR:-/tmp}/rpm_staging/uwulog_${VERSION}_${ARCH}"
RPM_TOPDIR="$STAGE_DIR/rpmbuild"
rm -rf "$STAGE_DIR"
mkdir -p "$RPM_TOPDIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS,tmp}
mkdir -p "$DIST_DIR"

# 4. Generate RPM .spec file
SPEC_FILE="$RPM_TOPDIR/SPECS/uwulog.spec"
cat << EOF > "$SPEC_FILE"
%define _build_id_links none
%define debug_package %{nil}
%global __os_install_post %{nil}

Name:           uwulog
Version:        $VERSION
Release:        $RELEASE%{?dist}
Summary:        High-performance log viewer and workspace monitor
License:        MIT
URL:            https://github.com/gavuhop/uwulog-rust
BuildArch:      $ARCH

%description
A blazing-fast log viewer, live streaming monitor, and remote agent
supporting structured search, filters, journald, and file tailing.

%install
mkdir -p %{buildroot}%{_bindir}
cp -a "$BIN_DIR/uwu-gui" %{buildroot}%{_bindir}/
cp -a "$BIN_DIR/uwu-tui" %{buildroot}%{_bindir}/
cp -a "$BIN_DIR/uwu-agent" %{buildroot}%{_bindir}/
cp -a "$BIN_DIR/uwulog" %{buildroot}%{_bindir}/
chmod 755 %{buildroot}%{_bindir}/*

mkdir -p %{buildroot}%{_datadir}/applications
cp -a "$PACKAGING_DIR/assets/uwulog.desktop" %{buildroot}%{_datadir}/applications/
chmod 644 %{buildroot}%{_datadir}/applications/uwulog.desktop

for size in 16 32 48 64 128 256 512; do
    icon_dir="%{buildroot}%{_datadir}/icons/hicolor/\${size}x\${size}/apps"
    mkdir -p "\$icon_dir"
    if [ -f "$PACKAGING_DIR/assets/icon_\${size}.png" ]; then
        cp -a "$PACKAGING_DIR/assets/icon_\${size}.png" "\$icon_dir/uwulog.png"
        chmod 644 "\$icon_dir/uwulog.png"
    fi
done

mkdir -p %{buildroot}%{_datadir}/pixmaps
cp -a "$PACKAGING_DIR/assets/icon_256.png" %{buildroot}%{_datadir}/pixmaps/uwulog.png
chmod 644 %{buildroot}%{_datadir}/pixmaps/uwulog.png

%post
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q %{_datadir}/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f %{_datadir}/icons/hicolor || true
fi
if command -v gtk4-update-icon-cache >/dev/null 2>&1; then
    gtk4-update-icon-cache -q -t -f %{_datadir}/icons/hicolor || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental 2>/dev/null || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    kbuildsycoca5 --noincremental 2>/dev/null || true
fi

%postun
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q %{_datadir}/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f %{_datadir}/icons/hicolor || true
fi
if command -v gtk4-update-icon-cache >/dev/null 2>&1; then
    gtk4-update-icon-cache -q -t -f %{_datadir}/icons/hicolor || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental 2>/dev/null || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    kbuildsycoca5 --noincremental 2>/dev/null || true
fi

%files
%{_bindir}/uwu-gui
%{_bindir}/uwu-tui
%{_bindir}/uwu-agent
%{_bindir}/uwulog
%{_datadir}/applications/uwulog.desktop
%{_datadir}/icons/hicolor/*/apps/uwulog.png
%{_datadir}/pixmaps/uwulog.png
EOF

# 7. Build RPM
rpmbuild -bb \
    --define "_topdir $RPM_TOPDIR" \
    --define "_tmppath $RPM_TOPDIR/tmp" \
    --target "$ARCH" \
    "$SPEC_FILE"

RPM_FILE=$(find "$RPM_TOPDIR/RPMS" -name "*.rpm" | head -n 1)
if [ -f "$RPM_FILE" ]; then
    cp -f "$RPM_FILE" "$DIST_DIR/"
    OUTPUT_RPM="$DIST_DIR/$(basename "$RPM_FILE")"
    rm -rf "$STAGE_DIR"
    echo ""
    echo "✅ Successfully built Fedora/RPM package: $OUTPUT_RPM"
    ls -lh "$OUTPUT_RPM"
else
    echo "❌ Error: RPM build failed to generate an RPM file."
    exit 1
fi
