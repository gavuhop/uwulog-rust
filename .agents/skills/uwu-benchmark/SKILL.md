---
name: uwu-benchmark
description: >-
  Execute performance benchmarks, CPU/RAM profiling, and generate aggregated benchmark reports for the uwulog-rust project.
  Activate this skill when requested to run benchmarks, measure log throughput, profile query latency percentiles, test Rayon multi-threading scalability, or generate performance reports.
---

# 🚀 uwulog-rust Benchmark & Performance Profiler Skill

This skill provides standard operating procedures for executing benchmark suites, measuring real-time performance metrics (CPU, RAM RSS, Throughput, Latency percentiles), and compiling comprehensive comparison reports for the `uwulog-rust` project.

---

## 1. Available Benchmark Suites

The `uwulog-rust` project provides two complementary benchmarking systems:

1. **Live End-to-End Profiler CLI ([`bench_profile`](file:///home/truongviet/projects/uwulog-rust/crates/benchmarks/src/bin/bench_profile.rs))**:
   * Fast, interactive, and measures 6 key dimensions:
     1. Ingestion throughput into the in-memory engine and RAM RSS footprint per log entry.
     2. Latency percentile matrix (Cold Run, Warm p50, Warm p95, Max, QPS) across [7 Standard Query Scenarios](./references/query_scenarios.md).
     3. Rayon multi-core scaling factor (Speedup $S_N$ & Efficiency $E_N$ from 1 to 12 cores).
     4. Concurrent stress tolerance (Lock contention while background thread ingests 10k - 50k logs/sec).
     5. Desktop UI frame budget validation (60 FPS / < 16.6ms target).
     6. End-to-end file ingestion speed from disk (`MB/s` and `logs/s`).
   * Automatically exports two standardized Markdown reports:
     * [`BENCHMARK_REPORT.md`](file:///home/truongviet/projects/uwulog-rust/BENCHMARK_REPORT.md): Comprehensive benchmark results for the **current working tree**.
     * [`benchmark_compare_report.md`](file:///home/truongviet/projects/uwulog-rust/benchmark_compare_report.md): In-depth **comparison report** between baseline commit and current code.

2. **Criterion Micro & Integration Benches ([`crates/benchmarks/benches/`](file:///home/truongviet/projects/uwulog-rust/crates/benchmarks/benches))**:
   * Statistical micro-benchmarks with nanosecond precision:
     * `micro_benches`: Tokenizer, AST Parser, Evaluator, Normalizer (JSON & Plain Text).
     * `engine_search_benches`: Search queries across 10k, 500k, and 5M logs.
     * `cpu_scaling_benches`: Evaluates 1..12 Rayon thread scaling.
     * `concurrent_stress_benches`: Concurrent stream background writes + query reads.
     * `file_ingestion_benches`: Reading 10k and 50k log lines from disk.
     * `ui_simulation_benches`: Render tick simulation for 100, 500, 2000, and 10000 logs/frame.

---

## 2. Standard Execution Commands

### A. Baseline Comparison Mode (Recommended / Default)
Automatically provisions a temporary Git worktree to compile and benchmark the baseline commit (`HEAD~1` if clean, or `HEAD` if dirty) in real time under identical thermal/load conditions, then benchmarks current code and generates a side-by-side comparison:
```bash
# On Linux / macOS:
bash .agents/skills/uwu-benchmark/scripts/run_bench.sh -m compare

# On Windows PowerShell:
powershell -ExecutionPolicy Bypass -File .agents/skills/uwu-benchmark/scripts/run_bench.ps1 -Mode compare
```
*Optional: Compare against an explicit baseline commit:*
```bash
bash .agents/skills/uwu-benchmark/scripts/run_bench.sh -m compare -b "HEAD~2" -l 500000 -q 50
```

*Or invoke `bench_profile` directly with comparison flags:*
```bash
cargo run --release --bin bench_profile -- --logs 500000 --queries 50 --output-md BENCHMARK_REPORT.md --compare-md benchmark_compare_report.md --compare target/baseline_snapshot.json --baseline-label "Baseline Commit" --current-label "Current Code"
```

### B. Quick Profile Mode
Ideal for quick sanity checks after small refactors:
```bash
# Linux / macOS:
bash .agents/skills/uwu-benchmark/scripts/run_bench.sh -m quick

# Or direct cargo command:
cargo run --release --bin bench_profile -- --logs 100000 --queries 50
```

### C. Standalone Full Profile Mode (Single Run without Comparison)
```bash
# Linux / macOS:
bash .agents/skills/uwu-benchmark/scripts/run_bench.sh -m full

# Or direct cargo command:
cargo run --release --bin bench_profile -- --logs 500000 --queries 100 --output-md BENCHMARK_REPORT.md
```

### D. Run Criterion Micro-Benchmarks
```bash
# Run micro benchmarks only:
cargo bench -p uwu-benchmarks --bench micro_benches

# Run entire Criterion suite:
cargo bench -p uwu-benchmarks
```

---

## 3. Analysis & Reporting Workflow

When a benchmark is requested, **always prioritize Comparison Mode** to clearly demonstrate the performance impact of recent code modifications:

### Step 1: Run Comparison Benchmark
Execute the runner script:
```bash
bash .agents/skills/uwu-benchmark/scripts/run_bench.sh -m compare -l 500000 -q 50
```
The script will automatically:
1. Create an isolated Git worktree for the baseline commit and measure it live (ensuring fair CPU thermal and background load parity).
2. Run benchmarks on the current working tree.
3. Compute percentage deltas ($\Delta\%$) and status indicators (🚀 Faster, 📉 Lower RAM, ⚠️ Slower, ➖ Parity).
4. Export current results to [`BENCHMARK_REPORT.md`](file:///home/truongviet/projects/uwulog-rust/BENCHMARK_REPORT.md).
5. Export detailed comparison breakdown to [`benchmark_compare_report.md`](file:///home/truongviet/projects/uwulog-rust/benchmark_compare_report.md).

### Step 2: Delta Analysis
Evaluate improvements or regressions against standard thresholds:
* **Ingestion Throughput (Logs/s)**: $\Delta > +3\%$ represents a notable speedup (🚀).
* **RAM Footprint / log (bytes)**: Any reduction $\Delta < 0\%$ indicates positive memory optimization (📉).
* **p50 Latency per Query Scenario**:
  * Reduction $\Delta < -3\%$: 🚀 Faster query response.
  * Increase $\Delta > +5\%$: ⚠️ Performance regression alert; investigate root causes (e.g. extra heap allocations, unneeded clones, lock contention).
* **Rayon Multi-Threading & Lock Contention**: Ensure speedup $\ge 4.0\times$ and lock contention overhead $\le 15\%$.

### Step 3: Summary Report Structure
When responding to the user, structure the findings as follows:
1. **Context & Environment**: Baseline commit vs. Current code, hardware specifications, and log dataset scale.
2. **Core Metrics Table**: Ingestion rate, RAM/log, total RAM RSS with Delta $\Delta\%$ and status tags.
3. **Query Scenario p50 Comparison**: Baseline p50 vs. Current p50, percentage delta, and query throughput.
4. **Root Cause Analysis & Key Insights**: Technical explanation of why metrics changed (e.g. replacing `Uuid` with `u64` reduced RAM by 8.7 B/log and boosted ingestion by 7.9%).

---

## 4. References

* Current performance report: [`BENCHMARK_REPORT.md`](file:///home/truongviet/projects/uwulog-rust/BENCHMARK_REPORT.md)
* Comparison report: [`benchmark_compare_report.md`](file:///home/truongviet/projects/uwulog-rust/benchmark_compare_report.md)
* Query scenarios definition: [references/query_scenarios.md](./references/query_scenarios.md)
* Shell runner: [scripts/run_bench.sh](./scripts/run_bench.sh)
* PowerShell runner: [scripts/run_bench.ps1](./scripts/run_bench.ps1)
