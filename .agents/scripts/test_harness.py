#!/usr/bin/env python3
"""
Fast stdlib unittest self-test suite for the uwulog-rust `.agents/` harness.
Covers:
  - Hooks: session_start.py, pre_command_guard.py, pre_write_guard.py, stop_arch_gate.py
  - AST & Affected crate indexer: ast_index.py
  - Architecture & Convention Guard: verify_arch_and_conv.py
  - Local MCP Server: uwu_mcp_server.py (JSON-RPC + head/tail truncation)
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import sys
import unittest
from pathlib import Path
from unittest import mock

SCRIPTS_DIR = Path(__file__).resolve().parent
HOOKS_DIR = SCRIPTS_DIR / "hooks"
REPO_ROOT = SCRIPTS_DIR.parent.parent
MCP_DIR = REPO_ROOT / ".agents" / "plugins" / "uwu-dev-kit"

for p in (SCRIPTS_DIR, HOOKS_DIR, MCP_DIR):
    if str(p) not in sys.path:
        sys.path.insert(0, str(p))

import ast_index
import pre_command_guard
import pre_write_guard
import session_start
import stop_arch_gate
import uwu_mcp_server
import verify_arch_and_conv


class TestPreCommandGuard(unittest.TestCase):
    def test_blocks_destructive_commands(self) -> None:
        dangerous = [
            "git reset --hard HEAD~1",
            "git clean -fd",
            "git push origin main --force",
            "git push -f",
            "rm -rf crates/",
            "rm -rf .agents",
            "cargo publish",
        ]
        for cmd in dangerous:
            with self.subTest(cmd=cmd):
                self.assertIsNotNone(pre_command_guard.check_command(cmd))

    def test_allows_safe_commands(self) -> None:
        safe = [
            "git status --short",
            "cargo test -p uwu-core-filter",
            "git push origin main",
            "rm -f /tmp/scratch.txt",
            "./.agents/scripts/harness.sh check-arch",
        ]
        for cmd in safe:
            with self.subTest(cmd=cmd):
                self.assertIsNone(pre_command_guard.check_command(cmd))


class TestPreWriteGuard(unittest.TestCase):
    def test_blocks_plan_files_inside_repo(self) -> None:
        artifact_dir = "/tmp/fake-brain/conv-123"
        for rel in ("plan.md", "PLAN.md", ".agents/implementation_plan.md", "docs/feature_plan.md"):
            payload = {
                "cwd": str(REPO_ROOT),
                "artifactDirectoryPath": artifact_dir,
                "toolCall": {"name": "write_file", "args": {"Path": str(REPO_ROOT / rel)}},
            }
            with self.subTest(rel=rel):
                reason = pre_write_guard.check_write_payload(payload)
                self.assertIsNotNone(reason)

    def test_allows_plan_inside_artifact_dir_and_normal_repo_files(self) -> None:
        artifact_dir = "/tmp/fake-brain/conv-123"
        allowed_paths = [
            f"{artifact_dir}/plan.md",
            f"{artifact_dir}/implementation_plan.md",
            str(REPO_ROOT / "README.md"),
            str(REPO_ROOT / "crates/core_util/src/lib.rs"),
            str(REPO_ROOT / ".agents/docs/AGENT_WORKFLOWS_AND_HARNESS.md"),
        ]
        for path in allowed_paths:
            payload = {
                "cwd": str(REPO_ROOT),
                "artifactDirectoryPath": artifact_dir,
                "toolCall": {"name": "write_file", "args": {"Path": path}},
            }
            with self.subTest(path=path):
                self.assertIsNone(pre_write_guard.check_write_payload(payload))

    def test_blocks_plan_when_cwd_is_subdirectory(self) -> None:
        sub_cwd = str(REPO_ROOT / "crates" / "ui_gui")
        payload_abs = {
            "cwd": sub_cwd,
            "toolCall": {"name": "write_file", "args": {"Path": str(REPO_ROOT / "plan.md")}},
        }
        self.assertIsNotNone(pre_write_guard.check_write_payload(payload_abs))

        payload_rel = {
            "cwd": sub_cwd,
            "toolCall": {"name": "write_file", "args": {"Path": "../../plan.md"}},
        }
        self.assertIsNotNone(pre_write_guard.check_write_payload(payload_rel))


class TestStopArchGate(unittest.TestCase):
    def test_skips_when_continuation_active(self) -> None:
        res = stop_arch_gate.evaluate_stop(
            {"continuationActive": True, "terminationReason": "NO_TOOL_CALL", "cwd": str(REPO_ROOT)},
            dirty_fn=lambda _: ["crates/core_util/src/lib.rs"],
            check_fn=lambda _: ["should not run"],
        )
        self.assertEqual(res, {})

    def test_skips_on_user_cancel_or_error(self) -> None:
        for reason in ("USER_CANCELED", "ERROR", "MAX_INVOCATIONS"):
            res = stop_arch_gate.evaluate_stop(
                {"continuationActive": False, "terminationReason": reason, "cwd": str(REPO_ROOT)},
                dirty_fn=lambda _: ["crates/core_util/src/lib.rs"],
                check_fn=lambda _: ["should not run"],
            )
            self.assertEqual(res, {})

    def test_skips_when_no_rust_or_cargo_dirty(self) -> None:
        res = stop_arch_gate.evaluate_stop(
            {"continuationActive": False, "terminationReason": "NO_TOOL_CALL", "cwd": str(REPO_ROOT)},
            dirty_fn=lambda _: [],
            check_fn=lambda _: ["should not run"],
        )
        self.assertEqual(res, {})

    def test_requests_continue_when_arch_errors_found(self) -> None:
        res = stop_arch_gate.evaluate_stop(
            {"continuationActive": False, "terminationReason": "NO_TOOL_CALL", "cwd": str(REPO_ROOT)},
            dirty_fn=lambda _: ["crates/core_filter/src/parser.rs"],
            check_fn=lambda _: ["Expr parity violation: 'Expr::Foo' missing"],
        )
        self.assertEqual(res.get("decision"), "continue")
        self.assertIn("Expr::Foo", res.get("reason", ""))

    def test_allows_stop_when_arch_passes(self) -> None:
        res = stop_arch_gate.evaluate_stop(
            {"continuationActive": False, "terminationReason": "NO_TOOL_CALL", "cwd": str(REPO_ROOT)},
            dirty_fn=lambda _: ["crates/core_util/src/lib.rs"],
            check_fn=lambda _: [],
        )
        self.assertEqual(res, {})

    def test_skips_when_not_fully_idle(self) -> None:
        res = stop_arch_gate.evaluate_stop(
            {
                "continuationActive": False,
                "fullyIdle": False,
                "terminationReason": "NO_TOOL_CALL",
                "cwd": str(REPO_ROOT),
            },
            dirty_fn=lambda _: ["crates/core_util/src/lib.rs"],
            check_fn=lambda _: ["should not run"],
        )
        self.assertEqual(res, {})

    def test_run_arch_check_captures_dag_runtime_error_and_continues(self) -> None:
        with (
            mock.patch.object(
                verify_arch_and_conv, "check_dag", side_effect=RuntimeError("cargo metadata failed: bad toml")
            ),
            mock.patch.object(
                verify_arch_and_conv,
                "check_invariants_and_conventions",
                return_value=(["conv error"], []),
            ),
        ):
            errs = stop_arch_gate.run_arch_check(REPO_ROOT)
        self.assertEqual(len(errs), 2)
        self.assertIn("cargo metadata failed", errs[0])
        self.assertEqual(errs[1], "conv error")

    def test_resolves_repo_root_from_subdirectory_cwd(self) -> None:
        seen_roots: list[Path] = []

        def fake_dirty(root: Path) -> list[str]:
            seen_roots.append(root)
            return []

        stop_arch_gate.evaluate_stop(
            {
                "continuationActive": False,
                "fullyIdle": True,
                "terminationReason": "NO_TOOL_CALL",
                "cwd": str(REPO_ROOT / "crates" / "ui_gui"),
            },
            dirty_fn=fake_dirty,
            check_fn=lambda _: [],
        )
        self.assertEqual(seen_roots, [REPO_ROOT.resolve()])


class TestSessionStartHook(unittest.TestCase):
    def test_returns_valid_inject_steps(self) -> None:
        resp = session_start.build_response({"cwd": str(REPO_ROOT)})
        self.assertIn("injectSteps", resp)
        self.assertEqual(len(resp["injectSteps"]), 1)
        msg = resp["injectSteps"][0]["systemMessage"]["systemMessage"]
        self.assertIn("uwulog-rust workspace", msg)
        self.assertIn("GEMINI.md", msg)


class TestAstIndexAndAffected(unittest.TestCase):
    def test_parse_rust_skeleton_extracts_items_and_test_ranges(self) -> None:
        sample = """
