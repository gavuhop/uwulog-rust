"""
feature_delivery_pipeline.py — Staged 4-phase multi-agent workflow (`run_workflow`)
for implementing a cross-tier feature in uwulog-rust AFTER interactive discovery & approval.

Prerequisite:
  Complete `.agents/skills/uwu-feature-discovery/SKILL.md` with the developer and obtain
  explicit approval on the Artifact Plan (`plan.md`) BEFORE running this workflow.

Pass `Args={"approved_spec": "<approved plan & decisions>"}` when running via `run_workflow`.
"""

approved_spec = (args or {}).get("approved_spec", "").strip()
if not approved_spec:
    raise ValueError(
        "Missing required Args['approved_spec']. Complete interactive discovery "
        "(.agents/skills/uwu-feature-discovery/SKILL.md) and get developer approval "
        "on the Artifact Plan before running feature_delivery_pipeline.py."
    )

phase("1. Architecture & AST Blueprint")
design_plan = await agent(
    prompt=f"""
You are the Architect & Locator Agent for uwulog-rust.
Approved feature specification (already aligned & signed off by the developer):
{approved_spec}

Instructions:
1. Run `./.agents/scripts/harness.sh tree` and `./.agents/scripts/harness.sh ast-skeleton <relevant_files>`
   to locate every struct, enum, trait, and function that needs changes across Tiers 0..4.
2. Verify dependencies strictly obey the 5-Tier 1-Way DAG (.agents/rules/GEMINI.md).
3. Produce a concise implementation blueprint listing exact file paths, line ranges,
   type signatures, and tier order of changes matching the approved spec (no scope creep).
   If you discover a critical ambiguity contradicting the approved spec, call it out explicitly
   instead of guessing. Do NOT edit files.
""",
    schema={"type": "string"},
    role="Architect & Locator",
)
log("Blueprint ready:\n", design_plan)

phase("2. Core & Engine Implementation (Tiers 0..3)")
core_summary = await agent(
    prompt=f"""
You are the Core & Engine Agent for uwulog-rust.
Approved feature specification:
{approved_spec}

Architect blueprint:
{design_plan}

Instructions:
1. Read `.agents/skills/uwu-dag-architecture/SKILL.md` and `.agents/skills/uwu-filter-arrow-dev/SKILL.md` if touching Tiers 0..2.
2. Implement ONLY the approved changes in Tiers 0..3 cleanly (zero-alloc hot paths, no runtime .unwrap(), Expr parity if touching filters, pre-release clean code without speculative shims).
3. Add unit/integration tests and run `./.agents/scripts/harness.sh check-arch` plus relevant `./.agents/scripts/harness.sh test-tier <0|1|2|3>`.
4. Return a summary of modified files and test results.
""",
    schema={"type": "string"},
    role="Core & Engine Implementer",
)
log("Core implementation:\n", core_summary)

phase("3. Presentation & CLI Integration (Tier 4)")
ui_summary = await agent(
    prompt=f"""
You are the GUI & CLI Presentation Agent for uwulog-rust.
Approved feature specification:
{approved_spec}

Architect blueprint:
{design_plan}

Core layer changes completed:
{core_summary}

Instructions:
1. Read `.agents/skills/uwu-gui-component-dev/SKILL.md` before modifying `crates/ui_gui` or `crates/core_keymap`.
2. Wire any new actions through `AppAction` / `AppEvent` / `reducer::reduce` and `theme::tokens`, following exact UX/notification decisions from the approved spec, plus headless tests using `create_test_app()`.
3. Run `./.agents/scripts/harness.sh test-tier 4`.
4. Return a summary of Tier 4 edits and test results.
""",
    schema={"type": "string"},
    role="Presentation Implementer",
)
log("Presentation integration:\n", ui_summary)

phase("4. QA & Acceptance Verification")
qa_report = await agent(
    prompt=f"""
You are the QA Gatekeeper Agent for uwulog-rust.
Verify the completed feature against the approved specification:
{approved_spec}

Instructions:
1. Run `./.agents/scripts/harness.sh verify` (checks 5-Tier DAG, Expr parity, fmt, clippy, and workspace tests).
2. Confirm every requirement and edge case in the approved spec is covered with no unintended regressions.
3. Fix or clearly report any failure. Return final pass/fail status and exact test counts.
""",
    schema={
        "type": "object",
        "properties": {
            "passed": {"type": "boolean"},
            "summary": {"type": "string"},
        },
        "required": ["passed", "summary"],
    },
    role="QA Gatekeeper",
)
log("QA Result:", qa_report)
