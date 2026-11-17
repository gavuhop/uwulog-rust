---
name: uwu-filter-arrow-dev
description: >-
  Develop, extend, or optimize the Filter DSL parser, scalar evaluator, and Apache Arrow
  vectorized QueryCompiler / ArrowStorage engine. Activate when modifying query syntax in
  core_filter or working on ArrowStorage, ActiveRecordBatchBuilder, BatchColumns, or
  QueryCompiler in core_engine.
---

# ⚡ `uwu-filter-arrow-dev` — Filter DSL & Apache Arrow Kernel Runbook

Detailed AST-to-Arrow mapping and Filter DSL grammar are documented in [.agents/docs/ARROW_ENGINE_AND_QUERY_DSL.md](../../docs/ARROW_ENGINE_AND_QUERY_DSL.md).

---

## 1. 5-Step Workflow for Filter DSL & Arrow Changes

1. **Inspect Skeletons First**:
   ```bash
   ./.agents/scripts/harness.sh ast-skeleton crates/core_filter/src/parser.rs
   ./.agents/scripts/harness.sh ast-skeleton crates/core_engine/src/arrow_storage/compiler.rs
   ```
2. **Update Tokenizer & Recursive-Descent Parser (`crates/core_filter/src/parser.rs`)**:
   - Keep `tokenize()` and `Parser::parse()` panic-free on malformed user input (unclosed quotes, empty parens, invalid regex, incomplete `..` ranges).
   - Resolve relative timestamps (`now`, `now-10m`, `1h..30m`) using `now: f64` (`current_data_now()`).
3. **Update Scalar Evaluator (`crates/core_filter/src/evaluator.rs`)**:
   - Implement every `Expr` variant in `eval_event(expr: &Expr, log: &LogEvent, now: f64) -> bool`.
   - Use `log.get_field_cow(field)` and `uwu_core_util::contains_ignore_case` without extra heap allocations.
4. **Mirror Identically in Arrow `QueryCompiler` (`crates/core_engine/src/arrow_storage/compiler.rs`)**:
   - Update `QueryCompiler::eval_batch_with_mask` and its helper kernels (`ColumnView`, `eval_masked`, `resolve_column`, `resolve_numeric_column`).
   - Propagate `mask: Option<&BooleanBuffer>` for short-circuiting (`And`, `Or`, `Not`) so skipped rows perform zero work.
   - Never call `batch.column_by_name()` inside per-row loops; resolve `ColumnView` or `BatchColumns` once per `RecordBatch`.
5. **Verify Parity, Concurrency & Performance**:
   - Add symmetric tests in `crates/core_filter/src/tests.rs` AND `crates/core_engine/src/lib.rs` / `tests/pipeline_integration.rs`.
   - Run:
     ```bash
     ./.agents/scripts/harness.sh check-arch     # Enforces Expr parity across crates
     ./.agents/scripts/harness.sh test-tier 0
     ./.agents/scripts/harness.sh test-tier 2    # Includes loom_concurrency & pipeline_integration
     ```

---

## 2. `ArrowStorage` & `SystemEngine` Engine Rules

- **Lock Discipline**: Keep `LogNormalizer::normalize`, `detect_timestamp_format`/`parse_with_format`, and schema diffing **outside** `ArrowStorage` write locks (`parking_lot::{Mutex, RwLock}`). Never hold locks across `.await`.
- **Dual-Trigger Flush**: Preserve `if raw_rx.is_empty() { storage_clone.flush(); }` in `SystemEngine::new` (`crates/core_engine/src/lib.rs`) alongside `TARGET_BATCH_SIZE = 4096` sealing (`crates/core_engine/src/arrow_storage/storage.rs`).
- **Dynamic Type Promotion**: `DynamicColumnBuilder` (`builder.rs`) promotes `Int64 -> Float64 -> String` cleanly without dropping prior values (`unify_and_concat` reconciles schemas across chunks).
- **Performance Regression Guard**: For hot-loop edits in `compiler.rs` or `storage.rs`, run `.agents/skills/uwu-benchmark/scripts/run_bench.sh -m compare`.
