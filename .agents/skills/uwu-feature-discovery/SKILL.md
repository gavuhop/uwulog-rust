---
name: uwu-feature-discovery
description: >-
  Run structured interactive requirement discovery, hybrid Q&A (batch options + single-topic
  deep-dive), and Artifact Plan approval before implementing any feature, logic, UI/UX, or
  architecture change in uwulog-rust. Activate before editing code whenever requirements,
  purpose, behavior, UX, or technical approach need clarification or alignment.
---

# 🧭 `uwu-feature-discovery` — Interactive Discovery, Alignment & Artifact Plan Runbook

> **Core Principle**: Build the *right* thing before building it fast. Never guess intent, silently invent behavior, or write code before the developer and agent share a clear mental model of **Why**, **What**, and **How**.

---

## 1. Scope & Exemptions (Strict Mode)

- **Mandatory** for every new feature, behavioral/logic change, UI/UX update, schema/protocol change, or non-trivial refactor.
- **Exempt ONLY** for 100%-unambiguous, mechanical tasks:
  - Fixing typos, comments, or formatting (`cargo fmt`).
  - Running commands/tests/benchmarks or read-only exploration/Q&A.
  - Strictly literal single-line edits where the developer already specified exact file, behavior, and values with zero ambiguity.

---

## 2. Phase 0: Codebase-First Discovery (Read-Only)

**Never ask the developer questions that can be answered by inspecting the codebase.**

Before asking your first question:
1. Run `./.agents/scripts/harness.sh tree` (or MCP `uwu_tree_map`) to locate affected crates and verify 5-Tier DAG boundaries.
2. Run `ast-skeleton` / `ast-symbol` / `fields` / `keymap` on relevant modules and read targeted line ranges.
3. Map out what already exists (reusable components, `AppAction`/`AppEvent`, `StandardField`, `Expr` variants, theme tokens, existing patterns) so every question is grounded in real code.

---

## 3. Phase 1: Hybrid Adaptive Clarification Dialogue

### Mode A — Batch Alignment (Default)
Use when clarifying a new request or multiple independent decisions:
- Ask **3–5 focused, high-leverage questions per turn** (prefer `ask_question` when options are clear, or a concise numbered list).
- Every question MUST:
  1. Present **concrete options** instead of open-ended blanks whenever possible.
  2. Mark a **`(Recommended)`** option first with a **1-sentence rationale** grounded in `uwulog-rust`'s architecture/UX.
  3. Include a **quick-proceed path** (e.g., *"Chốt theo đề xuất & làm ngay"* / *"Proceed with recommendation"*) when the decision is low-risk or nearly clear, so the developer isn't slowed down unnecessarily.
  4. Include an explicit **Deep-Dive trigger option** (e.g., *"Bàn thêm — phân tích sâu hơn về ý này"* / *"Discuss deeper — unpack options & trade-offs"*) so the developer can invoke Deep-Dive Mode with a single click without having to type a custom reply.

### Mode B — Single-Topic Deep-Dive (Auto-Triggered)
**Immediately switch out of batch questioning** when ANY of the following occurs:
- The developer selects the **"Bàn thêm / Discuss deeper"** option on any question.
- The developer types a custom/open response signaling they want to discuss an idea further.
- The developer seems unsure, says they don't know yet, or hasn't formed a clear answer.
- The developer pushes back or appears unconvinced by your recommendation.

**How to run Deep-Dive Mode:**
1. **Pause all other open questions** — focus on **one unresolved topic at a time** (if the developer marked multiple batch questions as *"Bàn thêm"*, queue them and resolve them **sequentially, one by one**).
2. **Clarify the problem space first**: briefly explain how the current codebase behaves (with file links) and *why* this decision matters (edge cases, performance, UX impact, or DAG constraints).
3. **Propose 2–3 concrete solutions** with crisp **pros, cons, and your recommendation + rationale**.
4. **Iterate** on that point until the developer feels confident and aligned, then move to the next queued deep-dive topic or remaining batch questions (if any).

---

## 4. Layer-Specific Discovery Checklists

Pick only the relevant items based on the tiers touched (do not ask irrelevant questions):

### A. Product, Purpose & Boundaries (Always Check)
- **Why**: What user problem or workflow friction does this solve?
- **What (Happy Path & Edge Cases)**: Expected inputs/outputs, empty/loading/error states, cancellation/interrupt behavior.
- **Scope**: What is explicitly **in-scope** vs **out-of-scope** for this iteration?

