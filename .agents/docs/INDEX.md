# Tài liệu Kỹ thuật `uwulog-rust` (`.agents/docs/INDEX.md`)

Chào mừng đến với trung tâm tài liệu kỹ thuật của **`uwulog-rust`** — trình xem và xử lý log đa nền tảng thời gian thực sử dụng lõi **Apache Arrow Columnar In-Memory**, bộ lọc vector hóa **Rayon**, giao thức remote streaming **`uwu-agent`**, và Desktop GUI **`egui`**.

---

## 1. Bản đồ Tài liệu

| Tài liệu | Đối tượng chính | Nội dung trọng tâm |
| :--- | :--- | :--- |
| [ARCHITECTURE_AND_CONVENTIONS.md](ARCHITECTURE_AND_CONVENTIONS.md) | Tech Lead, Contributor, AI Agent | Kiến trúc **5-Tier 1-Way DAG** trên 14 crates, ranh giới phụ thuộc, Taxonomy, SSOT `StandardField`, quy chuẩn Zero-Allocation / No-Panic / Lock Discipline, và Anti-Patterns. |
| [SYSTEM_FLOWS_AND_LIFECYCLES.md](SYSTEM_FLOWS_AND_LIFECYCLES.md) | Engine & GUI Developer, AI Agent | Giải phẫu 4 luồng sống còn: (A) Ingestion & Dual-Trigger Arrow Flush, (B) Vectorized Query & 4 Viewport Modes, (C) Unidirectional GUI Action-Reducer Flow, (D) Remote Agent `AGENT_READY_MARKER` + Framed RPC. |
| [ARROW_ENGINE_AND_QUERY_DSL.md](ARROW_ENGINE_AND_QUERY_DSL.md) | Core Engine / Filter Developer | Cấu trúc `ArrowStorage`, `ActiveRecordBatchBuilder`, `DynamicColumnBuilder` (`Int64 -> Float64 -> String`), `BatchColumns`, `QueryCompiler`, và đặc tả cú pháp Filter DSL (`Expr`). |
| [GUI_AND_KEYMAP_GUIDE.md](GUI_AND_KEYMAP_GUIDE.md) | Frontend / `egui` Developer | Kiến trúc `UwuGuiApp`, `WorkspaceManager`, `OverlayManager`, `GuiSession`, `theme::tokens`, Vector SVG Icons (`crates/icons`), Zed-style 4-layer Keymap (`crates/core_keymap`), và Headless GUI tests. |
| [REMOTE_AGENT_PROTOCOL.md](REMOTE_AGENT_PROTOCOL.md) | Transport / Daemon Developer | Đặc tả `core_protocol` (`AGENT_READY_MARKER`, 4-byte BE Framing, `ClientEnvelope`/`ServerEnvelope`), `uwu-agent`, `SshTransport` & `WslTransport`. |
| [AGENT_WORKFLOWS_AND_HARNESS.md](AGENT_WORKFLOWS_AND_HARNESS.md) | AI Agent & Maintainer | Sổ tay vận hành hệ sinh thái trong `.agents/`: Rules ([GEMINI.md](../rules/GEMINI.md)), AST Token-Optimizer ([ast_index.py](../scripts/ast_index.py)), Harness CLI, 6 Skills, 8 MCP Tools (`uwu-dev-mcp`), Hooks, và Multi-Agent `run_workflow`. |

---

## 2. Lệnh Nhanh Thường Dùng

```bash
# Khám phá codebase tiết kiệm 85-95% token
./.agents/scripts/harness.sh tree
./.agents/scripts/harness.sh ast-skeleton crates/core_engine/src/lib.rs
./.agents/scripts/harness.sh ast-symbol StandardField
./.agents/scripts/harness.sh ast-hotspots uwu-core-engine

# Kiểm chứng kiến trúc, format, clippy & unit tests
./.agents/scripts/harness.sh check-arch
./.agents/scripts/harness.sh test-tier <0|1|2|3|4|all>
./.agents/scripts/harness.sh verify

# Build tối ưu cho phát triển local (< 1s incremental)
cargo build --profile release-dev
```
