#!/usr/bin/env python3
"""
PreToolUse guard for `write_file` in uwulog-rust (.agents/hooks.json).
Prevents plan/spec draft markdown files (e.g. plan.md, implementation_plan.md) from being
written inside the git repository; they must be stored as Artifact Plans in artifactDirectoryPath.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, Optional

PLAN_FILENAME_RE = re.compile(
    r"^(?:plan|implementation_plan|feature_plan|.*_plan|.*-plan)\.md$",
    re.IGNORECASE,
)


def _is_within(child: Path, parent: Path) -> bool:
    try:
        child.relative_to(parent)
        return True
    except ValueError:
        return False


def check_write_payload(payload: Dict[str, Any]) -> Optional[str]:
    tool_call = payload.get("toolCall") or {}
    args = tool_call.get("args") or {}
    raw_path = args.get("Path") or args.get("path")
    if not raw_path:
        return None

    cwd_raw = payload.get("cwd") or (payload.get("workspacePaths") or [None])[0]
    repo_root = (
        Path(str(cwd_raw)).resolve()
        if cwd_raw
        else Path(__file__).resolve().parents[3].resolve()
    )

    target = Path(str(raw_path))
    if not target.is_absolute():
        target = repo_root / target
    target = target.resolve()

    artifact_dir_raw = payload.get("artifactDirectoryPath")
    if artifact_dir_raw:
        artifact_dir = Path(str(artifact_dir_raw)).resolve()
        if _is_within(target, artifact_dir):
            return None

    if not _is_within(target, repo_root):
        return None

    if PLAN_FILENAME_RE.match(target.name):
        rel = str(target.relative_to(repo_root)).replace("\\", "/")
        return (
            f"Blocked by uwu-write-guard: '{rel}' looks like a plan file inside the git repository. "
            "Save plans as user-facing Artifact Plans inside artifactDirectoryPath (with ArtifactMetadata), "
            "never inside the repository."
        )

    return None


def main() -> int:
    raw = sys.stdin.read().strip()
    if not raw:
        print(json.dumps({"decision": "allow"}))
        return 0

    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        print(json.dumps({"decision": "allow"}))
        return 0

    if not isinstance(payload, dict):
        print(json.dumps({"decision": "allow"}))
        return 0

    reason = check_write_payload(payload)
    if reason:
        print(json.dumps({"decision": "deny", "reason": reason}, ensure_ascii=False))
        return 0

    print(json.dumps({"decision": "allow"}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
