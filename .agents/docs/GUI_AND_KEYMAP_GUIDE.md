# Hướng dẫn GUI (`egui`), Theme, Icons & Keymap 4 Tầng (`GUI_AND_KEYMAP_GUIDE.md`)

Tài liệu này bao phủ tầng trình diễn `crates/ui_gui`, hệ thống icon vector `crates/icons`, và bộ quản lý phím tắt `crates/core_keymap`.

---

## 1. Cấu trúc Module `crates/ui_gui`

| Đường dẫn | Vai trò |
| :--- | :--- |
| [src/main.rs](../../crates/ui_gui/src/main.rs) | Khởi tạo Tokio runtime và `eframe::run_native` với `UwuGuiApp`. |
| [src/app/mod.rs](../../crates/ui_gui/src/app/mod.rs) | `UwuGuiApp` shell, vòng lặp `eframe::App::ui`, `handle_keybindings`, `dispatch_action`, và các hàm `spawn_*`. |
| [src/app/workspace_manager.rs](../../crates/ui_gui/src/app/workspace_manager.rs) & [overlay_manager.rs](../../crates/ui_gui/src/app/overlay_manager.rs) | Quản lý nhiều phiên workspace/tabs và ngăn xếp overlay (`OverlayLayer` / `OverlayStack`). |
| [src/actions.rs](../../crates/ui_gui/src/actions.rs) | Định nghĩa `AppAction` (UI $\rightarrow$ App) và `AppEvent` (Worker $\rightarrow$ UI). |
| [src/reducer.rs](../../crates/ui_gui/src/reducer.rs) | Hàm `reduce(app, event)` xử lý tập trung các `AppEvent` trên UI thread trong $O(1)$. |
| [src/session.rs](../../crates/ui_gui/src/session.rs) & `src/state/` | `GuiSession`, trạng thái tìm kiếm, cột, viewport, autocomplete, lịch sử, và unfiltered stream. |
| `src/views/` & `src/components/` | Các thành phần giao diện `egui` thuần túy (header, table, detail inspector, modals, remote server picker). |
| `src/theme/` | Bảng token màu ([tokens.rs](../../crates/ui_gui/src/theme/tokens.rs)), presets, loader, ANSI highlighter. |

### Quy tắc Luồng Một Chiều & 60 FPS
- Không chạy `std::process::Command` đồng bộ hoặc quét dữ liệu trực tiếp trong `views/`.
- View phát `AppAction`; `UwuGuiApp::dispatch_action` xử lý hoặc spawn tác vụ lên `self.rt` (`tokio::runtime::Handle`) rồi gửi kết quả về qua `self.event_tx` dưới dạng `AppEvent` để `reducer::reduce` áp dụng.

---

## 2. Theme Tokens & Vector SVG Icons

- **Theme Tokens ([tokens.rs](../../crates/ui_gui/src/theme/tokens.rs))**: Dùng các struct chuẩn (`SurfaceColors`, `BorderColors`, `TextColors`, `SelectionColors`, `StatusColors`, `LogTableColors`, `ControlColors`, `AnsiColors`) thay vì hardcode `Color32::from_rgb(...)` trong `views/` hoặc `components/`.
- **Vector Icons ([crates/icons/src/lib.rs](../../crates/icons/src/lib.rs))**:
  - Mọi file `.svg` đặt tại `assets/icons/<name>.svg` và được `crates/icons/build.rs` biên dịch tĩnh thành `VectorShape`.
  - Khi thêm icon mới, đăng ký biến thể vào `IconName` (`file_stem`, `path`, `ALL`) và cập nhật `test_icon_count`. Kiểm thử với `./.agents/scripts/harness.sh test-tier 0`.

---

## 3. Hệ thống Phím tắt 4 Tầng Kiểu Zed (`crates/core_keymap`)

`uwu-core-keymap` tách biệt hoàn toàn logic phím tắt khỏi `egui`:

1. **Tầng 1 — Keystroke & Key Parser** ([key.rs](../../crates/core_keymap/src/key.rs), [keystroke.rs](../../crates/core_keymap/src/keystroke.rs)): Chuẩn hóa `Key` + modifiers (`ctrl`, `alt`, `shift`, `super`) và adapter `egui`.
2. **Tầng 2 — Dynamic Context Hierarchy** ([context.rs](../../crates/core_keymap/src/context.rs)): 6 ngữ cảnh chuẩn kế thừa phân cấp:
   - `RemoteServers` $\rightarrow$ `Modal` $\rightarrow$ `Global`
   - `Autocomplete` $\rightarrow$ `SearchInput` $\rightarrow$ `Global`
   - `Table` $\rightarrow$ `Global`
3. **Tầng 3 — Semantic Action Registry** ([action.rs](../../crates/core_keymap/src/action.rs)): Định nghĩa 36 `KeyAction` với tên canonical dạng `namespace::Action` (ví dụ `workspace::ToggleProjectPicker`, `search::Focus`), ánh xạ sang `AppAction` qua `KeyActionExt` tại [crates/ui_gui/src/keymap/mod.rs](../../crates/ui_gui/src/keymap/mod.rs).
4. **Tầng 4 — User Config & Delta Persistence** ([config.rs](../../crates/core_keymap/src/config.rs), [manager.rs](../../crates/core_keymap/src/manager.rs)): `KeymapManager` giữ 46 default bindings, cache nhãn `O(1)` cho UI, hỗ trợ override/unbind qua `keymap.json`.

---

## 4. Headless GUI Testing

Mọi kiểm thử điều phối action, keybinding, latch, reverse pagination và unfiltered stream đều chạy không cần mở cửa sổ OS thông qua `create_test_app()` tại [crates/ui_gui/src/app/tests/test_helpers.rs](../../crates/ui_gui/src/app/tests/test_helpers.rs):

```bash
./.agents/scripts/harness.sh test-tier 3    # Kiểm thử core_workspace & core_keymap
./.agents/scripts/harness.sh test-tier 4    # Kiểm thử 157 tests của uwu-ui-gui + uwulog + uwu-agent
```
