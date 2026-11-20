#!/usr/bin/env python3
"""
uwu_mcp_server.py — Local Project MCP Server (`uwu-dev-mcp`) for uwulog-rust.
Speaks standard MCP JSON-RPC 2.0 over stdio (newline-delimited JSON).
Thin adapter delegating all AST/Architecture/Harness operations to `.agents/scripts/`.

Exposes 8 semantic, AST token-optimizer, architecture & harness tools:
  1. uwu_tree_map
  2. uwu_ast_skeleton
  3. uwu_find_symbol
  4. uwu_ast_hotspots
  5. uwu_inspect_dag_and_conv
  6. uwu_standard_fields_info
  7. uwu_keymap_lookup
  8. uwu_run_harness
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

REPO_ROOT = Path(__file__).resolve().parents[3]
SCRIPTS_DIR = REPO_ROOT / ".agents" / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import ast_index
import verify_arch_and_conv

SERVER_NAME = "uwu-dev-mcp"
SERVER_VERSION = "0.2.0"
DEFAULT_PROTOCOL_VERSION = "2024-11-05"


# ---------------------------------------------------------------------------
# Tool Implementations (Thin Adapters to ast_index & verify_arch_and_conv)
# ---------------------------------------------------------------------------

def _capture_ast_call(fn, ns_kwargs: Dict[str, Any]) -> str:
    ns = argparse.Namespace(**ns_kwargs)
    buf = io.StringIO()
    err_buf = io.StringIO()
    with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(err_buf):
        rc = fn(ns, REPO_ROOT)
    out = buf.getvalue().strip()
    err = err_buf.getvalue().strip()
    if rc != 0:
        raise RuntimeError(err or out or f"ast_index exited with code {rc}")
    return out


def tool_tree_map(args: Dict[str, Any]) -> str:
    return _capture_ast_call(
        ast_index.cmd_tree,
        {
            "target": args.get("target"),
            "tier": args.get("tier"),
            "crate": args.get("crate_name"),
            "json": bool(args.get("json", False)),
        },
    )


def tool_ast_skeleton(args: Dict[str, Any]) -> str:
    target = args.get("target")
    if not target:
        raise ValueError("'target' is required")
    return _capture_ast_call(
        ast_index.cmd_skeleton,
        {
            "target": str(target),
            "tests": bool(args.get("include_tests", False)),
            "pub_only": bool(args.get("pub_only", False)),
            "json": bool(args.get("json", False)),
        },
    )


def tool_find_symbol(args: Dict[str, Any]) -> str:
    name = args.get("name")
    if not name:
        raise ValueError("'name' is required")
    return _capture_ast_call(
        ast_index.cmd_symbol,
        {
            "name": str(name),
            "crate": args.get("crate_name"),
            "defs_only": bool(args.get("defs_only", False)),
            "fuzzy": bool(args.get("fuzzy", False)),
            "limit": int(args.get("limit", 40)),
            "json": bool(args.get("json", False)),
        },
    )


def tool_ast_hotspots(args: Dict[str, Any]) -> str:
    return _capture_ast_call(
        ast_index.cmd_hotspots,
        {
            "target": args.get("target"),
            "top": int(args.get("top", 15)),
            "tests": bool(args.get("include_tests", False)),
            "json": bool(args.get("json", False)),
        },
    )


def tool_inspect_dag_and_conv(args: Dict[str, Any]) -> str:
    argv: List[str] = ["--json"]
    if args.get("dag_only"):
        argv.append("--dag-only")
    if args.get("conv_only"):
        argv.append("--conv-only")
    if args.get("strict"):
        argv.append("--strict")
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        verify_arch_and_conv.main(argv)
    return buf.getvalue().strip()


def tool_standard_fields_info(args: Dict[str, Any]) -> str:
    return _capture_ast_call(
        ast_index.cmd_fields,
        {
            "filter": args.get("filter"),
            "json": True,
        },
    )


def tool_keymap_lookup(args: Dict[str, Any]) -> str:
    return _capture_ast_call(
        ast_index.cmd_keymap,
        {
            "query": args.get("query"),
            "context": args.get("context"),
            "json": True,
        },
    )


def _truncate_head_tail(text: str, head: int = 40, tail: int = 80) -> str:
    lines = text.splitlines()
    if len(lines) <= head + tail:
        return text
    omitted = len(lines) - (head + tail)
    return "\n".join(
        [
            *lines[:head],
            f"... ({omitted} middle line(s) omitted; showing first {head} + last {tail} lines) ...",
            *lines[-tail:],
        ]
    )


def tool_run_harness(args: Dict[str, Any]) -> str:
    action = str(args.get("action") or "").strip()
    allowed = {"check-arch", "fmt", "clippy", "test-tier", "test-changed", "self-test"}
    if action not in allowed:
        raise ValueError(f"Unsupported harness action '{action}'. Allowed: {sorted(allowed)}")

    cmd = ["bash", str(SCRIPTS_DIR / "harness.sh"), action]
    if action == "test-tier":
        tier = str(args.get("tier") or "all").strip()
        if tier not in {"0", "1", "2", "3", "4", "all"}:
            raise ValueError("Invalid 'tier'; expected 0, 1, 2, 3, 4, or all")
        cmd.append(tier)
    elif action == "test-changed":
        if args.get("downstream"):
            cmd.append("--downstream")

    res = subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
    )
    combined = (res.stdout + ("\n" + res.stderr if res.stderr else "")).strip()
    combined = _truncate_head_tail(combined, head=40, tail=80)

    return json.dumps(
        {
            "command": " ".join(cmd),
            "exit_code": res.returncode,
            "ok": res.returncode == 0,
            "output": combined,
        },
        indent=2,
        ensure_ascii=False,
    )


TOOLS: List[Dict[str, Any]] = [
    {
        "name": "uwu_tree_map",
        "description": "Return a token-optimized 5-Tier directory and Rust module map with LOC and top-level type/function summaries.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "target": {"type": "string", "description": "Optional crate or directory filter"},
                "tier": {"type": "integer", "minimum": 0, "maximum": 4, "description": "Filter by DAG Tier (0..4)"},
                "crate_name": {"type": "string", "description": "Filter by crate name (e.g. uwu-core-engine)"},
                "json": {"type": "boolean", "default": False},
            },
        },
    },
    {
        "name": "uwu_ast_skeleton",
        "description": "Extract Rust structs, enums, traits, impls, and fn signatures with [Lstart-Lend] line ranges while omitting function bodies (85-95% token reduction).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "target": {"type": "string", "description": "File path, directory, or crate name"},
                "include_tests": {"type": "boolean", "default": False},
                "pub_only": {"type": "boolean", "default": False},
                "json": {"type": "boolean", "default": False},
            },
            "required": ["target"],
        },
    },
    {
        "name": "uwu_find_symbol",
        "description": "Locate cross-crate definitions ([Lstart-Lend]) and reference/call sites with enclosing function context.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Exact symbol name (or substring when fuzzy=true)"},
                "crate_name": {"type": "string", "description": "Optional crate filter"},
                "defs_only": {"type": "boolean", "default": False},
                "fuzzy": {"type": "boolean", "default": False},
                "limit": {"type": "integer", "default": 40},
                "json": {"type": "boolean", "default": False},
            },
            "required": ["name"],
        },
    },
    {
        "name": "uwu_ast_hotspots",
        "description": "Identify largest Rust files, longest functions, and concurrency/parallelism/unsafe sites (RwLock, Mutex, Atomic, par_iter, unsafe).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "target": {"type": "string", "description": "Optional crate or directory filter"},
                "top": {"type": "integer", "default": 15},
                "include_tests": {"type": "boolean", "default": False},
                "json": {"type": "boolean", "default": False},
            },
        },
    },
    {
        "name": "uwu_inspect_dag_and_conv",
        "description": "Verify the 5-Tier 1-Way DAG dependency rules, Expr parity across core_filter/core_engine, SSOT schema, and convention invariants.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "dag_only": {"type": "boolean", "default": False},
                "conv_only": {"type": "boolean", "default": False},
                "strict": {"type": "boolean", "default": False},
            },
        },
    },
    {
        "name": "uwu_standard_fields_info",
        "description": "Inspect live StandardField SSOT canonical names, aliases, FieldType, and Arrow core '__' system columns.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": {"type": "string", "description": "Optional substring filter on field name or alias"},
            },
        },
    },
    {
        "name": "uwu_keymap_lookup",
        "description": "Inspect all KeyAction variants, KeyContext hierarchy, and default keybindings in core_keymap to prevent shortcut conflicts.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Optional filter on action name, alias, or keystroke"},
                "context": {"type": "string", "description": "Optional KeyContext filter (e.g. Global, Table, Autocomplete)"},
            },
        },
    },
    {
        "name": "uwu_run_harness",
        "description": "Execute scoped workspace checks via .agents/scripts/harness.sh (check-arch, fmt, clippy, test-tier, test-changed, self-test) with head+tail concise output.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["check-arch", "fmt", "clippy", "test-tier", "test-changed", "self-test"],
                },
                "tier": {
                    "type": "string",
                    "enum": ["0", "1", "2", "3", "4", "all"],
                    "description": "Used when action='test-tier'",
                },
                "downstream": {
                    "type": "boolean",
                    "default": False,
                    "description": "Used when action='test-changed' to also test downstream DAG crates",
                },
            },
            "required": ["action"],
        },
    },
]

TOOL_HANDLERS = {
    "uwu_tree_map": tool_tree_map,
    "uwu_ast_skeleton": tool_ast_skeleton,
    "uwu_find_symbol": tool_find_symbol,
    "uwu_ast_hotspots": tool_ast_hotspots,
    "uwu_inspect_dag_and_conv": tool_inspect_dag_and_conv,
    "uwu_standard_fields_info": tool_standard_fields_info,
    "uwu_keymap_lookup": tool_keymap_lookup,
    "uwu_run_harness": tool_run_harness,
}


# ---------------------------------------------------------------------------
# JSON-RPC 2.0 Stdio Transport Loop
# ---------------------------------------------------------------------------

def _send_response(payload: Dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def handle_request(req: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    req_id = req.get("id")
    method = req.get("method", "")
    params = req.get("params") or {}

    if req_id is None:
        return None

    if method == "initialize":
        client_proto = params.get("protocolVersion") or DEFAULT_PROTOCOL_VERSION
        return {
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": client_proto,
                "capabilities": {"tools": {"listChanged": False}},
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
            },
        }

    if method == "ping":
        return {"jsonrpc": "2.0", "id": req_id, "result": {}}

    if method == "tools/list":
        return {"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}}

    if method == "tools/call":
        tool_name = params.get("name", "")
        tool_args = params.get("arguments") or {}
        handler = TOOL_HANDLERS.get(tool_name)
        if not handler:
            return {
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {"code": -32602, "message": f"Unknown tool: {tool_name}"},
            }
        try:
            text_out = handler(tool_args)
            return {
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "content": [{"type": "text", "text": text_out}],
                    "isError": False,
                },
            }
        except Exception as exc:
            return {
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "content": [{"type": "text", "text": f"Tool error ({tool_name}): {exc}"}],
                    "isError": True,
                },
            }

    return {
        "jsonrpc": "2.0",
        "id": req_id,
        "error": {"code": -32601, "message": f"Method not found: {method}"},
    }


def main() -> int:
    os.chdir(REPO_ROOT)
    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as exc:
            _send_response({
                "jsonrpc": "2.0",
                "id": None,
                "error": {"code": -32700, "message": f"Parse error: {exc}"},
            })
            continue
        resp = handle_request(req)
        if resp is not None:
            _send_response(resp)
    return 0


if __name__ == "__main__":
    sys.exit(main())
