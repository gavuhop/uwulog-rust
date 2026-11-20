# Sổ tay Agent Harness, AST Token-Optimizer, MCP & Workflows (`AGENT_WORKFLOWS_AND_HARNESS.md`)

Tài liệu hướng dẫn vận hành trọn bộ hệ sinh thái AI Agent bên trong [`.agents/`](..) của `uwulog-rust`.

---

## 1. Cấu trúc Thư mục `.agents/`

```text
.agents/
├── docs/                                # Knowledge Base & Architecture Docs (INDEX.md, 5-Tier DAG, Flows, Arrow, GUI, Protocol, Harness)
├── hooks.json                           # Lifecycle hooks: SessionStart, PreToolUse
├── plugins/
│   └── uwu-dev-kit/
│       ├── plugin.json                  # Workspace plugin manifest
│       ├── mcp_config.json              # Tự động khởi chạy uwu-dev-mcp
│       └── uwu_mcp_server.py            # Local MCP Server (8 tools over stdio JSON-RPC 2.0)
├── rules/
│   └── GEMINI.md                        # Rulebook duy nhất tự nạp vào mọi prompt (không rải file ra root/crates)
├── scripts/
│   ├── ast_index.py                     # Bộ trích xuất Tree / AST Skeleton / Symbol / Hotspots / Fields / Keymap / Affected
│   ├── verify_arch_and_conv.py          # Kiểm chứng 5-Tier DAG, Expr/Icon parity, SSOT & Diff-Aware Conventions
│   ├── test_harness.py                  # Bộ unittest stdlib tự kiểm chứng hooks, AST, verify & MCP (< 1s)
│   ├── harness.sh                       # CLI Harness thống nhất (Linux/macOS/WSL/Git Bash)
│   └── hooks/
│       ├── session_start.py             # Bơm git branch & dirty count khi bắt đầu phiên (cross-platform)
│       ├── pre_command_guard.py         # Chặn lệnh shell phá hủy
│       ├── pre_write_guard.py           # Chặn ghi file plan.md vào git repo (bắt dùng Artifact Plan)
│       └── stop_arch_gate.py            # Tự chạy check-arch trước khi dừng nếu có file Rust/Cargo/SVG thay đổi
├── skills/                              # 7 Skills chuyên biệt (Progressive Disclosure)
│   ├── uwu-benchmark/
│   ├── uwu-dag-architecture/
│   ├── uwu-feature-discovery/           # Hỏi đáp làm rõ yêu cầu, Deep-Dive & chốt Artifact Plan trước khi code
│   ├── uwu-filter-arrow-dev/
│   ├── uwu-gui-component-dev/
│   ├── uwu-release-packaging/
│   └── uwu-remote-agent-debug/
└── workflows/                           # 2 kịch bản điều phối đa tác tử (`run_workflow`)
    ├── feature_delivery_pipeline.py
    └── bug_investigation_flow.py
```

---

## 2. Bộ Trích xuất Cây & AST Tối ưu Token ([ast_index.py](../scripts/ast_index.py))

Thay vì đọc toàn bộ các file `.rs` lớn (tốn hàng nghìn token), Agent thực hiện theo chuỗi:

1. **`tree`**: Bản đồ 14 crates theo 5 Tiers kèm số dòng LOC và tóm tắt modules/types:
   ```bash
   ./.agents/scripts/harness.sh tree [--tier 2] [-p uwu-ui-gui] [--json]
   ```
2. **`ast-skeleton`**: Giữ lại `struct`, `enum`, `trait`, `impl`, chữ ký `fn` và `[Lstart-Lend]`, cắt bỏ phần thân hàm (giảm 85–95% dung lượng văn bản):
   ```bash
   ./.agents/scripts/harness.sh ast-skeleton crates/core_engine/src/lib.rs [--tests] [--pub-only] [--json]
   ```
3. **`ast-symbol`**: Tìm định nghĩa chính xác và mọi nơi sử dụng trên toàn workspace kèm tên hàm bao quanh:
   ```bash
   ./.agents/scripts/harness.sh ast-symbol ArrowStorage [-p crate] [--fuzzy] [--defs-only] [--json]
   ```
