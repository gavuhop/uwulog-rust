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
