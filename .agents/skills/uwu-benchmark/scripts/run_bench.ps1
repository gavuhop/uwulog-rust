<#
.SYNOPSIS
    Runs performance benchmarks and compares results between the previous commit and current code.

.PARAMETER Mode
    Benchmark mode: 'compare' (default), 'quick', 'full', 'criterion', 'micro'

.PARAMETER BaselineCommit
    Git commit/ref to compare against (default: auto-detected: HEAD if working tree dirty, otherwise HEAD~1)

.PARAMETER Logs
    Number of logs to generate and profile (default: 500000 for full/compare, 100000 for quick)

.PARAMETER Queries
    Number of iterations per query scenario (default: 50 for full/compare, 20 for quick)

.PARAMETER OutputMd
    Path to save Current Benchmark Markdown report (default: 'BENCHMARK_REPORT.md')

.PARAMETER CompareMd
    Path to save Comparison Markdown report between baseline and current code (default: 'benchmark_compare_report.md')
#>
param (
    [ValidateSet("compare", "quick", "full", "criterion", "micro")]
    [string]$Mode = "compare",

    [string]$BaselineCommit = "",

    [int]$Logs = 0,

    [int]$Queries = 0,

    [string]$OutputMd = "BENCHMARK_REPORT.md",

    [string]$CompareMd = "benchmark_compare_report.md"
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " 🚀 uwulog-rust Benchmark Runner: Mode = [$Mode] " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# Ensure target directory exists
if (-not (Test-Path "target")) {
    New-Item -ItemType Directory -Path "target" | Out-Null
}

switch ($Mode) {
    "compare" {
        $logCount = if ($Logs -gt 0) { $Logs } else { 500000 }
        $queryCount = if ($Queries -gt 0) { $Queries } else { 50 }

        # Auto-detect baseline commit
        $isDirty = (git status --porcelain).Length -gt 0
        $targetBaseline = if ($BaselineCommit -ne "") {
            $BaselineCommit
        } elseif ($isDirty) {
            "HEAD"
        } else {
            "HEAD~1"
        }

        $baseHash = git rev-parse --short $targetBaseline 2>$null
        if (-not $baseHash) {
            Write-Host "Warning: Could not resolve baseline commit '$targetBaseline', falling back to regular benchmark." -ForegroundColor Yellow
            cargo run --release --bin bench_profile -- --logs $logCount --queries $queryCount --output-md $OutputMd
            break
        }

        $baseSubject = (git log -1 --format=%s $targetBaseline 2>$null)
        $baseLabel = "Commit $baseHash ($baseSubject)"
        $currentLabel = if ($isDirty) { "Current Code (Working Tree)" } else { "Commit $(git rev-parse --short HEAD)" }

        Write-Host "Performance comparison between:" -ForegroundColor Cyan
        Write-Host "   * Baseline: $baseLabel" -ForegroundColor DarkCyan
        Write-Host "   * Current:  $currentLabel" -ForegroundColor DarkCyan
        Write-Host "   * Dataset:  $logCount logs, $queryCount queries/scenario" -ForegroundColor DarkCyan

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

        $rootPath = (Get-Location).Path
        $baselineReportPath = Join-Path $rootPath "target/baseline_report.md"
        $baselineJsonPath = Join-Path $rootPath "target/baseline_snapshot.json"

        # Re-run baseline commit in temporary git worktree
        # to ensure fair real-time benchmarking parity (eliminates CPU thermal drift)
        Write-Host "Compiling and benchmarking baseline commit $baseHash in temporary git worktree..." -ForegroundColor Yellow
        $tempWorktree = Join-Path $rootPath "target/bench_worktree_$baseHash"
        if (Test-Path $tempWorktree) {
            git worktree remove --force $tempWorktree 2>$null
        }
        git worktree add --detach $tempWorktree $targetBaseline | Out-Null
        try {
            Push-Location $tempWorktree
            cargo run --release --bin bench_profile -- --logs $logCount --queries $queryCount --output-md $baselineReportPath --output-json $baselineJsonPath | Out-Null
            Pop-Location
        } finally {
            if (Test-Path $tempWorktree) {
                git worktree remove --force $tempWorktree 2>$null
            }
        }

        # Run profiler on current code and compare with baseline
        Write-Host "Benchmarking current working tree and generating reports..." -ForegroundColor Green
        $cargoArgs = @(
            "run", "--release", "--bin", "bench_profile", "--",
            "--logs", "$logCount",
            "--queries", "$queryCount",
            "--output-md", "$OutputMd",
            "--compare-md", "$CompareMd",
            "--output-json", "target/current_benchmark.json",
            "--compare", "$baselineJsonPath",
            "--baseline-label", "$baseLabel",
            "--current-label", "$currentLabel"
        )
        & cargo @cargoArgs
    }

    "quick" {
        $logCount = if ($Logs -gt 0) { $Logs } else { 100000 }
        $queryCount = if ($Queries -gt 0) { $Queries } else { 50 }
        Write-Host "Running Quick Profiler with $logCount logs, $queryCount queries/scenario..." -ForegroundColor Yellow
        cargo run --release --bin bench_profile -- --logs $logCount --queries $queryCount
    }

    "full" {
        $logCount = if ($Logs -gt 0) { $Logs } else { 500000 }
        $queryCount = if ($Queries -gt 0) { $Queries } else { 100 }
        Write-Host "Running Full Profiler with $logCount logs, $queryCount queries/scenario..." -ForegroundColor Yellow
        Write-Host "Saving report to: $OutputMd" -ForegroundColor Yellow
        cargo run --release --bin bench_profile -- --logs $logCount --queries $queryCount --output-md $OutputMd
    }

    "criterion" {
        Write-Host "Running all Criterion benchmark suites..." -ForegroundColor Yellow
        cargo bench -p uwu-benchmarks
    }

    "micro" {
        Write-Host "Running Criterion Micro-benchmark suite..." -ForegroundColor Yellow
        cargo bench -p uwu-benchmarks --bench micro_benches
    }
}

if ($LASTEXITCODE -eq 0) {
    Write-Host "============================================================" -ForegroundColor Green
    Write-Host " Benchmark finished successfully!" -ForegroundColor Green
    if ($Mode -eq "compare") {
        Write-Host " 📄 Current Benchmark Report: $OutputMd" -ForegroundColor Cyan
        Write-Host " ⚖️ Comparison Report:         $CompareMd" -ForegroundColor Cyan
    }
    Write-Host "============================================================" -ForegroundColor Green
} else {
    Write-Host "============================================================" -ForegroundColor Red
    Write-Host " Benchmark exited with code $LASTEXITCODE" -ForegroundColor Red
    Write-Host "============================================================" -ForegroundColor Red
    exit $LASTEXITCODE
}