4. **`ast-hotspots`**: Liệt kê file lớn, hàm dài nhất và các điểm `RwLock`/`Mutex`/`Atomic*`/`tokio::spawn`/`par_iter`/`unsafe`:
   ```bash
   ./.agents/scripts/harness.sh ast-hotspots [uwu-core-engine] [--top 15] [--json]
   ```

---

## 3. Bộ Kiểm chứng Kiến trúc & Conventions

### 3.1. Architecture & Convention Guard ([verify_arch_and_conv.py](../scripts/verify_arch_and_conv.py))
- `./.agents/scripts/harness.sh check-arch` (hỗ trợ `--dag-only`, `--conv-only`, `--strict`, `--json`):
  - **Hard Errors**: Vi phạm 5-Tier DAG từ `cargo metadata`, phụ thuộc vòng (DFS), sai tên package `uwu-<dir>`, lệch biến thể `Expr` giữa `core_filter` và `core_engine`, thiếu biến thể `StandardField` hoặc cột `__*` của Arrow.
  - **Advisory Warnings**: `.unwrap()`/`.expect()` runtime ngoài `#[cfg(test)]`, `Color32::from_rgb*` ngoài `theme/` & `window_controls/`, `std::thread::sleep` trong mã bất đồng bộ/UI.

---

## 4. MCP Server (`uwu-dev-mcp`) & Lifecycle Hooks

- **MCP Tools** ([uwu_mcp_server.py](../plugins/uwu-dev-kit/uwu_mcp_server.py)): `uwu_tree_map`, `uwu_ast_skeleton`, `uwu_find_symbol`, `uwu_ast_hotspots`, `uwu_inspect_dag_and_conv`, `uwu_standard_fields_info`, `uwu_keymap_lookup`, `uwu_run_harness`.
- **Hooks** ([hooks.json](../hooks.json)):
  - `uwu-session-context` (`SessionStart`, `enabled: true`): `session_start.py` (Python thuần cross-platform) báo nhánh Git, số file thay đổi và nhắc dùng Harness/MCP.
  - `uwu-command-guard` (`PreToolUse` on `run_command`, `enabled: true`): Chặn `git reset --hard`, `git clean -fd`, `git push --force`, `rm -rf` vào thư mục gốc/quan trọng, `cargo publish`.
  - `uwu-write-guard` (`PreToolUse` on `write_file`, `enabled: true`): Chặn ghi file plan (`plan.md`, `implementation_plan.md`, `*_plan.md`) vào trong git repo; đảm bảo plan chỉ lưu ở Artifact Plan.
  - `uwu-stop-arch-gate` (`Stop`, `enabled: true`): Khi có thay đổi ở `crates/**/*.rs`, `Cargo.toml`, hoặc `assets/icons/*.svg`, tự chạy `verify_arch_and_conv.py` trước khi dừng turn; nếu lỗi DAG/parity/convention thì yêu cầu agent tự sửa trước khi kết thúc.
- **Targeted Testing & Self-Test**:
  - `./.agents/scripts/harness.sh affected [--downstream] [--json]` và `test-changed [--downstream]`: Tự suy ra các crate bị chạm từ `git status` để chạy test mục tiêu.
  - `./.agents/scripts/harness.sh self-test`: Chạy bộ `unittest` (`test_harness.py`) kiểm chứng toàn bộ hooks, `ast_index.py`, `verify_arch_and_conv.py` và `uwu_mcp_server.py`.

---

## 5. Multi-Agent Workflows (`.agents/workflows/`)

Dùng với công cụ `run_workflow(ScriptPath=".agents/workflows/<script>.py", Args={...})`:
1. [feature_delivery_pipeline.py](../workflows/feature_delivery_pipeline.py): Pipeline 4 pha (`Architect & AST Blueprint` $\rightarrow$ `Core & Engine` $\rightarrow$ `Presentation` $\rightarrow$ `QA Gatekeeper`). **Yêu cầu tiên quyết**: hoàn tất pha hỏi đáp làm rõ theo [uwu-feature-discovery](../skills/uwu-feature-discovery/SKILL.md) và được developer duyệt **Artifact Plan** trước khi truyền `Args={"approved_spec": "..."}`.
2. [bug_investigation_flow.py](../workflows/bug_investigation_flow.py): Truy vết call-path bằng `ast-symbol`, tạo test tái hiện lỗi và sửa.