### B. Tier 0–2: Schema, Filter DSL & Arrow Engine (`core_schema`, `core_filter`, `core_engine`)
- **SSOT Schema**: Does this touch canonical fields (`timestamp`, `level`, `message`) or `StandardField` aliases? Remember user `"id"` is never aliased to `__id`.
- **Arrow Columns & Memory**: System (`__`), user (`f:`), or null-marker (`n:`) columns? Impact on `ActiveRecordBatchBuilder`, `BatchColumns`, or zero-alloc hot paths?
- **Filter DSL & Expr Parity**: Exact syntax/precedence/case-sensitivity/null semantics? Must behave identically in `core_filter/src/evaluator.rs` AND `core_engine/src/arrow_storage/compiler.rs`.
- **Concurrency & Locks**: Any lock/channel/Rayon scan or flush behavior implications?

### C. Tier 1 & Tier 0 Protocol: Remote Agent & Sources (`core_protocol`, `driver_transport`, `driver_sources`, `cli_agent`)
- **Wire Protocol**: Changes to `ClientEnvelope` / `ServerEnvelope` or handshake? (Pre-release: no speculative backward-compat shims unless requested.)
- **Failure & Lifecycle**: Disconnect/reconnect, file rotation/truncation, EOF, child process cleanup, cross-platform differences (Linux/macOS/Windows/WSL/SSH).

### D. Tier 3–4: Workspace, Keymap, GUI & CLI (`core_workspace`, `core_keymap`, `ui_gui`, `cli`)
- **User Flow & State**: Where in the UI does this live? Should state persist across sessions/workspaces (`core_workspace`) or remain transient (`GuiSession`)?
- **Shortcuts & KeyContext**: Need a new `KeyAction`? Which `KeyContext` (`Global`, `Table`, `SearchInput`, `Modal`, etc.)? Check conflicts via `keymap`.
- **Notifications & Errors (Mandatory Rule 4.10)**: Never guess — explicitly ask & recommend between **Bottom-Right Toast** vs **Inline Error Banner**.
- **60 FPS & Frozen View**: Confirm no blocking I/O on the UI thread and whether frozen view (`is_auto_scroll == false`) stays unaffected.

---

## 5. Phase 2: Artifact Plan Gate (No Repo Plan Files)

Once all questions are aligned, **before modifying any project code**:
1. Create (or update) an **Artifact Plan** at `<appDataDir>/brain/<conversation-id>/plan.md` using `write_file` with:
   - `ArtifactMetadata: { UserFacing: true, RequestFeedback: true, Summary: "..." }`
   - **Never** write plan/spec markdown files into the git repository.
2. Keep the Artifact Plan concise and actionable using this template:

```markdown
# Plan: <Feature / Change Title>

## 1. Goal & Motivation
- **Problem**: ...
- **Agreed Behavior**: ...
- **Non-Goals (Out of Scope)**: ...

## 2. Key Decisions Confirmed
- Decision 1: <choice> — <brief reason>
- Decision 2: <choice> — <brief reason>

## 3. Architecture & Change Map (5-Tier DAG Order)
- **Tier 0** (`crates/...`): ...
- **Tier 1** (`crates/...`): ...
- **Tier 2** (`crates/...`): ...
- **Tier 3** (`crates/...`): ...
- **Tier 4** (`crates/...`): ...

## 4. Edge Cases & Error Handling
- ...

## 5. Verification Plan
- Unit / integration / headless GUI tests to add or update
- Harness checks: `./.agents/scripts/harness.sh check-arch` + `test-tier <N>` / `verify`
```

3. Point the developer to the Artifact Plan without re-summarizing the entire file in chat, and **wait for approval** before editing source files.

---

## 6. Phase 3: Hand-off to Implementation

After the developer approves the Artifact Plan:
- **Standard changes**: Implement directly following the relevant domain skills (`uwu-dag-architecture`, `uwu-filter-arrow-dev`, `uwu-gui-component-dev`, `uwu-remote-agent-debug`) and verify with `harness.sh`.
- **Large cross-tier features**: Optionally orchestrate via:
  ```text
  run_workflow(
    ScriptPath=".agents/workflows/feature_delivery_pipeline.py",
    Args={"approved_spec": "<approved plan summary & decisions>"}
  )
  ```
- If an unexpected design ambiguity surfaces mid-implementation, **pause and check back** rather than silently guessing.
