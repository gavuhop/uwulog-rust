---
name: uwu-dag-architecture
description: >-
  Enforce the 5-Tier 1-Way DAG architecture, crate taxonomy, StandardField SSOT schema,
  zero-allocation conventions, and lock discipline across uwulog-rust. Activate when adding a
  new crate, modifying Cargo.toml dependencies, updating StandardField/LogEvent in core_schema,
  or auditing cross-crate architecture.
---

# 🏗️ `uwu-dag-architecture` — 5-Tier DAG & Conventions Runbook

Full crate-by-crate dependency matrix and convention rules are documented in [.agents/docs/ARCHITECTURE_AND_CONVENTIONS.md](../../docs/ARCHITECTURE_AND_CONVENTIONS.md).

---

## 1. Adding or Modifying Crate Dependencies

1. Check allowed dependency direction (`Tier 4 -> Tier 3 -> Tier 2 -> Tier 1 -> Tier 0`) in [.agents/docs/ARCHITECTURE_AND_CONVENTIONS.md](../../docs/ARCHITECTURE_AND_CONVENTIONS.md).
2. Add internal dependencies via `[workspace.dependencies]` in root `Cargo.toml` and reference with `.workspace = true` in `crates/<crate>/Cargo.toml`.
3. Never introduce:
   - Upward edges (e.g., `core_engine` importing `core_workspace`, or `driver_sources` importing `core_engine`).
   - Forbidden intra-tier edges (e.g., `driver_transport -> driver_sources`, `core_util -> core_schema`, or `ui_gui -> cli`).
4. Verify immediately:
   ```bash
   ./.agents/scripts/harness.sh check-arch
   ```

---

## 2. Adding a New Workspace Crate

1. **Taxonomy & Naming**: Folder `crates/<group>_<name>` MUST use package name `uwu-<group>-<name>` (`_` $\rightarrow$ `-`), where `<group>` is `core`, `driver`, `ui`, or `cli`.
2. **Register in 3 places**:
   - Add member & `[workspace.dependencies]` entry in root `Cargo.toml`.
   - Register `(tier, "Tier N: ...")` in `CRATE_TIERS` inside [ast_index.py](../../scripts/ast_index.py).
   - If intra-tier edges are required, update `ALLOWED_INTRA_TIER_EDGES` in [verify_arch_and_conv.py](../../scripts/verify_arch_and_conv.py) only when strictly acyclic.
3. Validate with `./.agents/scripts/harness.sh check-arch`.

---

## 3. Extending SSOT Schema (`core_schema`)

1. Canonical semantic fields (`timestamp`, `level`, `message`) and their aliases (`ts`, `lvl`, `severity`, `msg`, etc.) are owned exclusively by `StandardField` in [crates/core_schema/src/lib.rs](../../../crates/core_schema/src/lib.rs) (user `"id"` keys are never aliased to internal row IDs).
2. When adding an alias or canonical field:
   - Update `StandardField::from_alias`, `canonical_name()`, and `field_type()`.
   - Keep Arrow's `__`-prefixed system columns (`__id`, `__color`, `__timestamp`, `__timestamp_secs`, `__message`), `f:`-prefixed user columns, and `n:`-prefixed null-marker columns aligned in `ActiveRecordBatchBuilder::seal` (`builder.rs`), `BatchColumns::extract` (`storage.rs`), and `QueryCompiler::resolve_column` (`compiler.rs`).
   - Keep `LogFields` flat-vector representation ([fields.rs](../../../crates/core_schema/src/fields.rs)) and `value_to_cow` zero-alloc paths intact.

---

## 4. Verification Checklist

```bash
./.agents/scripts/harness.sh check-arch            # DAG + taxonomy + Expr parity + conventions
./.agents/scripts/harness.sh check-arch --strict   # Also fail on advisory convention warnings
./.agents/scripts/harness.sh test-tier 0           # Or test-tier all for cross-tier changes
```
