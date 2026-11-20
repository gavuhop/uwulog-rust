#!/usr/bin/env python3
"""
Stop hook quality gate for uwulog-rust (.agents/hooks.json).
When the agent is about to finish a normal turn (`NO_TOOL_CALL`, not already continuing)
and Rust/Cargo/SVG icon files have uncommitted changes, runs verify_arch_and_conv.py.
If hard architecture/convention errors exist, asks the agent to continue and fix them.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import verify_arch_and_conv


def _is_arch_relevant_path(rel_path: str) -> bool:
    norm = rel_path.replace("\\", "/").lstrip("./")
    if norm == "Cargo.toml":
        return True
    if norm.startswith("crates/") and (norm.endswith(".rs") or norm.endswith("/Cargo.toml")):
        return True
    if norm.startswith("assets/icons/") and norm.endswith(".svg"):
        return True
    return False


def get_relevant_dirty_files(repo_root: Path) -> List[str]:
    try:
        res = subprocess.run(
            ["git", "status", "--porcelain", "--", "crates/", "Cargo.toml", "assets/icons/"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        if res.returncode != 0:
            return []
    except Exception:
        return []

    dirty: List[str] = []
    for raw_line in res.stdout.splitlines():
        if len(raw_line) < 4:
            continue
        path_part = raw_line[3:].strip()
        if " -> " in path_part:
            path_part = path_part.split(" -> ", 1)[1].strip()
        path_part = path_part.strip('"')
        if _is_arch_relevant_path(path_part):
            dirty.append(path_part)
    return dirty


def run_arch_check(repo_root: Path) -> List[str]:
    dag_errors, _ = verify_arch_and_conv.check_dag(repo_root)
    conv_errors, _ = verify_arch_and_conv.check_invariants_and_conventions(repo_root)
    return dag_errors + conv_errors


def evaluate_stop(
    payload: Dict[str, Any],
    dirty_fn: Callable[[Path], List[str]] = get_relevant_dirty_files,
    check_fn: Callable[[Path], List[str]] = run_arch_check,
) -> Dict[str, Any]:
    # Never block again if we already asked the agent to continue once this turn
    if payload.get("continuationActive"):
        return {}

    termination_reason = str(payload.get("terminationReason") or "")
    if termination_reason and termination_reason != "NO_TOOL_CALL":
        return {}

    cwd_raw = payload.get("cwd") or (payload.get("workspacePaths") or [None])[0]
    repo_root = (
        Path(str(cwd_raw)).resolve()
        if cwd_raw
        else Path(__file__).resolve().parents[3].resolve()
    )

    dirty = dirty_fn(repo_root)
    if not dirty:
        return {}

    try:
        errors = check_fn(repo_root)
    except Exception:
        # Fail-open on unexpected hook runtime issues
        return {}

    if not errors:
        return {}

    shown = errors[:12]
    extra = f"\n  ... and {len(errors) - len(shown)} more error(s)." if len(errors) > len(shown) else ""
    bullet_lines = "\n".join(f"  - {e}" for e in shown)
    reason = (
        "uwu-stop-arch-gate detected architecture/convention failures in modified files "
        f"({len(errors)} error(s)):\n{bullet_lines}{extra}\n"
        "Please fix these issues and re-run `./.agents/scripts/harness.sh check-arch` before finishing."
    )
    return {"decision": "continue", "reason": reason}


def main() -> int:
    raw = sys.stdin.read().strip()
    if not raw:
        print(json.dumps({}))
        return 0

    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        print(json.dumps({}))
        return 0

    if not isinstance(payload, dict):
        print(json.dumps({}))
        return 0

    print(json.dumps(evaluate_stop(payload), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
