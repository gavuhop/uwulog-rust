# Kế Hoạch Chi Tiết Phát Triển `uwu-log` (Zed-Style Decoupled Architecture)

Tài liệu chi tiết hóa kế hoạch tái cấu trúc và phát triển dự án `uwu-log` theo mô hình **Client-Agent Decoupled Remoting** (học hỏi từ kiến trúc của **Zed Editor**: `remote_server` + `remote_connection` + `remote`), mang lại khả năng stream log từ xa mạnh mẽ cho **WSL**, **Linux Servers (SSH)** và **Containers**.

---

## 📋 MỤC LỤC CÁC GIAI ĐOẠN

- [Giai Đoạn 1: Xây Dựng Headless Remote Agent (`crates/agent`)](#1-giai-đoạn-1-xây-dựng-headless-remote-agent-cratesagent)
- [Giai Đoạn 2: Xây Dựng Driver Nguồn WSL Trong `crates/core` (`WslSource`)](#2-giai-đoạn-2-xây-dựng-driver-nguồn-wsl-trong-cratescore-wslsource)
- [Giai Đoạn 3: Nâng Cấp Giao Diện Desktop GUI (`crates/gui`)](#3-giai-đoạn-3-nâng-cấp-giao-diện-desktop-gui-cratesgui)
- [Giai Đoạn 4: Chuẩn Hóa WSL CLI Interop & Script Cài Đặt](#4-giai-đoạn-4-chuẩn-hóa-wsl-cli-interop--script-cài-đặt)
- [Giai Đoạn 5: Hoàn Thiện Các Tính Năng Roadmap (UI/UX & Performance)](#5-giai-đoạn-5-hoàn-thiện-các-tính-năng-roadmap-uiux--performance)

---

## 1. GIAI ĐOẠN 1: XÂY DỰNG HEADLESS REMOTE AGENT (`crates/agent`)

> **Mục tiêu**: Tạo binary `uwu-agent` siêu nhẹ (không GUI, pure Rust), chạy trực tiếp trong Linux / WSL / Remote Server để thu thập log tại nguồn và stream về client qua stdio pipe hoặc socket theo định dạng NDJSON.

- [x] Tạo crate `crates/agent` trong Cargo workspace (`uwu-agent`).
- [x] Xử lý các cờ CLI chính cho agent:
  - `--file <path>`: Tail file log cục bộ bằng cơ chế inotify / seek polling.
  - `--cmd "<command>"`: Khởi chạy command trong môi trường Linux và stream stdout/stderr.
  - `--journald [unit]`: Stream log từ systemd journalctl theo định dạng JSON.
- [x] Cơ chế xuất dữ liệu NDJSON (Newline-Delimited JSON) qua stdout: Mỗi log entry được serialize thành 1 dòng JSON chứa `source_id` và `payload`.
- [x] Quản lý lifecycle & tín hiệu: Bắt `SIGINT` / `SIGTERM` để tự động ngắt các tiến trình con sạch sẽ.

---

## 2. GIAI ĐOẠN 2: XÂY DỰNG DRIVER NGUỒN WSL TRONG `crates/core` (`WslSource`)

> **Mục tiêu**: Cung cấp driver `WslSource` chuẩn hóa triển khai trait `LogSource` trong `uwu-core`.

- [x] Tạo module [`crates/core/src/sources/wsl.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/sources/wsl.rs):
  - Định nghĩa `WslSource` với cấu hình: `distro`, `working_dir`, `target_mode` (Command, File, Journald).
  - Tự động spawn tiến trình `wsl.exe -d <distro> -- <command_or_agent>`.
  - Đọc bất đồng bộ stream stdout qua `BufReader` và parse NDJSON vào `RawLogEntry`.
  - Quản lý lifecycle: Đảm bảo khi ngắt kết nối hoặc thoát ứng dụng thì process con trong WSL bị hủy triệt để.
- [x] Export `WslSource` trong [`crates/core/src/sources/mod.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/sources/mod.rs) và [`crates/core/src/lib.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/lib.rs).

---

## 3. GIAI ĐOẠN 3: NÂNG CẤP GIAO DIỆN DESKTOP GUI (`crates/gui`)

> **Mục tiêu**: Tích hợp nguồn WSL vào GUI một cách tự nhiên và trực quan như các ứng dụng Desktop chuyên nghiệp.

- [x] Mở rộng `SourceType` trong [`crates/gui/src/app.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/app.rs):
  - `SourceType::Process` (Local Windows process)
  - `SourceType::File` (Local Windows file)
  - `SourceType::WinEvent` (Windows Event Log)
  - `SourceType::Wsl` (WSL Remote Agent/Process)
- [x] Cập nhật modal cấu hình [`crates/gui/src/ui/launch_modal.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/launch_modal.rs):
  - Thêm lựa chọn **🐧 WSL (Windows Subsystem for Linux)**.
  - Cho phép chọn Distro name (mặc định `Ubuntu`).
  - Cho phép chọn loại nguồn trong WSL: 🚀 Command, 📁 Linux File, 📜 Systemd Journald.
- [x] Cập nhật Header Bar [`crates/gui/src/ui/header.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/header.rs):
  - Hiển thị badge trạng thái `🐧 WSL (<Distro>)` khi đang stream log từ WSL.

---

## 4. GIAI ĐOẠN 4: CHUẨN HÓA WSL CLI INTEROP

> **Mục tiêu**: Đảm bảo lệnh `uwulog` trong terminal WSL hoạt động hoàn hảo và gọi đúng các cờ cấu hình của Windows GUI.

- [x] Cập nhật [`scripts/uwulog`](file:///D:/Learn/Go/uwulog-rust/scripts/uwulog) để hỗ trợ các tham số cấu trúc mới.

---

## 5. GIAI ĐOẠN 5: WORKSPACE PERSISTENCE & AUTOMATED INSTALLER

> **Mục tiêu**: Lưu trữ cấu hình dự án dạng IDE trong Settings Modal và cung cấp bộ cài đặt tự động cho Windows & WSL.

- [x] Tạo module [`crates/core/src/workspace/mod.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/workspace/mod.rs) (`Workspace`, `WorkspaceLocation`, `WorkspaceStore`).
- [x] Tích hợp mục **📁 Project & Workspace** vào [`crates/gui/src/ui/launch_modal.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/launch_modal.rs).
- [x] Hiển thị badge Project Name và hỗ trợ chuyển đổi nhanh trên [`crates/gui/src/ui/header.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/header.rs).
- [x] Xây dựng bộ cài đặt tự động [`installer/install.ps1`](file:///D:/Learn/Go/uwulog-rust/installer/install.ps1), launcher [`installer/uwulog.cmd`](file:///D:/Learn/Go/uwulog-rust/installer/uwulog.cmd) và [`installer/uwulog`](file:///D:/Learn/Go/uwulog-rust/installer/uwulog).

---

## 6. GIAI ĐOẠN 6: HOÀN THIỆN CÁC TÍNH NĂNG ROADMAP (UI/UX & PERFORMANCE)

- [ ] **Tùy chỉnh cột**: Lưu trạng thái cột hiển thị vào cấu hình.
- [ ] **Lọc nhanh bằng chuột**: Click vào bất kỳ cell nào để append query vào search bar.
- [ ] **Highlight hàng & từ khóa**: Tô màu nổi bật dòng log quan trọng hoặc từ khóa trên bảng.
- [ ] **Unfiltered Context Jump**: Nhảy mượt mà từ Filtered View sang Unfiltered View tại đúng vị trí lỗi.

---

## 📊 TIẾN ĐỘ THỰC HIỆN TỔNG QUAN

| Hạng Mục | Độ Ưu Tiên | Trạng Thái | Người Phụ Trách |
| :--- | :--- | :--- | :--- |
| **1. Crate `uwu-agent` (`crates/agent`)** | 🔴 Rất Cao | ✅ Hoàn Thành | Antigravity Pair Dev |
| **2. Driver `WslSource` (`crates/core`)** | 🔴 Rất Cao | ✅ Hoàn Thành | Antigravity Pair Dev |
| **3. GUI Launch Modal & Source Selector** | 🟡 Cao | ✅ Hoàn Thành | Antigravity Pair Dev |
| **4. WSL CLI Interop Script** | 🟡 Cao | ✅ Hoàn Thành | Antigravity Pair Dev |
| **5. Workspace Persistence & Installer** | 🟡 Cao | ✅ Hoàn Thành | Antigravity Pair Dev |
| **6. Roadmap Features (Columns, Highlight, Context)** | 🟢 Trung Bình | 📋 Đang Lên Kế Hoạch | Antigravity Pair Dev |