pub struct Foo {
    pub x: i32,
}

impl Foo {
    pub fn bar(&self) -> i32 {
        self.x + 1
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bar() {}
}
"""
        items, _ = ast_index.parse_rust_skeleton(sample)
        names = {(it.kind, it.name) for it in items}
        self.assertIn(("struct", "Foo"), names)
        self.assertIn(("impl", "Foo"), names)
        impl_item = next(it for it in items if it.kind == "impl" and it.name == "Foo")
        child_names = {(c.kind, c.name) for c in impl_item.children}
        self.assertIn(("fn", "bar"), child_names)
        test_ranges = ast_index.get_test_line_ranges(sample)
        self.assertTrue(len(test_ranges) >= 1)

    def test_compute_affected_crates_direct_and_downstream(self) -> None:
        direct_only = ast_index.compute_affected_crates(
            REPO_ROOT,
            changed_files=["crates/core_filter/src/parser.rs"],
            include_downstream=False,
        )
        self.assertFalse(direct_only["test_all"])
        self.assertEqual(direct_only["affected_crates"], ["uwu-core-filter"])

        with_down = ast_index.compute_affected_crates(
            REPO_ROOT,
            changed_files=["crates/core_filter/src/parser.rs"],
            include_downstream=True,
        )
        self.assertIn("uwu-core-filter", with_down["affected_crates"])
        self.assertIn("uwu-core-engine", with_down["downstream_crates"])

    def test_compute_affected_icons_and_root_cargo(self) -> None:
        icons_res = ast_index.compute_affected_crates(
            REPO_ROOT,
            changed_files=["assets/icons/search.svg"],
            include_downstream=False,
        )
        self.assertEqual(icons_res["affected_crates"], ["uwu-icons"])

        root_res = ast_index.compute_affected_crates(
            REPO_ROOT,
            changed_files=["Cargo.toml"],
            include_downstream=False,
        )
        self.assertTrue(root_res["test_all"])

    def test_fields_and_keymap_commands_produce_json(self) -> None:
        for fn, ns in (
            (ast_index.cmd_fields, argparse.Namespace(filter=None, json=True)),
            (ast_index.cmd_keymap, argparse.Namespace(query=None, context=None, json=True)),
        ):
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                rc = fn(ns, REPO_ROOT)
            self.assertEqual(rc, 0)
            parsed = json.loads(buf.getvalue())
            self.assertIsInstance(parsed, dict)

    def test_cmd_affected_fails_on_invalid_base_ref(self) -> None:
        err_buf = io.StringIO()
        with contextlib.redirect_stderr(err_buf):
            rc = ast_index.cmd_affected(
                argparse.Namespace(base="nonexistent-ref-xyz-9999", downstream=False, json=True),
                REPO_ROOT,
            )
        self.assertEqual(rc, 1)
        self.assertIn("git diff --name-only", err_buf.getvalue())


class TestVerifyArchAndMcp(unittest.TestCase):
    def test_verify_arch_dag_only_json(self) -> None:
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = verify_arch_and_conv.main(["--dag-only", "--json"])
        self.assertEqual(rc, 0)
        data = json.loads(buf.getvalue())
        self.assertTrue(data["ok"])
        self.assertGreater(data["dag_edges_count"], 0)

    def test_mcp_initialize_list_and_call(self) -> None:
        init = uwu_mcp_server.handle_request({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
        self.assertIsNotNone(init)
        assert init is not None
        self.assertEqual(init["result"]["serverInfo"]["name"], "uwu-dev-mcp")

        listed = uwu_mcp_server.handle_request({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
        assert listed is not None
        tool_names = [t["name"] for t in listed["result"]["tools"]]
        self.assertEqual(len(tool_names), 8)
        self.assertIn("uwu_run_harness", tool_names)

        called = uwu_mcp_server.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "uwu_standard_fields_info", "arguments": {}},
            }
        )
        assert called is not None
        self.assertFalse(called["result"]["isError"])
        payload = json.loads(called["result"]["content"][0]["text"])
        self.assertIn("standard_fields", payload)

    def test_mcp_head_tail_truncation(self) -> None:
        lines = [f"line-{i}" for i in range(200)]
        truncated = uwu_mcp_server._truncate_head_tail("\n".join(lines), head=40, tail=80)
        out_lines = truncated.splitlines()
        self.assertEqual(len(out_lines), 40 + 1 + 80)
        self.assertEqual(out_lines[0], "line-0")
        self.assertEqual(out_lines[39], "line-39")
        self.assertIn("80 middle line(s) omitted", out_lines[40])
        self.assertEqual(out_lines[-1], "line-199")


if __name__ == "__main__":
    unittest.main()
