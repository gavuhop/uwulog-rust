#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# 🚀 uwulog-rust Benchmark Runner for Linux & macOS
# ============================================================

MODE="compare"
BASELINE_COMMIT=""
LOGS=0
QUERIES=0
OUTPUT_MD="BENCHMARK_REPORT.md"
COMPARE_MD="benchmark_compare_report.md"

print_usage() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  -m, --mode <mode>          Benchmark mode: compare (default), quick, full, criterion, micro"
    echo "  -b, --baseline <commit>    Baseline commit/ref (default: auto-detected: HEAD if dirty, else HEAD~1)"
    echo "  -l, --logs <number>        Number of logs to profile (default: 500000 for full/compare, 100000 for quick)"
    echo "  -q, --queries <number>     Number of iterations per query scenario (default: 50 for full/compare, 20 for quick)"
    echo "  -o, --output-md <path>     Current Benchmark Markdown output (default: BENCHMARK_REPORT.md)"
    echo "  -c, --compare-md <path>    Comparison Markdown output (default: benchmark_compare_report.md)"
    echo "  -h, --help                 Show this help message"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -m|--mode)
            MODE="$2"
            shift 2
            ;;
        -b|--baseline|--baseline-commit)
            BASELINE_COMMIT="$2"
            shift 2
            ;;
        -l|--logs)
            LOGS="$2"
            shift 2
            ;;
        -q|--queries)
            QUERIES="$2"
            shift 2
            ;;
        -o|--output-md)
            OUTPUT_MD="$2"
            shift 2
            ;;
        -c|--compare-md)
            COMPARE_MD="$2"
            shift 2
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

echo "============================================================"
echo " 🚀 uwulog-rust Benchmark Runner: Mode = [$MODE] "
echo "============================================================"

mkdir -p target

case "$MODE" in
    compare)
        LOG_COUNT=$(( LOGS > 0 ? LOGS : 500000 ))
        QUERY_COUNT=$(( QUERIES > 0 ? QUERIES : 50 ))

        IS_DIRTY=false
        if [[ -n "$(git status --porcelain)" ]]; then
            IS_DIRTY=true
        fi

        TARGET_BASELINE="$BASELINE_COMMIT"
        if [[ -z "$TARGET_BASELINE" ]]; then
            if [[ "$IS_DIRTY" == true ]]; then
                TARGET_BASELINE="HEAD"
            else
                TARGET_BASELINE="HEAD~1"
            fi
        fi

        BASE_HASH=$(git rev-parse --short "$TARGET_BASELINE" 2>/dev/null || true)
        if [[ -z "$BASE_HASH" ]]; then
            echo "Warning: Could not resolve baseline commit '$TARGET_BASELINE', running regular benchmark."
            cargo run --release --bin bench_profile -- --logs "$LOG_COUNT" --queries "$QUERY_COUNT" --output-md "$OUTPUT_MD"
            exit 0
        fi

        BASE_SUBJECT=$(git log -1 --format=%s "$TARGET_BASELINE" 2>/dev/null || true)
        BASE_LABEL="Commit $BASE_HASH ($BASE_SUBJECT)"
        if [[ "$IS_DIRTY" == true ]]; then
            CURRENT_LABEL="Current Code (Working Tree)"
        else
            CURRENT_LABEL="Commit $(git rev-parse --short HEAD)"
        fi

        echo "Performance comparison between:"
        echo "   * Baseline: $BASE_LABEL"
        echo "   * Current:  $CURRENT_LABEL"
        echo "   * Dataset:  $LOG_COUNT logs, $QUERY_COUNT queries/scenario"

        ROOT_PATH="$(pwd)"
        BASELINE_REPORT_PATH="$ROOT_PATH/target/baseline_report.md"
        BASELINE_JSON_PATH="$ROOT_PATH/target/baseline_snapshot.json"
        TEMP_WORKTREE="$ROOT_PATH/target/bench_worktree_$BASE_HASH"

        echo "Compiling and benchmarking baseline commit $BASE_HASH in temporary git worktree..."
        if [[ -d "$TEMP_WORKTREE" ]]; then
            git worktree remove --force "$TEMP_WORKTREE" 2>/dev/null || rm -rf "$TEMP_WORKTREE"
        fi

        git worktree add --detach "$TEMP_WORKTREE" "$TARGET_BASELINE"
        cleanup_worktree() {
            if [[ -d "$TEMP_WORKTREE" ]]; then
                git worktree remove --force "$TEMP_WORKTREE" 2>/dev/null || rm -rf "$TEMP_WORKTREE"
            fi
        }
        trap cleanup_worktree EXIT

        pushd "$TEMP_WORKTREE" > /dev/null
        cargo run --release --bin bench_profile -- \
            --logs "$LOG_COUNT" \
            --queries "$QUERY_COUNT" \
            --output-md "$BASELINE_REPORT_PATH" \
            --output-json "$BASELINE_JSON_PATH"
        popd > /dev/null

        cleanup_worktree
        trap - EXIT

        echo "Benchmarking current working tree and generating reports..."
        cargo run --release --bin bench_profile -- \
            --logs "$LOG_COUNT" \
            --queries "$QUERY_COUNT" \
            --output-md "$OUTPUT_MD" \
            --compare-md "$COMPARE_MD" \
            --output-json "target/current_benchmark.json" \
            --compare "$BASELINE_JSON_PATH" \
            --baseline-label "$BASE_LABEL" \
            --current-label "$CURRENT_LABEL"
        ;;

    quick)
        LOG_COUNT=$(( LOGS > 0 ? LOGS : 100000 ))
        QUERY_COUNT=$(( QUERIES > 0 ? QUERIES : 50 ))
        echo "Running Quick Profiler with $LOG_COUNT logs, $QUERY_COUNT queries/scenario..."
        cargo run --release --bin bench_profile -- --logs "$LOG_COUNT" --queries "$QUERY_COUNT"
        ;;

    full)
        LOG_COUNT=$(( LOGS > 0 ? LOGS : 500000 ))
        QUERY_COUNT=$(( QUERIES > 0 ? QUERIES : 100 ))
        echo "Running Full Profiler with $LOG_COUNT logs, $QUERY_COUNT queries/scenario..."
        echo "Saving report to: $OUTPUT_MD"
        cargo run --release --bin bench_profile -- --logs "$LOG_COUNT" --queries "$QUERY_COUNT" --output-md "$OUTPUT_MD"
        ;;

    criterion)
        echo "Running all Criterion benchmark suites..."
        cargo bench -p uwu-benchmarks
        ;;

    micro)
        echo "Running Criterion Micro-benchmark suite..."
        cargo bench -p uwu-benchmarks --bench micro_benches
        ;;

    *)
        echo "Invalid mode: $MODE"
        print_usage
        exit 1
        ;;
esac

echo "============================================================"
echo " Benchmark finished successfully!"
if [[ "$MODE" == "compare" ]]; then
    echo " 📄 Current Benchmark Report: $OUTPUT_MD"
    echo " ⚖️ Comparison Report:         $COMPARE_MD"
fi
echo "============================================================"
