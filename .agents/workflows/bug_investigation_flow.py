"""
bug_investigation_flow.py — Structured Root-Cause, Reproduction & Fix Workflow (`run_workflow`).

Pass `Args={"bug": "<symptom or failing behavior>"}` when running via `run_workflow`.
"""

bug_desc = (args or {}).get("bug", "").strip()
if not bug_desc:
    raise ValueError(
        "Missing required Args['bug'] describing the issue to investigate."
    )

phase("1. Symbol & Flow Trace")
trace = await agent(
    prompt=f"""
Investigate this bug in uwulog-rust without editing files yet:
{bug_desc}

Steps:
1. Run `./.agents/scripts/harness.sh tree` and `ast-symbol` / `ast-skeleton` to trace the relevant call path across Tiers 0..4.
2. Inspect only the targeted line slices with `read_file`.
3. Identify the exact root cause, affected crates/lines, and how to reproduce it in a deterministic unit/integration test.
""",
    schema={"type": "string"},
    role="Root-Cause Tracer",
)
log("Root-Cause Analysis:\n", trace)

phase("2. Regression Test & Fix")
fix_result = await agent(
    prompt=f"""
Fix the bug traced below and add a regression test:
Bug: {bug_desc}

Root-Cause Analysis:
{trace}

Steps:
1. Write a failing reproduction test first in the appropriate crate (or `pipeline_integration.rs` if cross-crate).
2. Implement the minimal, idiomatic fix respecting 5-Tier DAG and project conventions (.agents/rules/GEMINI.md).
3. Run `./.agents/scripts/harness.sh verify` to confirm all checks pass.
""",
    schema={"type": "string"},
    role="Fix & Regression Tester",
)
log("Fix Summary:\n", fix_result)
