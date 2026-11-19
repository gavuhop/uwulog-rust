---
name: uwu-gui-component-dev
description: >-
  Build or modify Desktop GUI views, components, theme tokens, SVG vector icons, and Zed-style
  4-layer keybindings in uwu-ui-gui, uwu-core-keymap, and uwu-icons. Activate when editing
  crates/ui_gui, crates/core_keymap, crates/icons, or assets/icons.
---

# 🎨 `uwu-gui-component-dev` — `egui`, Theme, Icons & Keymap Runbook

---

## 1. Unidirectional GUI State Loop (`crates/ui_gui`)

Inspect `crates/ui_gui/src/app/mod.rs` and `actions.rs` via `./.agents/scripts/harness.sh ast-skeleton` before editing.

1. **Views & Components (`src/views/`, `src/components/`)**: Emit `AppAction` (`src/actions.rs`) via `ActionContext` or direct dispatch; never run blocking I/O or spawn heavy scans directly inside rendering code.
2. **Dispatch (`src/app/mod.rs`)**: `UwuGuiApp::dispatch_action` handles synchronous UI mutations (`OverlayManager`, `WorkspaceManager`) and spawns async search/pagination/source work on `self.rt` (`tokio::runtime::Handle`), passing `self.event_tx.clone()`.
3. **Async Reducer (`src/reducer.rs`)**: Background workers emit `AppEvent` (`SearchResultsReady`, `IncrementalLogsReady`, `IncrementalUnfilteredReady`, `ReversePaginationReady`); `reducer::reduce` applies them to `GuiSession` in $O(1)$ on the UI thread.
4. **60 FPS Budget (`< 16.6ms`/frame)**: Keep frozen view (`is_auto_scroll == false`) at 0% scan CPU; keep table scrolling virtualized (`views/table/`).

---

## 2. Theme Tokens (`src/theme/tokens.rs`)

- Always pull colors from the active `Theme` structs (`SurfaceColors`, `BorderColors`, `TextColors`, `SelectionColors`, `StatusColors`, `LogTableColors`, `ControlColors`, `AnsiColors`).
- Do NOT hardcode `Color32::from_rgb(...)` inside `views/` or `components/` (exception: OS-native window buttons in `views/header/window_controls/`).

---

## 3. Adding Vector SVG Icons (`assets/icons/` + `crates/icons`)

1. Place the clean `.svg` asset at `assets/icons/<snake_name>.svg`.
2. Register the variant in `IconName`, `IconName::file_stem()`, `IconName::path()`, and `IconName::ALL` inside [crates/icons/src/lib.rs](../../../crates/icons/src/lib.rs).
3. Update `test_icon_count` in `crates/icons/src/lib.rs` (which asserts `IconName::ALL.len()`, existence of every SVG, and no dangling unmapped SVGs in `assets/icons/`).
4. Verify:
   ```bash
   ./.agents/scripts/harness.sh test-tier 0    # Tests uwu-icons --all-features
   ```

---

## 4. Adding Shortcuts (`crates/core_keymap`)

1. Add the variant to `define_actions! { ... }` (`canonical_name`, aliases) and `KeyAction::all()`, `category()`, `display_name()`, `description()` in [crates/core_keymap/src/action.rs](../../../crates/core_keymap/src/action.rs).
2. Register default keybindings in `KeymapManager::register_defaults()` ([crates/core_keymap/src/manager.rs](../../../crates/core_keymap/src/manager.rs)) with the appropriate `KeyContext` (`Global`, `Table`, `SearchInput`, `Autocomplete`, `Modal`, `RemoteServers`).
3. Map `KeyAction` $\rightarrow$ `AppAction` inside `crates/ui_gui/src/keymap/mod.rs` (`KeyActionExt`) and ensure `UwuGuiApp::resolve_active_key_context` (`app/mod.rs`) picks the right stack context.
4. Avoid binding conflicts by checking existing bindings via MCP `uwu_keymap_lookup` or `manager.rs`.

---

## 5. Headless GUI Testing (`crates/ui_gui/src/app/tests/`)

- Use `create_test_app()` in [test_helpers.rs](../../../crates/ui_gui/src/app/tests/test_helpers.rs) inside `#[tokio::test]` (no native OS window required).
- Verify all changes:
  ```bash
  ./.agents/scripts/harness.sh check-arch
  ./.agents/scripts/harness.sh test-tier 3    # core_workspace & core_keymap
  ./.agents/scripts/harness.sh test-tier 4    # uwu-ui-gui headless suite
  ```

---

## 6. Notifications, Toasts & Error Feedback

### Mandatory Decision Rule (Ask Developer First)
Before implementing any new notification, warning, or error message in `ui_gui`, **ask the developer** which display mode they want and recommend one based on context:
1. **Option A — Bottom-Right Toast** *(recommended for async, background, or global workspace notices unrelated to a specific modal/input)*
2. **Option B — Inline Error Banner** *(recommended for immediate failures tied to a specific action, form, picker, or modal, e.g. trying to enter a Dev Container without `.devcontainer` config)*

### Form A: Bottom-Right Stacked Toast
References: [collision_toast.rs](../../../crates/ui_gui/src/views/collision_toast.rs), [devcontainer.rs](../../../crates/ui_gui/src/views/header/devcontainer.rs), [toolbar.rs](../../../crates/ui_gui/src/views/header/toolbar.rs), [views/mod.rs](../../../crates/ui_gui/src/views/mod.rs).

- **Stacking Order (Bottom-Newest)**: Render at `Order::Foreground`, `Align2::RIGHT_BOTTOM`, `x = -16.0`, base `y = -16.0`. **The newest / highest-priority toast sits at the bottom (`-16.0`), pushing older visible toasts upward** (e.g. step `-68.0` / `-84.0` depending on toast height; track visible stack height or boolean visibility flags in `views/mod.rs` so toasts never overlap).
- **Frame & Tokens**:
  - `fill(theme.surfaces.mantle)`
  - `stroke(Stroke::new(1.0, accent_or_border))` (`theme.status.error`, `theme.status.warning`, or `theme.borders.border`)
  - `corner_radius(CornerRadius::same(8))`, `inner_margin(Margin::symmetric(12, 10))`
  - `Shadow { offset: [0, 6], blur: 20, spread: 0, color: Color32::from_black_alpha(55) }`
- **Content Layout**: `ui.horizontal`: `15x15` `IconName` (colored with matching status token) -> `ui.vertical` (`12.0` strong `theme.text.primary` title + `11.0` `theme.text.muted` detail) -> optional action `AppButton` + `IconButton::new(IconName::Close).size(22.0)`.

### Form B: Inline Context Error Banner
References: [devcontainer_picker.rs](../../../crates/ui_gui/src/views/remote_servers/devcontainer_picker.rs#L199-L223), [ssh_picker.rs](../../../crates/ui_gui/src/views/remote_servers/ssh_picker.rs#L84-L100).

- **Placement**: Render directly inside the active modal, picker, or section where the error occurred (not as a floating toast).
- **Frame & Tokens**:
  - `fill(theme.status.error.gamma_multiply(0.15))`
  - `stroke(Stroke::new(1.0, theme.status.error))`
  - `corner_radius(CornerRadius::same(6))`, `inner_margin(Margin::symmetric(10, 8))`
- **Content Layout**: `ui.horizontal`: `14x14` `IconName::AlertTriangle` (`theme.status.error`) + `4px` gap + wrapped `egui::Label` (`RichText::new(err_msg).size(11.5).color(theme.status.error)`).
