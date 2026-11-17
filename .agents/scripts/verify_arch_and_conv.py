#!/usr/bin/env python3
"""
verify_arch_and_conv.py — 5-Tier DAG Architecture & Code Convention Guard for uwulog-rust.

Checks:
  1. Architecture & Invariants (Hard Errors):
     - Live `cargo metadata` 5-Tier 1-Way DAG validation (no upward, cyclic, or forbidden intra-tier deps)
     - Crate package naming taxonomy (`crates/<dir>` <-> `uwu-<dir>`)
     - Cross-crate `Expr` AST parity (`core_filter/parser.rs` <-> `evaluator.rs` <-> `core_engine/compiler.rs`)
     - SSOT `StandardField` & Arrow internal `__` columns consistency
     - Vector SVG Icon parity (`assets/icons/*.svg` <-> `crates/icons/src/lib.rs`)
     - Zero-lookup `BatchColumns` discipline (`.column_by_name(` forbidden outside single-lookup resolvers)
  2. Conventions (Diff-Aware Ratchet + Advisory Warnings, or all Errors with `--strict`):
     - Runtime `.unwrap()` / `.expect(` outside `#[cfg(test)]`
     - Hardcoded `Color32::from_rgb*` in `ui_gui/src/views` & `components` outside `theme/` & `window_controls/`
     - Blocking `std::thread::sleep` in async/UI paths outside tests
     - `std::sync::{Mutex, RwLock}` in `core_engine` instead of `parking_lot`
     - Any new violation on git-modified lines or exceeding `BASELINE_WARNING_BUDGET` fails immediately.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Set, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from ast_index import (
    CRATE_TIERS,
    IGNORED_DIRS,
    _enclosing_fn,
    _extract_fn_source,
    find_repo_root,
    get_test_line_ranges,
    parse_rust_skeleton,
    tokenize_rust,
)

BASELINE_WARNING_BUDGET = 27

ALLOWED_INTRA_TIER_EDGES: Dict[str, Set[str]] = {
    "uwu-core-schema": {"uwu-core-util"},
    "uwu-core-protocol": {"uwu-core-schema"},
    "uwu-core-filter": {"uwu-core-schema", "uwu-core-util"},
    "uwu-driver-sources": {"uwu-driver-transport"},
}

ALLOWED_COLUMN_BY_NAME_FNS = {
    "resolve_column",
    "resolve_numeric_column",
    "extract",
    "eval_text",
    "unify_and_concat",
    "find_log_index_by_id",
    "find_log_index_or_lower_bound",
}


def run_cargo_metadata(repo_root: Path) -> dict:
    res = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if res.returncode != 0:
        raise RuntimeError(f"cargo metadata failed: {res.stderr.strip()}")
    return json.loads(res.stdout)


def get_modified_rust_lines(repo_root: Path) -> Tuple[Set[str], Dict[str, Set[int]]]:
    """Return (untracked_rs_files, {rel_file: set_of_added_or_modified_line_numbers}) under crates/."""
    untracked: Set[str] = set()
    modified_lines: Dict[str, Set[int]] = {}

    try:
        u_res = subprocess.run(
            ["git", "ls-files", "--others", "--exclude-standard", "--", "crates/"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=False,
        )
        if u_res.returncode == 0:
            for line in u_res.stdout.splitlines():
                p = line.strip().replace("\\", "/")
                if p.endswith(".rs"):
                    untracked.add(p)

        d_res = subprocess.run(
            ["git", "diff", "-U0", "HEAD", "--", "crates/"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=False,
        )
        if d_res.returncode == 0:
            cur_file: Optional[str] = None
            for raw in d_res.stdout.splitlines():
                if raw.startswith("+++ b/"):
                    cur_file = raw[6:].strip().replace("\\", "/")
                elif raw.startswith("@@ ") and cur_file:
                    m = re.search(r"\+(\d+)(?:,(\d+))?", raw)
                    if m:
                        start = int(m.group(1))
                        count = int(m.group(2)) if m.group(2) is not None else 1
                        if count > 0:
                            modified_lines.setdefault(cur_file, set()).update(range(start, start + count))
    except Exception:
        pass

    return untracked, modified_lines


def check_dag(repo_root: Path) -> Tuple[List[str], List[dict]]:
    errors: List[str] = []
    edges: List[dict] = []

    meta = run_cargo_metadata(repo_root)
    ws_ids = set(meta.get("workspace_members", []))
    packages = [p for p in meta.get("packages", []) if p.get("id") in ws_ids]
    ws_names = {p["name"] for p in packages}

    adj: Dict[str, List[str]] = {}

    for pkg in packages:
        name = pkg["name"]
        manifest_path = Path(pkg["manifest_path"]).resolve()
        try:
            rel_dir = str(manifest_path.parent.relative_to(repo_root)).replace("\\", "/")
        except ValueError:
            rel_dir = str(manifest_path.parent)

        dir_leaf = Path(rel_dir).name
        expected_name = f"uwu-{dir_leaf.replace('_', '-')}"
        if name != expected_name:
            errors.append(
                f"Taxonomy mismatch at {rel_dir}: package name '{name}' != expected '{expected_name}'"
            )

        if name not in CRATE_TIERS:
            errors.append(f"Unmapped workspace crate '{name}' ({rel_dir}) not registered in 5-Tier CRATE_TIERS")
            continue

        src_tier, _ = CRATE_TIERS[name]
        internal_deps: List[str] = []

        for dep in pkg.get("dependencies", []):
            dep_name = dep.get("name", "")
            dep_kind = dep.get("kind")
            if dep_name not in ws_names or dep_kind == "dev":
                continue
            internal_deps.append(dep_name)

            if dep_name not in CRATE_TIERS:
                errors.append(f"Unknown internal dependency '{name}' -> '{dep_name}'")
                continue

            dst_tier, _ = CRATE_TIERS[dep_name]
            status = "OK"
            if src_tier < dst_tier:
                status = "UPWARD_VIOLATION"
                errors.append(
                    f"5-Tier DAG violation: '{name}' (Tier {src_tier}) depends upward on '{dep_name}' (Tier {dst_tier})"
                )
            elif src_tier == dst_tier:
                allowed = ALLOWED_INTRA_TIER_EDGES.get(name, set())
                if dep_name not in allowed:
                    status = "INTRA_TIER_VIOLATION"
                    errors.append(
                        f"Forbidden intra-tier dependency: '{name}' (Tier {src_tier}) -> '{dep_name}' (Tier {dst_tier})"
                    )

            edges.append({
                "from": name,
                "from_tier": src_tier,
                "to": dep_name,
                "to_tier": dst_tier,
                "status": status,
            })

        adj[name] = internal_deps

    visiting: Set[str] = set()
    visited: Set[str] = set()

    def dfs(node: str, stack: List[str]) -> None:
        if node in visited:
            return
        if node in visiting:
            cycle_start = stack.index(node)
            cycle = " -> ".join(stack[cycle_start:] + [node])
            errors.append(f"Dependency cycle detected: {cycle}")
            return
        visiting.add(node)
        stack.append(node)
        for nxt in adj.get(node, []):
            dfs(nxt, stack)
        stack.pop()
        visiting.remove(node)
        visited.add(node)

    for pkg_name in sorted(adj.keys()):
        dfs(pkg_name, [])

    return errors, edges


def check_invariants_and_conventions(repo_root: Path) -> Tuple[List[str], List[dict]]:
    errors: List[str] = []
    warnings: List[dict] = []

    # 1. Cross-Crate Expr Variant Parity (core_filter <-> core_engine)
    parser_path = repo_root / "crates/core_filter/src/parser.rs"
    eval_path = repo_root / "crates/core_filter/src/evaluator.rs"
    compiler_path = repo_root / "crates/core_engine/src/arrow_storage/compiler.rs"

    if parser_path.is_file() and eval_path.is_file() and compiler_path.is_file():
        parser_src = parser_path.read_text(encoding="utf-8", errors="replace")
        eval_src = eval_path.read_text(encoding="utf-8", errors="replace")
        compiler_src = compiler_path.read_text(encoding="utf-8", errors="replace")

        items, _ = parse_rust_skeleton(parser_src)
        expr_item = next((it for it in items if it.kind == "enum" and it.name == "Expr"), None)
        if expr_item and expr_item.body_text:
            variants = re.findall(r"(?m)^\s*([A-Z][A-Za-z0-9_]*)\s*(?:\(|\{|,)", expr_item.body_text)
            for v in variants:
                pat = f"Expr::{v}"
                if pat not in eval_src:
                    errors.append(f"Expr parity violation: '{pat}' missing in crates/core_filter/src/evaluator.rs")
                if pat not in compiler_src:
                    errors.append(
                        f"Expr parity violation: '{pat}' missing in crates/core_engine/src/arrow_storage/compiler.rs"
                    )
        else:
            errors.append("Could not locate `enum Expr` in crates/core_filter/src/parser.rs")

    # 2. SSOT StandardField & Arrow internal columns
    schema_path = repo_root / "crates/core_schema/src/lib.rs"
    builder_path = repo_root / "crates/core_engine/src/arrow_storage/builder.rs"
    storage_path = repo_root / "crates/core_engine/src/arrow_storage/storage.rs"

    if schema_path.is_file():
        schema_src = schema_path.read_text(encoding="utf-8", errors="replace")
        for expected_var in ("Timestamp", "Level", "Message", "Id"):
            if expected_var not in schema_src:
                errors.append(f"SSOT StandardField missing variant: {expected_var}")

    core_arrow_cols = ("__id", "__color", "__timestamp", "__timestamp_secs", "__message")
    if builder_path.is_file() and storage_path.is_file():
        b_src = builder_path.read_text(encoding="utf-8", errors="replace")
        s_src = storage_path.read_text(encoding="utf-8", errors="replace")
        for col in core_arrow_cols:
            if f'"{col}"' not in b_src:
                errors.append(f"Arrow builder missing core column '{col}' in builder.rs")
            if f'"{col}"' not in s_src:
                errors.append(f"Arrow storage missing core column '{col}' in storage.rs")

    # 3. Vector SVG Icon Parity (assets/icons/*.svg <-> crates/icons/src/lib.rs)
    icons_dir = repo_root / "assets/icons"
    icons_lib = repo_root / "crates/icons/src/lib.rs"
    if icons_dir.is_dir() and icons_lib.is_file():
        svg_stems = {p.stem for p in icons_dir.glob("*.svg")}
        icons_src = icons_lib.read_text(encoding="utf-8", errors="replace")
        file_stem_fn = _extract_fn_source(icons_src, "file_stem")
        registered_stems = set(re.findall(r'Self::\w+\s*=>\s*"([^"]+)"', file_stem_fn))
        if registered_stems:
            missing_svgs = sorted(registered_stems - svg_stems)
            unregistered_svgs = sorted(svg_stems - registered_stems)
            if missing_svgs:
                errors.append(f"Icon parity violation: missing SVG files in assets/icons/: {missing_svgs}")
            if unregistered_svgs:
                errors.append(
                    f"Icon parity violation: SVG files in assets/icons/ not registered in IconName: {unregistered_svgs}"
                )

    # 4. Convention & Hot-Path Scanner across src/*.rs (excluding #[cfg(test)], tests/, benches/, build.rs)
    untracked_rs, modified_lines = get_modified_rust_lines(repo_root)
    crates_dir = repo_root / "crates"

    def is_newly_modified(rel_path: str, line_no: int) -> bool:
        return rel_path in untracked_rs or line_no in modified_lines.get(rel_path, set())

    for rs_file in sorted(crates_dir.rglob("*.rs")):
        if any(part in IGNORED_DIRS for part in rs_file.parts):
            continue
        rel = str(rs_file.relative_to(repo_root)).replace("\\", "/")
        if "/tests/" in rel or "/benches/" in rel or rel.startswith("crates/benchmarks/") or rel.endswith("/build.rs"):
            continue

        source = rs_file.read_text(encoding="utf-8", errors="replace")
        items, tokens = parse_rust_skeleton(source)
        test_ranges = get_test_line_ranges(source)
        lines = source.splitlines()

        def in_test_block(line_no: int) -> bool:
            return any(s <= line_no <= e for s, e in test_ranges)

        # 4a. Scan token pairs for `.unwrap()` and `.expect(`
        n_tok = len(tokens)
        for idx in range(n_tok - 2):
            t0, t1, t2 = tokens[idx], tokens[idx + 1], tokens[idx + 2]
            if in_test_block(t0.line):
                continue
            if t0.kind == "PUNCT" and t0.text == "." and t1.kind == "IDENT" and t1.text in ("unwrap", "expect"):
                if t2.kind == "PUNCT" and t2.text == "(":
                    if rel == "crates/ui_gui/src/main.rs" and t1.text == "expect":
                        continue
                    snippet = lines[t0.line - 1].strip() if 0 < t0.line <= len(lines) else ""
                    msg = f"Runtime `.{t1.text}()` outside #[cfg(test)]: {snippet[:100]}"
                    if is_newly_modified(rel, t0.line):
                        errors.append(f"[new-code:no-panic-runtime] {rel}:L{t0.line} — {msg}")
                    else:
                        warnings.append({
                            "rule": "no-panic-runtime",
                            "file": rel,
                            "line": t0.line,
                            "message": msg,
                        })

        # 4b. Core engine specific rules: zero-lookup BatchColumns & parking_lot locks
        if rel.startswith("crates/core_engine/src/"):
            for line_no, line_txt in enumerate(lines, start=1):
                if in_test_block(line_no):
                    continue
                stripped = line_txt.strip()
                if stripped.startswith("//"):
                    continue
                if ".column_by_name(" in stripped:
                    enc = _enclosing_fn(items, line_no) or ""
                    enc_leaf = enc.split("::")[-1]
                    if enc_leaf not in ALLOWED_COLUMN_BY_NAME_FNS:
                        errors.append(
                            f"Zero-lookup BatchColumns violation at {rel}:L{line_no} (in {enc or 'top-level'}): "
                            f"use BatchColumns::extract or ColumnView once per RecordBatch instead of `.column_by_name(`"
                        )
                if "std::sync::Mutex" in stripped or "std::sync::RwLock" in stripped:
                    msg = f"Use parking_lot::{{Mutex, RwLock}} in core_engine instead of std::sync: {stripped[:100]}"
                    if is_newly_modified(rel, line_no):
                        errors.append(f"[new-code:engine-lock-type] {rel}:L{line_no} — {msg}")
                    else:
                        warnings.append({
                            "rule": "engine-lock-type",
                            "file": rel,
                            "line": line_no,
                            "message": msg,
                        })

        # 4c. Scan hardcoded Color32::from_rgb* in ui_gui views/components outside theme/ & window_controls/
        if rel.startswith("crates/ui_gui/src/views/") or rel.startswith("crates/ui_gui/src/components/"):
            if "/window_controls/" not in rel:
                for line_no, line_txt in enumerate(lines, start=1):
                    if in_test_block(line_no):
                        continue
                    stripped = line_txt.strip()
                    if stripped.startswith("//"):
                        continue
                    if "Color32::from_rgb" in stripped:
                        msg = f"Hardcoded Color32::from_rgb* outside theme/: {stripped[:100]}"
                        if is_newly_modified(rel, line_no):
                            errors.append(f"[new-code:ui-theme-tokens] {rel}:L{line_no} — {msg}")
                        else:
                            warnings.append({
                                "rule": "ui-theme-tokens",
                                "file": rel,
                                "line": line_no,
                                "message": msg,
                            })

        # 4d. Scan std::thread::sleep in async/engine/ui code (outside tests)
        if any(rel.startswith(p) for p in ("crates/core_engine/", "crates/driver_", "crates/ui_gui/")):
            for line_no, line_txt in enumerate(lines, start=1):
                if in_test_block(line_no):
                    continue
                stripped = line_txt.strip()
                if stripped.startswith("//"):
                    continue
                if "std::thread::sleep" in stripped:
                    msg = f"Blocking std::thread::sleep in async/UI crate: {stripped[:100]}"
                    if is_newly_modified(rel, line_no):
                        errors.append(f"[new-code:no-blocking-sleep] {rel}:L{line_no} — {msg}")
                    else:
                        warnings.append({
                            "rule": "no-blocking-sleep",
                            "file": rel,
                            "line": line_no,
                            "message": msg,
                        })

    if len(warnings) > BASELINE_WARNING_BUDGET:
        errors.append(
            f"Convention warning ratchet exceeded: {len(warnings)} warnings > baseline budget {BASELINE_WARNING_BUDGET}"
        )

    return errors, warnings


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Verify 5-Tier DAG & conventions for uwulog-rust")
    parser.add_argument("--dag-only", action="store_true", help="Only run 5-Tier DAG architecture check")
    parser.add_argument("--conv-only", action="store_true", help="Only run parity & convention checks")
    parser.add_argument("--strict", action="store_true", help="Treat legacy advisory warnings as failures too")
    parser.add_argument("--json", action="store_true", help="Output machine-readable JSON")
    args = parser.parse_args(argv)

    repo_root = find_repo_root()
    dag_errors: List[str] = []
    dag_edges: List[dict] = []
    conv_errors: List[str] = []
    conv_warnings: List[dict] = []

    if not args.conv_only:
        try:
            dag_errors, dag_edges = check_dag(repo_root)
        except Exception as exc:
            dag_errors.append(str(exc))

    if not args.dag_only:
        conv_errors, conv_warnings = check_invariants_and_conventions(repo_root)

    all_errors = dag_errors + conv_errors
    failed = len(all_errors) > 0 or (args.strict and len(conv_warnings) > 0)

    if args.json:
        print(
            json.dumps(
                {
                    "ok": not failed,
                    "errors": all_errors,
                    "warnings_count": len(conv_warnings),
                    "baseline_warning_budget": BASELINE_WARNING_BUDGET,
                    "warnings": conv_warnings,
                    "dag_edges_count": len(dag_edges),
                    "dag_edges": dag_edges,
                },
                indent=2,
            )
        )
        return 1 if failed else 0

    print("=== uwulog-rust Architecture & Convention Guard ===")
    if not args.conv_only:
        print(
            f"[1/2] 5-Tier DAG & Taxonomy Check: {len(dag_edges)} internal edges checked — "
            f"{'PASS' if not dag_errors else f'FAIL ({len(dag_errors)} errors)'}"
        )
        for err in dag_errors:
            print(f"  [ERROR] {err}")

    if not args.dag_only:
        print(
            f"[2/2] Parity, Invariants & Diff-Aware Convention Guard: "
            f"{'PASS' if not conv_errors else f'FAIL ({len(conv_errors)} errors)'} "
            f"({len(conv_warnings)}/{BASELINE_WARNING_BUDGET} legacy advisory warnings)"
        )
        for err in conv_errors:
            print(f"  [ERROR] {err}")
        for w in conv_warnings:
            tag = "ERROR" if args.strict else "WARN"
            print(f"  [{tag}:{w['rule']}] {w['file']}:L{w['line']} — {w['message']}")

    print(f"Result: {'FAILED' if failed else 'PASSED'}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
