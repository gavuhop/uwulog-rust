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
    Path to save Markdown report (default: 'BENCHMARK_REPORT.md')
#>
param (
    [ValidateSet("compare", "quick", "full", "criterion", "micro")]
    [string]$Mode = "compare",

    [string]$BaselineCommit = "",

    [int]$Logs = 0,

    [int]$Queries = 0,

    [string]$OutputMd = "BENCHMARK_REPORT.md"
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
        $currentLabel = if ($isDirty) { "Code Hien Tai (Working Tree)" } else { "Commit $(git rev-parse --short HEAD)" }

        Write-Host "So sanh hieu nang giua:" -ForegroundColor Cyan
        Write-Host "   * Baseline: $baseLabel" -ForegroundColor DarkCyan
        Write-Host "   * Current:  $currentLabel" -ForegroundColor DarkCyan
        Write-Host "   * Quy mo:   $logCount logs, $queryCount queries/scenario" -ForegroundColor DarkCyan

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

        $rootPath = (Get-Location).Path
        $baselineReportPath = Join-Path $rootPath "target/baseline_report.md"
        $baseReportContent = git show "${targetBaseline}:BENCHMARK_REPORT.md" 2>$null

        if ($baseReportContent -and ($baseReportContent | Select-String "RAM RSS Footprint")) {
            Write-Host "Da tim thay benchmark report trong commit $baseHash, su dung lam baseline." -ForegroundColor Green
            $baseReportContent | Set-Content -Path $baselineReportPath -Encoding utf8
        } else {
            Write-Host "Chua co benchmark report trong commit $baseHash. Dang chay profiler trong git worktree..." -ForegroundColor Yellow
            $tempWorktree = "target/bench_worktree_$baseHash"
            if (Test-Path $tempWorktree) {
                git worktree remove --force $tempWorktree 2>$null
            }
            git worktree add --detach $tempWorktree $targetBaseline | Out-Null
            try {
                Push-Location $tempWorktree
                cargo run --release --bin bench_profile -- --logs $logCount --queries $queryCount --output-md $baselineReportPath | Out-Null
                Pop-Location
            } finally {
                if (Test-Path $tempWorktree) {
                    git worktree remove --force $tempWorktree 2>$null
                }
            }
        }

        # Chay profiler cho code hien tai va so sanh voi baseline
        Write-Host "Dang chay benchmark code hien tai va doi chieu voi baseline..." -ForegroundColor Green
        $cargoArgs = @(
            "run", "--release", "--bin", "bench_profile", "--",
            "--logs", "$logCount",
            "--queries", "$queryCount",
            "--output-md", "$OutputMd",
            "--output-json", "target/current_benchmark.json",
            "--compare", "$baselineReportPath",
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
    Write-Host "============================================================" -ForegroundColor Green
} else {
    Write-Host "============================================================" -ForegroundColor Red
    Write-Host " Benchmark exited with code $LASTEXITCODE" -ForegroundColor Red
    Write-Host "============================================================" -ForegroundColor Red
    exit $LASTEXITCODE
}
