# `uwulog-rust` Agent Rules (`.agents/rules/GEMINI.md`)

> Real-time cross-platform log viewer powered by an in-memory Apache Arrow engine (`uwu-core-engine`), vectorized Rayon query compiler, `egui` GUI (`uwu-ui-gui`), and framed remote agent (`uwu-agent`).

## 1. Token-First Exploration Protocol (Mandatory)

Never dump large `.rs` files (> 150 lines) into context just to locate symbols:
1. `./.agents/scripts/harness.sh tree` *(or MCP `uwu_tree_map`)* — map 14 crates, Tiers, LOC, and top-level types.
2. `./.agents/scripts/harness.sh ast-skeleton <file_or_crate>` *(or MCP `uwu_ast_skeleton`)* — extract `struct`/`enum`/`trait`/`impl`/`fn` signatures + `[Lstart-Lend]` without bodies (85–95% token savings).
3. `./.agents/scripts/harness.sh ast-symbol <Name> | ast-hotspots [crate] | fields | keymap` *(or MCP `uwu_find_symbol`, `uwu_ast_hotspots`, `uwu_standard_fields_info`, `uwu_keymap_lookup`)* — locate definitions, call sites, locks, `par_iter`, `unsafe`, SSOT schema, and shortcuts.
4. `read_file` with targeted `StartLine` and `EndLine` covering only the needed block.
5. Verify before finishing: `./.agents/scripts/harness.sh check-arch` + `test-tier <N>` (or `verify` for cross-tier edits; MCP `uwu_inspect_dag_and_conv` / `uwu_run_harness`).

## 2. 5-Tier 1-Way DAG Architecture (Strict Law)

Dependencies flow strictly top-down (`Tier 4 -> Tier 3 -> Tier 2 -> Tier 1 -> Tier 0`). Upward, cyclic, or forbidden intra-tier dependencies fail `./.agents/scripts/harness.sh check-arch`:
- **Tier 4** *(Presentation & Binaries)*: `uwu-cli`, `uwu-ui-gui`, `uwu-cli-agent`, `uwu-benchmarks` $\rightarrow$ Tiers `0..3` (no sibling Tier 4 deps).
- **Tier 3** *(Workspace & Keymap)*: `uwu-core-workspace` $\rightarrow$ Tiers `0..2`; `uwu-core-keymap` $\rightarrow$ standalone (optional `egui`). Never depend on `uwu-ui-gui` or Tier 4.
- **Tier 2** *(Columnar Engine)*: `uwu-core-engine` $\rightarrow$ Tier 1 (`uwu-driver-sources`) + Tier 0 (`uwu-core-filter`, `uwu-core-schema`, `uwu-core-util`). Zero GUI/workspace awareness.
- **Tier 1** *(Drivers & Transport)*: `uwu-driver-sources` $\rightarrow$ `uwu-driver-transport` + Tier 0; `uwu-driver-transport` $\rightarrow$ `uwu-core-util`. No `core_engine` (Arrow) or `core_filter`.
- **Tier 0** *(Foundations)*: `uwu-core-util` (`[]`), `uwu-core-schema` (`-> core_util`), `uwu-core-protocol` (`-> core_schema`), `uwu-core-filter` (`-> core_schema + core_util`), `uwu-icons` (`[]`).

## 3. Core Conventions

