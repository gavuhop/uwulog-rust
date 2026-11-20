#!/usr/bin/env python3
"""
SessionStart hook for uwulog-rust (.agents/hooks.json).
Cross-platform (Linux / macOS / Windows) replacement for session_start.sh.
Reads camelCase Antigravity hook payload from stdin and injects workspace context.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, Tuple


def get_git_status(cwd: Path) -> Tuple[str, int]:
    branch = "unknown"
    dirty_count = 0
    try:
        b = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        if b.returncode == 0 and b.stdout.strip():
            branch = b.stdout.strip()

        s = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        if s.returncode == 0:
            dirty_count = len([line for line in s.stdout.splitlines() if line.strip()])
    except Exception:
        pass
    return branch, dirty_count


def build_response(payload: Dict[str, Any]) -> Dict[str, Any]:
    raw_cwd = (
        payload.get("cwd")
        or (payload.get("workspacePaths") or [None])[0]
        or str(Path(__file__).resolve().parents[3])
    )
    branch, dirty_count = get_git_status(Path(str(raw_cwd)))
    msg = (
        f"[uwulog-rust workspace] branch={branch}, dirty_files={dirty_count}. "
        "Rules loaded from .agents/rules/GEMINI.md (including Interactive Discovery & Artifact Plan Gate). "
        "Use ./.agents/scripts/harness.sh (tree | ast-skeleton | ast-symbol | ast-hotspots | affected | "
        "check-arch | test-changed | test-tier | self-test | verify) or uwu-dev-mcp tools before reading large .rs files."
    )
    return {
        "injectSteps": [
            {
                "systemMessage": {
                    "systemMessage": msg,
                }
            }
        ]
    }


def main() -> int:
    raw = sys.stdin.read().strip()
    try:
        payload = json.loads(raw) if raw else {}
    except json.JSONDecodeError:
        payload = {}
    if not isinstance(payload, dict):
        payload = {}

    print(json.dumps(build_response(payload), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
