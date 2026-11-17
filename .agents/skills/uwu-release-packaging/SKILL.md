---
name: uwu-release-packaging
description: >-
  Build, package, and validate cross-platform release artifacts and CI workflows for uwulog-rust
  (Linux tar.gz/deb/rpm, macOS app/dmg, Windows Inno Setup/portable zip, install.sh, and
  .github/workflows/release.yml). Activate when updating packaging scripts, release profiles,
  or GitHub release automation.
---

# 📦 `uwu-release-packaging` — Cross-Platform Build & Release Runbook

---

## 1. Release Binaries & Build Profiles

All platform packages bundle the **3 workspace binaries** defined in [Cargo.toml](../../../Cargo.toml):
1. `uwulog` (`crates/cli`) — universal launcher executable
2. `uwu-gui` (`crates/ui_gui`) — native desktop GUI
3. `uwu-agent` (`crates/cli_agent`) — headless remote log streaming agent

Profiles in root `Cargo.toml`:
- `--profile release-dev`: fast local optimized dev builds (`lto = "off"`, `codegen-units = 16`, `incremental = true`).
- `--release`: production packaging builds (`opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`).

---

## 2. Packaging Assets & Scripts Map

- **CI Pipeline**: [.github/workflows/release.yml](../../../.github/workflows/release.yml) (Windows `x64`/`arm64`, Linux `x86_64`/`aarch64`, macOS `arm64`/`x86_64`, triggered on tag `v*` or `workflow_dispatch`).
- **Linux**:
  - `packaging/linux/build-tarball.sh` (`uwulog-v*-linux-<arch>-portable.tar.gz`)
  - `packaging/linux/build-deb.sh` (`uwulog_*_<deb_arch>.deb`)
  - `packaging/linux/build-rpm.sh` (`uwulog-*.<arch>.rpm`)
- **macOS**: `packaging/macos/Info.plist`, `packaging/macos/build-app.sh`, `packaging/macos/build-dmg.sh`
- **Windows**: `packaging/windows/setup.iss`, `packaging/windows/build-windows.ps1`
- **Installer & Desktop Metadata**: `packaging/install.sh`, `packaging/assets/uwulog.desktop`, `packaging/assets/generate_icons.py`

---

## 3. Release Verification Checklist

1. **Sync Versions**: Ensure `version` in `crates/cli/Cargo.toml`, `crates/ui_gui/Cargo.toml`, `crates/cli_agent/Cargo.toml`, `packaging/macos/Info.plist`, and `packaging/windows/setup.iss` match the target release tag.
2. **Pre-Release Gate**:
   ```bash
   ./.agents/scripts/harness.sh verify
   cargo build --release --bin uwulog --bin uwu-gui --bin uwu-agent
   ```
3. **Linux Portable Smoke Test** (when on Linux):
   ```bash
   bash packaging/linux/build-tarball.sh --skip-build
   ```
   Verify `dist/uwulog-v*-linux-*-portable.tar.gz` contains `uwulog`, `uwu-gui`, and `uwu-agent`, and verify archive naming matches `packaging/install.sh` and `.github/workflows/release.yml`.
