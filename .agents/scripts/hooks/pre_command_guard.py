#!/usr/bin/env python3
"""
PreToolUse guard for `run_command` in uwulog-rust (.agents/hooks.json).
Reads camelCase Antigravity hook payload from stdin and outputs `{"decision": "allow" | "deny"}`.
"""

from __future__ import annotations

import json
import re
import sys
from typing import Optional

DESTRUCTIVE_PATTERNS = [
    (r"\bgit\s+reset\s+--hard\b", "git reset --hard discards uncommitted work"),
    (r"\bgit\s+clean\b[^\n;|&]*(?:\s-[a-zA-Z]*f|\s--force\b)", "git clean -f/-fd/-df removes untracked files/directories"),
    (r"\bgit\s+push\b[^\n;|&]*(?:\s--force\b|\s-f\b)", "force-pushing requires explicit manual execution"),
    (
        r"\brm\s+(?:-[a-zA-Z]*[rR][a-zA-Z]*\s+|-[a-zA-Z]*f[a-zA-Z]*\s+|--(?:recursive|force)\s+)+"
        r"(?:/|\*|~|\.\.?|\.agents|crates)(?=/|\s|$|;|&|\||\"|')",
        "recursive/force delete targeting critical workspace or system paths",
    ),
    (r"\bcargo\s+publish\b", "cargo publish requires explicit manual release authorization"),
]


def check_command(cmd: str) -> Optional[str]:
    for pattern, reason in DESTRUCTIVE_PATTERNS:
        if re.search(pattern, cmd):
            return reason
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

    tool_call = payload.get("toolCall") or {}
    args = tool_call.get("args") or {}
    cmd = str(args.get("CommandLine") or args.get("command") or "")

    reason = check_command(cmd)
    if reason:
        print(
            json.dumps(
                {
                    "decision": "deny",
                    "reason": f"Blocked by uwu-command-guard: {reason}. Run manually if truly intended.",
                }
            )
        )
        return 0

    print(json.dumps({"decision": "allow"}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