1. **Taxonomy**: `core_*` (domain/logic), `driver_*` (OS I/O/transport), `ui_*` (presentation), `cli_*` (binaries). Package name = `uwu-<dir_name>` (`_` $\rightarrow$ `-`).
2. **SSOT Schema & Arrow Columns**: Resolve canonical semantic fields (`timestamp`, `level`, `message`) and aliases exclusively via `StandardField::from_alias` / `canonical_name()` (`crates/core_schema/src/lib.rs`); user `"id"` fields are never aliased to internal row IDs. Arrow system columns use `__` prefix (`__id`, `__color`, `__timestamp`, `__timestamp_secs`, `__message`), user payload columns use `f:` prefix (`f:<key>`, optional `uwu.json="1"` metadata for structured JSON arrays/objects), and explicit null markers use `n:` prefix (`n:<key>`).
3. **Zero-Allocation Hot Path**: Prefer `Cow<'_, str>` (`value_to_cow`, `get_field_cow`, `strip_ansi`), flat `LogFields(Vec<(String, Value)>)` over per-event `HashMap`s, `memchr`, and `BatchColumns` zero-lookup caches per `RecordBatch`.
4. **No-Panic Runtime**: Never `.unwrap()` / `.expect()` on untrusted payloads, JSON, UTF-8, user queries/regex, channels, or locks outside `#[cfg(test)]`.
5. **Lock & 60 FPS Discipline**: Never hold `RwLock`/`Mutex` across `.await` or inside `LogNormalizer::normalize`. Maintain `< 16.6ms`/frame in `ui_gui` (no blocking I/O on UI thread; offload via `tokio::runtime::Handle` + `event_tx` -> `AppEvent` -> `reducer::reduce`).
6. **UI Tokens & Keymap**: UI events emit `AppAction` (`app/mod.rs`); colors/spacing come from `theme::tokens` (not raw `Color32::from_rgb` in `views/`/`components/`). Icons are SVGs in `assets/icons/` (`crates/icons`); shortcuts use `KeyAction` + `KeyContext` (`crates/core_keymap`).
7. **Expr Parity**: Any `Expr` variant in `core_filter/src/parser.rs` MUST be implemented identically in both `core_filter/src/evaluator.rs` and `core_engine/src/arrow_storage/compiler.rs`.
8. **Git**: Conventional Commits (`<type>(<scope>): <subject>`).
9. **Pre-Release Clean Code (No Unnecessary Fallbacks)**: The project is pre-release. Never add speculative backward-compatibility shims, legacy data migrations, or silent fallbacks for missing/old metadata unless explicitly requested.
10. **Notifications & Error Placement (Ask First)**: Never guess where to render a new notification or error in `ui_gui`. Before implementing, ask the developer and recommend between: (a) **Bottom-Right Toast** (async/global/background notices, anchored `Align2::RIGHT_BOTTOM` at `(-16.0, -16.0)` with newest toast at the bottom pushing older visible toasts upward), or (b) **Inline Error Banner** (action/form-scoped errors directly inside the failing view/modal, like `devcontainer_picker.rs`). Follow exact styling & stacking templates in `.agents/skills/uwu-gui-component-dev/SKILL.md`.

## 4. Core System Flows

- **Flow A (Ingestion & Dual-Trigger Flush)**: `LogSource` $\rightarrow$ `mpsc(10_000)` $\rightarrow$ drain `<= 512` $\rightarrow$ `LogNormalizer::normalize` + adaptive timestamp + schema discovery **outside storage lock** $\rightarrow$ `ArrowStorage::push_events` $\rightarrow$ immediate `flush()` when `raw_rx.is_empty()`.
- **Flow B (Vectorized Query & Viewport)**: Full Rayon scan (`search_with_count`), live delta (`filter_incremental`), reverse pagination (`search_before`), unfiltered window (`get_unfiltered_events`).
- **Flow C (GUI Unidirectional Loop)**: `KeymapManager`/Widgets $\rightarrow$ `AppAction` $\rightarrow$ `UwuGuiApp::dispatch_action` $\rightarrow$ Tokio/Rayon task $\rightarrow$ `AppEvent` via `event_tx` $\rightarrow$ `reducer::reduce` $\rightarrow$ next frame.
- **Flow D (Remote Framed RPC)**: `SshTransport`/`WslTransport` spawns `uwu-agent proxy` $\rightarrow$ `wait_for_ready_marker` skips MOTD/`.bashrc` until `AGENT_READY_MARKER` (`__UWU_AGENT_READY__\n`) $\rightarrow$ 4-byte BE `FramedWriter`/`FramedReader` (`ClientEnvelope`/`ServerEnvelope`).

## 5. Harness Commands

```bash
./.agents/scripts/harness.sh tree | ast-skeleton <path> | ast-symbol <Name> | ast-hotspots [crate] | fields | keymap
./.agents/scripts/harness.sh check-arch            # 5-Tier DAG + Expr/Icon parity + Diff-aware conventions
./.agents/scripts/harness.sh test-tier <0|1|2|3|4|all>
./.agents/scripts/harness.sh verify                # check-arch + fmt + clippy + tests
cargo build --profile release-dev                  # Fast optimized local build
```
