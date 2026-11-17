#!/usr/bin/env bash
set -euo pipefail

python3 -c '
import json, subprocess, sys
from pathlib import Path

raw = sys.stdin.read().strip()
payload = json.loads(raw) if raw else {}
cwd = payload.get("cwd") or (payload.get("workspacePaths") or [None])[0] or str(Path.cwd().parent)

branch = "unknown"
dirty_count = 0
try:
    b = subprocess.run(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=cwd, capture_output=True, text=True, check=False)
    if b.returncode == 0:
        branch = b.stdout.strip()
    s = subprocess.run(["git", "status", "--porcelain"], cwd=cwd, capture_output=True, text=True, check=False)
    if s.returncode == 0:
        dirty_count = len([l for l in s.stdout.splitlines() if l.strip()])
except Exception:
    pass

msg = (
    f"[uwulog-rust workspace] branch={branch}, dirty_files={dirty_count}. "
    "Rules loaded from .agents/rules/GEMINI.md. "
    "Use ./.agents/scripts/harness.sh (tree | ast-skeleton | ast-symbol | ast-hotspots | check-arch | test-tier | verify) "
    "or uwu-dev-mcp tools before reading large .rs files."
)

print(json.dumps({
    "injectSteps": [
        {
            "systemMessage": {
                "systemMessage": msg
            }
        }
    ]
}))
'
