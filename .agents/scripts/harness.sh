#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# uwulog-rust Agent Harness CLI (Linux / macOS)
# Unified token-efficient exploration, architecture/convention guard & test gate
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

print_usage() {
    cat <<'EOF'
Usage: ./.agents/scripts/harness.sh <command> [args...]

Token-Optimized Exploration (AST & Tree):
  tree [target] [--tier N] [-p crate] [--json]   Annotated 5-Tier directory & module tree
  ast-skeleton <file_or_crate> [--tests] [--json] Extract signatures + [Lstart-Lend], omit fn bodies
  ast-symbol <name> [-p crate] [--fuzzy] [--json] Cross-crate symbol definitions & call sites
  ast-hotspots [target] [--top N] [--json]        Longest fns, locks, par_iter, unsafe sites
  fields [filter] [--json]                        SSOT StandardField & Arrow '__' system columns
  keymap [query] [-c context] [--json]            KeyAction, KeyContext & default keybindings
  affected [--downstream] [--base REF] [--json]   Detect crates affected by current git changes

Architecture, Convention & Quality Gates:
  check-arch [--dag-only|--conv-only|--strict|--json]
                                                  Verify 5-Tier DAG, Expr parity & conventions
  fmt                                             Check Rust formatting (cargo fmt --all --check)
  clippy                                          Run workspace clippy with -D warnings
  test-tier <0|1|2|3|4|all>                       Run tests for a specific DAG tier or all
  test-changed [--downstream] [--base REF]        Run targeted tests for git-modified crates only
  self-test                                       Run Python stdlib self-tests for .agents/ harness
  bench-quick                                     Run quick performance profiler
  verify                                          Full gate: check-arch + fmt + clippy + tests
EOF
}

run_test_tier() {
    local tier="${1:-all}"
    case "$tier" in
        0)
            echo "[harness] Testing Tier 0 (Foundations & Protocols)..."
            cargo test -p uwu-core-util -p uwu-core-schema -p uwu-core-protocol -p uwu-core-filter
            cargo test -p uwu-icons --all-features
            ;;
        1)
            echo "[harness] Testing Tier 1 (Drivers & OS Transport)..."
            cargo test -p uwu-driver-transport -p uwu-driver-sources
            ;;
        2)
            echo "[harness] Testing Tier 2 (Core Data Engine)..."
            cargo test -p uwu-core-engine
            ;;
        3)
            echo "[harness] Testing Tier 3 (Workspace & Keymap)..."
            cargo test -p uwu-core-workspace --features uwu-icons/egui
            cargo test -p uwu-core-keymap
            ;;
        4)
            echo "[harness] Testing Tier 4 (Presentation & Binaries)..."
            cargo test -p uwu-ui-gui -p uwu-cli-agent -p uwu-cli
            cargo check -p uwu-benchmarks --benches --bins
            ;;
        all)
            echo "[harness] Testing full workspace..."
            cargo test --workspace
            ;;
        *)
            echo "Invalid tier '$tier'. Expected: 0, 1, 2, 3, 4, or all." >&2
            return 1
            ;;
    esac
}

run_test_changed() {
    local affected_json
    affected_json="$(python3 "$SCRIPT_DIR/ast_index.py" affected --json "$@")"

    local test_all
    test_all="$(python3 -c 'import json, sys; d = json.loads(sys.stdin.read()); print("1" if d.get("test_all") else "0")' <<< "$affected_json")"
    if [[ "$test_all" == "1" ]]; then
        echo "[harness] Root Cargo.toml/Cargo.lock changed — running full workspace tests..."
        run_test_tier "all"
        return 0
    fi

    local crates_str
    crates_str="$(python3 -c 'import json, sys; d = json.loads(sys.stdin.read()); print(" ".join(d.get("affected_crates", [])))' <<< "$affected_json")"
    if [[ -z "$crates_str" ]]; then
        echo "[harness] No Rust crates affected by current git changes."
        return 0
    fi

    echo "[harness] Testing affected crates: $crates_str"
    local std_crates=()
    local run_icons=0
    local run_workspace=0
    local run_bench=0

    for c in $crates_str; do
        case "$c" in
            uwu-icons)
                run_icons=1
                ;;
            uwu-core-workspace)
                run_workspace=1
                ;;
            uwu-benchmarks)
                run_bench=1
                ;;
            *)
                std_crates+=("-p" "$c")
                ;;
        esac
    done

    if [[ ${#std_crates[@]} -gt 0 ]]; then
        cargo test "${std_crates[@]}"
    fi
    if [[ "$run_icons" == "1" ]]; then
        cargo test -p uwu-icons --all-features
    fi
    if [[ "$run_workspace" == "1" ]]; then
        cargo test -p uwu-core-workspace --features uwu-icons/egui
    fi
    if [[ "$run_bench" == "1" ]]; then
        cargo check -p uwu-benchmarks --benches --bins
    fi
}

CMD="${1:-help}"
if [[ $# -gt 0 ]]; then
    shift
fi

case "$CMD" in
    tree)
        python3 "$SCRIPT_DIR/ast_index.py" tree "$@"
        ;;
    ast-skeleton|skeleton)
        python3 "$SCRIPT_DIR/ast_index.py" skeleton "$@"
        ;;
    ast-symbol|symbol)
        python3 "$SCRIPT_DIR/ast_index.py" symbol "$@"
        ;;
    ast-hotspots|hotspots)
        python3 "$SCRIPT_DIR/ast_index.py" hotspots "$@"
        ;;
    fields|standard-fields)
        python3 "$SCRIPT_DIR/ast_index.py" fields "$@"
        ;;
    keymap)
        python3 "$SCRIPT_DIR/ast_index.py" keymap "$@"
        ;;
    affected)
        python3 "$SCRIPT_DIR/ast_index.py" affected "$@"
        ;;
    check-arch|check-dag)
        python3 "$SCRIPT_DIR/verify_arch_and_conv.py" "$@"
        ;;
    fmt)
        echo "[harness] Checking cargo fmt..."
        cargo fmt --all -- --check
        ;;
    clippy)
        echo "[harness] Running cargo clippy..."
        cargo clippy --workspace --all-targets -- -D warnings
        ;;
    test-tier)
        run_test_tier "${1:-all}"
        ;;
    test-changed)
        run_test_changed "$@"
        ;;
    self-test)
        echo "[harness] Running .agents/ harness self-test suite..."
        python3 -m unittest discover -s "$SCRIPT_DIR" -p "test_*.py" -v
        ;;
    bench-quick)
        bash "$REPO_ROOT/.agents/skills/uwu-benchmark/scripts/run_bench.sh" -m quick "$@"
        ;;
    verify)
        echo "============================================================"
        echo " uwulog-rust Harness Full Verification"
        echo "============================================================"
        python3 "$SCRIPT_DIR/verify_arch_and_conv.py"
        echo "[harness] Checking cargo fmt..."
        cargo fmt --all -- --check
        echo "[harness] Running cargo clippy..."
        cargo clippy --workspace --all-targets -- -D warnings
        run_test_tier "all"
        echo "============================================================"
        echo " Harness Verification PASSED"
        echo "============================================================"
        ;;
    -h|--help|help)
        print_usage
        ;;
    *)
        echo "Unknown command: $CMD" >&2
        print_usage
        exit 1
        ;;
esac
