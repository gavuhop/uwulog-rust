//! Live CPU, RAM, Latency, Real-World Concurrency & Throughput Profiler CLI for uwulog-rust

use clap::Parser;
use rayon::ThreadPoolBuilder;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Pid, RefreshKind, System};
use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::sleep;
use uwu_benchmarks::{SyntheticLogGenerator, BENCHMARK_QUERIES};
use uwu_core_engine::SystemEngine;
use uwu_core_schema::{RawLogEntry, RawPayload};
use uwu_driver_sources::LogNormalizer;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Comprehensive Performance, CPU & Real-World Concurrency Profiler for uwulog-rust",
    long_about = None
)]
struct Args {
    /// Số lượng log cần nạp vào Engine để đo kiểm
    #[arg(short, long, default_value_t = 500_000)]
    logs: usize,

    /// Số vòng lặp thực thi mỗi query để đo phân vị độ trễ (p50/p95/p99)
    #[arg(short, long, default_value_t = 100)]
    queries: usize,

    /// Đường dẫn xuất báo cáo Markdown
    #[arg(short, long)]
    output_md: Option<PathBuf>,
}

#[allow(dead_code)]
struct LatencyStats {
    min_ms: f64,
    mean_ms: f64,
    p50_ms: f64,
    p90_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    max_ms: f64,
    qps: f64,
    throughput_m_logs_sec: f64,
}

impl LatencyStats {
    fn compute(mut samples: Vec<f64>, total_logs: usize) -> Self {
        if samples.is_empty() {
            return Self {
                min_ms: 0.0,
                mean_ms: 0.0,
                p50_ms: 0.0,
                p90_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                max_ms: 0.0,
                qps: 0.0,
                throughput_m_logs_sec: 0.0,
            };
        }

        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = samples.len();
        let sum: f64 = samples.iter().sum();
        let mean = sum / (count as f64);
        let min = samples[0];
        let max = samples[count - 1];

        let p50 = samples[(count as f64 * 0.50).min((count - 1) as f64) as usize];
        let p90 = samples[(count as f64 * 0.90).min((count - 1) as f64) as usize];
        let p95 = samples[(count as f64 * 0.95).min((count - 1) as f64) as usize];
        let p99 = samples[(count as f64 * 0.99).min((count - 1) as f64) as usize];

        let qps = if mean > 0.0 { 1000.0 / mean } else { 0.0 };
        let throughput_m_logs_sec = if mean > 0.0 {
            (total_logs as f64 / 1_000_000.0) / (mean / 1000.0)
        } else {
            0.0
        };

        Self {
            min_ms: min,
            mean_ms: mean,
            p50_ms: p50,
            p90_ms: p90,
            p95_ms: p95,
            p99_ms: p99,
            max_ms: max,
            qps,
            throughput_m_logs_sec,
        }
    }
}

struct ScalingResult {
    threads: usize,
    duration_ms: f64,
    speedup: f64,
    efficiency: f64,
}

struct UiTickResult {
    batch_size: usize,
    tick_time_ms: f64,
    fps_equivalent: f64,
    within_60fps_budget: bool,
}

struct FileIoResult {
    line_count: usize,
    file_size_mb: f64,
    duration_ms: f64,
    throughput_mb_s: f64,
    throughput_logs_s: f64,
}

struct ConcurrencyResult {
    stream_rate: usize,
    idle_search_ms: f64,
    contention_search_ms: f64,
    overhead_pct: f64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("================================================================================");
    println!("        🚀 UWULOG-RUST END-TO-END PERFORMANCE & PROFILER SUITE 🚀             ");
    println!("================================================================================");

    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    sys.refresh_all();

    let pid = Pid::from_u32(std::process::id());
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let logical_cores = sys.cpus().len();
    let total_ram_gb = sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

    println!("💻 System Configuration:");
    println!("   • CPU Model:      {}", cpu_brand);
    println!("   • Logical Cores:  {}", logical_cores);
    println!("   • Total RAM:      {:.2} GB", total_ram_gb);
    println!(
        "   • Target Logs:    {} ({:.1} M entries)",
        args.logs,
        args.logs as f64 / 1_000_000.0
    );
    println!("   • Iterations/Qry: {}", args.queries);
    println!("--------------------------------------------------------------------------------");

    // 1. Sinh dữ liệu và nạp vào Engine
    println!(
        "⏳ [1/6] In-Memory Preloading: Generating & Ingesting {} Logs...",
        args.logs
    );
    let gen_start = Instant::now();
    let mut gen = SyntheticLogGenerator::new(2026);
    let events = gen.generate_events(args.logs);
    let gen_duration = gen_start.elapsed();

    let engine = Arc::new(SystemEngine::new(args.logs));
    let ingest_start = Instant::now();
    engine.push_events(events);
    let ingest_duration = ingest_start.elapsed();

    sys.refresh_processes();
    let mem_used_bytes = sys.process(pid).map(|p| p.memory()).unwrap_or(0);
    let mem_used_mb = mem_used_bytes as f64 / (1024.0 * 1024.0);
    let ingest_rate = args.logs as f64 / ingest_duration.as_secs_f64();

    println!(
        "   ✓ Generated in {:.2?} | Ingested in {:.2?} ({:.0} logs/sec)",
        gen_duration, ingest_duration, ingest_rate
    );
    println!(
        "   ✓ In-Memory RAM RSS Footprint: {:.2} MB ({:.2} bytes/log)",
        mem_used_mb,
        if args.logs > 0 {
            mem_used_bytes as f64 / args.logs as f64
        } else {
            0.0
        }
    );
    println!("--------------------------------------------------------------------------------");

    // 2. Thực thi 7 kịch bản truy vấn tĩnh
    println!("⏳ [2/6] Running 7 Query Scenarios & Measuring Latency / Throughput...");
    let mut query_results = Vec::new();
    let mut peak_cpu_observed = 0.0f32;
    let mut cpu_samples = Vec::new();

    for bq in BENCHMARK_QUERIES {
        let mut latencies = Vec::with_capacity(args.queries);

        for _ in 0..5 {
            let _ = engine.search_limited(bq.query, 1000);
        }

        let query_start = Instant::now();
        for _ in 0..args.queries {
            let iter_start = Instant::now();
            let _ = engine.search_limited(bq.query, 1000);
            let elapsed_ms = iter_start.elapsed().as_secs_f64() * 1000.0;
            latencies.push(elapsed_ms);
        }
        let total_query_time = query_start.elapsed();

        sys.refresh_processes();
        sys.refresh_all();
        let cur_proc_cpu = sys.process(pid).map(|p| p.cpu_usage()).unwrap_or(0.0);
        let cur_global_cpu = sys.global_cpu_info().cpu_usage();

        if cur_proc_cpu > peak_cpu_observed {
            peak_cpu_observed = cur_proc_cpu;
        }
        cpu_samples.push(cur_proc_cpu);

        let stats = LatencyStats::compute(latencies, args.logs);
        query_results.push((bq, stats, total_query_time, cur_proc_cpu, cur_global_cpu));
    }
    println!("   ✓ Completed in-memory query profiling.");
    println!("--------------------------------------------------------------------------------");

    // 3. Rayon Multi-Core Scaling
    println!("⏳ [3/6] Measuring Rayon Multi-Core Scaling Efficiency...");
    let complex_query =
        "(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat";

    let mut thread_configs = vec![1, 2, 4];
    if logical_cores >= 8 {
        thread_configs.push(8);
    }
    if !thread_configs.contains(&logical_cores) {
        thread_configs.push(logical_cores);
    }
    thread_configs.sort_unstable();

    let mut scaling_results = Vec::new();
    let mut baseline_ms = 0.0;

    for &num_threads in &thread_configs {
        let pool = ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .expect("Failed to build rayon thread pool");

        for _ in 0..3 {
            pool.install(|| {
                let _ = engine.search_limited(complex_query, 1000);
            });
        }

        let iterations = 20;
        let start = Instant::now();
        for _ in 0..iterations {
            pool.install(|| {
                let _ = engine.search_limited(complex_query, 1000);
            });
        }
        let avg_duration_ms = (start.elapsed().as_secs_f64() * 1000.0) / (iterations as f64);

        if num_threads == 1 {
            baseline_ms = avg_duration_ms;
        }

        let speedup = if avg_duration_ms > 0.0 {
            baseline_ms / avg_duration_ms
        } else {
            1.0
        };
        let efficiency = (speedup / (num_threads as f64)) * 100.0;

        scaling_results.push(ScalingResult {
            threads: num_threads,
            duration_ms: avg_duration_ms,
            speedup,
            efficiency,
        });
    }
    println!("   ✓ Completed multi-core scaling analysis.");
    println!("--------------------------------------------------------------------------------");

    // 4. Real-World Concurrency: Vừa Stream Log Nền Vừa Search
    println!("⏳ [4/6] Real-World Concurrency: Stress Testing Under Live Background Log Stream...");
    let mut concurrency_results = Vec::new();
    let stream_rates = [10_000, 50_000];

    for &rate in &stream_rates {
        let idle_search_ms = scaling_results
            .iter()
            .find(|r| r.threads == logical_cores)
            .map(|r| r.duration_ms)
            .unwrap_or(10.0);

        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = Arc::clone(&is_running);
        let tx = engine.get_channel();

        let producer_handle = tokio::spawn(async move {
            let mut p_gen = SyntheticLogGenerator::new(777);
            let batch_size = (rate / 100).max(10);
            let interval = Duration::from_millis(10);

            while is_running_clone.load(Ordering::Relaxed) {
                let entries = p_gen.generate_raw_json_entries(batch_size);
                for e in entries {
                    if tx.send(e).await.is_err() {
                        break;
                    }
                }
                sleep(interval).await;
            }
        });

        // Chạy 30 query trong lúc stream đang bắn
        let iters = 30;
        let c_start = Instant::now();
        for _ in 0..iters {
            let _ = engine.search_limited(complex_query, 1000);
        }
        let contention_search_ms = (c_start.elapsed().as_secs_f64() * 1000.0) / (iters as f64);

        is_running.store(false, Ordering::Relaxed);
        let _ = producer_handle.await;

        let overhead_pct = ((contention_search_ms - idle_search_ms) / idle_search_ms) * 100.0;
        concurrency_results.push(ConcurrencyResult {
            stream_rate: rate,
            idle_search_ms,
            contention_search_ms,
            overhead_pct,
        });
    }
    println!("   ✓ Completed concurrent streaming stress test.");
    println!("--------------------------------------------------------------------------------");

    // 5. UI Frame Budget Simulation (< 16.6ms for 60 FPS)
    println!("⏳ [5/6] Simulating UI Frame Budget (60 FPS / 16.6ms Target)...");
    let mut ui_results = Vec::new();
    let batch_sizes = [100, 500, 2_000, 5_000];

    for &b_size in &batch_sizes {
        let mut last_proc = engine.total_processed();
        let mut p_gen = SyntheticLogGenerator::new(888);
        let iters = 20;

        let start = Instant::now();
        for _ in 0..iters {
            let new_events = p_gen.generate_events(b_size);
            engine.push_events(new_events);

            let (_matched, logs) = engine.filter_incremental(complex_query, last_proc);
            let skip_count = logs.len().saturating_sub(60);
            let _visible: Vec<_> = logs.into_iter().skip(skip_count).collect();
            let _schema = engine.get_schema_map();
            last_proc = engine.total_processed();
        }
        let avg_tick_ms = (start.elapsed().as_secs_f64() * 1000.0) / (iters as f64);
        let fps = if avg_tick_ms > 0.0 {
            1000.0 / avg_tick_ms
        } else {
            0.0
        };

        ui_results.push(UiTickResult {
            batch_size: b_size,
            tick_time_ms: avg_tick_ms,
            fps_equivalent: fps,
            within_60fps_budget: avg_tick_ms <= 16.66,
        });
    }
    println!("   ✓ Completed UI frame budget simulation.");
    println!("--------------------------------------------------------------------------------");

    // 6. End-to-End File I/O Ingestion Rate
    println!("⏳ [6/6] End-to-End File I/O Ingestion Benchmark...");
    let file_line_counts = [50_000, 100_000];
    let mut file_results = Vec::new();

    for &f_count in &file_line_counts {
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp log file");
        let mut f_bytes = 0u64;

        for i in 0..f_count {
            let json = serde_json::json!({
                "timestamp": "2026-09-03T12:00:00Z",
                "level": if i % 10 == 0 { "ERROR" } else { "INFO" },
                "message": format!("Application worker processed transaction #{}", i),
                "tag": "app.backend",
                "latency": 45 + (i % 300)
            });
            let s = json.to_string();
            writeln!(temp_file, "{}", s).unwrap();
            f_bytes += s.len() as u64 + 1;
        }
        temp_file.flush().unwrap();
        let file_path = temp_file.path().to_path_buf();
        let file_size_mb = f_bytes as f64 / (1024.0 * 1024.0);

        let f_engine = Arc::new(SystemEngine::new(f_count + 1000));
        let read_start = Instant::now();

        let file = File::open(&file_path).await.unwrap();
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut batch = Vec::with_capacity(512);

        while reader.read_line(&mut line).await.unwrap() > 0 {
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                let entry = RawLogEntry {
                    payload: RawPayload::Text(trimmed.to_string()),
                };
                let event = LogNormalizer::normalize(entry);
                batch.push(event);

                if batch.len() >= 512 {
                    f_engine.push_events(std::mem::take(&mut batch));
                }
            }
            line.clear();
        }
        if !batch.is_empty() {
            f_engine.push_events(batch);
        }

        let duration_ms = read_start.elapsed().as_secs_f64() * 1000.0;
        let throughput_mb_s = file_size_mb / (duration_ms / 1000.0);
        let throughput_logs_s = f_count as f64 / (duration_ms / 1000.0);

        file_results.push(FileIoResult {
            line_count: f_count,
            file_size_mb,
            duration_ms,
            throughput_mb_s,
            throughput_logs_s,
        });
    }
    println!("   ✓ Completed file I/O ingestion benchmark.");
    println!("================================================================================");
    println!("                             📊 PERFORMANCE SUMMARY                             ");
    println!("================================================================================");

    // 1. In-Memory Search Latency
    println!("\n📌 [1. In-Memory Query Latency & Throughput Matrix]");
    println!(
        "{:<24} | {:>8} | {:>8} | {:>8} | {:>8} | {:>10} | {:>14}",
        "Query Scenario", "p50 (ms)", "p95 (ms)", "p99 (ms)", "Max (ms)", "QPS", "Throughput"
    );
    println!(
        "{:-<24}-|-{:-<8}-|-{:-<8}-|-{:-<8}-|-{:-<8}-|-{:-<10}-|-{:-<14}",
        "", "", "", "", "", "", ""
    );

    for (bq, stats, _, _, _) in &query_results {
        println!(
            "{:<24} | {:>8.2} | {:>8.2} | {:>8.2} | {:>8.2} | {:>10.1} | {:>10.2} M/s",
            bq.name,
            stats.p50_ms,
            stats.p95_ms,
            stats.p99_ms,
            stats.max_ms,
            stats.qps,
            stats.throughput_m_logs_sec
        );
    }

    // 2. Multi-Core Scaling
    println!("\n📌 [2. Rayon Multi-Core Scaling & Parallel Efficiency]");
    println!(
        "{:<12} | {:>14} | {:>10} | {:>12}",
        "Cores / Threads", "Latency (ms)", "Speedup", "Efficiency"
    );
    println!("{:-<12}-|-{:-<14}-|-{:-<10}-|-{:-<12}", "", "", "", "");

    for res in &scaling_results {
        let tag = if res.threads == logical_cores {
            " (Max)"
        } else {
            ""
        };
        println!(
            "{:<12} | {:>14.2} | {:>9.2}x | {:>11.1}%",
            format!("{}{}", res.threads, tag),
            res.duration_ms,
            res.speedup,
            res.efficiency
        );
    }

    // 3. Real-World Concurrency
    println!("\n📌 [3. Real-World Concurrency: Streaming Stress vs Query Latency]");
    println!(
        "{:<22} | {:>16} | {:>18} | {:>14}",
        "Background Stream Rate", "Idle Search (ms)", "Contention Search", "Lock Overhead"
    );
    println!("{:-<22}-|-{:-<16}-|-{:-<18}-|-{:-<14}", "", "", "", "");

    for c in &concurrency_results {
        println!(
            "{:<22} | {:>16.2} | {:>18.2} | {:>13.1}%",
            format!("{} logs/s", c.stream_rate),
            c.idle_search_ms,
            c.contention_search_ms,
            c.overhead_pct
        );
    }

    // 4. UI Frame Budget
    println!("\n📌 [4. UI Frame Budget Simulation (Target < 16.6ms for 60 FPS)]");
    println!(
        "{:<20} | {:>14} | {:>14} | {:>14}",
        "Incoming Logs/Frame", "Frame Time (ms)", "Equiv. FPS", "60 FPS Status"
    );
    println!("{:-<20}-|-{:-<14}-|-{:-<14}-|-{:-<14}", "", "", "", "");

    for u in &ui_results {
        let status = if u.within_60fps_budget {
            "✅ Pass (>=60 FPS)"
        } else {
            "⚠️ Drop Frame"
        };
        println!(
            "{:<20} | {:>14.2} | {:>14.1} | {:>14}",
            format!("{} logs/frame", u.batch_size),
            u.tick_time_ms,
            u.fps_equivalent,
            status
        );
    }

    // 5. File I/O
    println!("\n📌 [5. End-to-End File Ingestion Performance (Disk -> Parse -> Engine)]");
    println!(
        "{:<16} | {:>14} | {:>14} | {:>16} | {:>16}",
        "Log Lines", "File Size", "Read Time (ms)", "Throughput (MB/s)", "Throughput (Logs/s)"
    );
    println!(
        "{:-<16}-|-{:-<14}-|-{:-<14}-|-{:-<16}-|-{:-<16}",
        "", "", "", "", ""
    );

    for f in &file_results {
        println!(
            "{:<16} | {:>12.2} MB | {:>14.2} | {:>14.2} MB/s | {:>14.0} /s",
            format!("{} lines", f.line_count),
            f.file_size_mb,
            f.duration_ms,
            f.throughput_mb_s,
            f.throughput_logs_s
        );
    }

    // 6. Resource stats
    let avg_cpu: f32 = if !cpu_samples.is_empty() {
        cpu_samples.iter().sum::<f32>() / (cpu_samples.len() as f32)
    } else {
        0.0
    };

    println!("\n📌 [6. Overall Resource Footprint]");
    println!("   • Peak Process CPU:     {:.1}%", peak_cpu_observed);
    println!("   • Average Query CPU:    {:.1}%", avg_cpu);
    println!("   • In-Memory RAM RSS:    {:.2} MB", mem_used_mb);
    println!("================================================================================");

    // Xuất báo cáo Markdown
    if let Some(ref md_path) = args.output_md {
        let md_content = generate_markdown_report(
            &cpu_brand,
            logical_cores,
            total_ram_gb,
            args.logs,
            ingest_rate,
            mem_used_mb,
            peak_cpu_observed,
            avg_cpu,
            &query_results,
            &scaling_results,
            &concurrency_results,
            &ui_results,
            &file_results,
        );

        if let Err(e) = fs::write(md_path, md_content) {
            eprintln!("❌ Failed to write Markdown report to {:?}: {}", md_path, e);
        } else {
            println!("💾 Markdown report successfully saved to: {:?}", md_path);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_markdown_report(
    cpu_brand: &str,
    logical_cores: usize,
    total_ram_gb: f64,
    logs: usize,
    ingest_rate: f64,
    mem_used_mb: f64,
    peak_cpu: f32,
    avg_cpu: f32,
    query_results: &[(
        &uwu_benchmarks::BenchmarkQuery,
        LatencyStats,
        Duration,
        f32,
        f32,
    )],
    scaling_results: &[ScalingResult],
    concurrency_results: &[ConcurrencyResult],
    ui_results: &[UiTickResult],
    file_results: &[FileIoResult],
) -> String {
    let mut out = String::new();
    out.push_str("# 📊 Báo Cáo Hiệu Năng Toàn Diện & Thực Tế (`uwulog-rust`)\n\n");
    out.push_str(&format!(
        "- **Ngày thử nghiệm**: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    out.push_str(&format!(
        "- **CPU**: {} ({} Logical Cores)\n",
        cpu_brand, logical_cores
    ));
    out.push_str(&format!("- **RAM**: {:.2} GB\n", total_ram_gb));
    out.push_str(&format!("- **Quy mô tập log in-memory**: {} dòng\n", logs));
    out.push_str(&format!(
        "- **Tốc độ Ingestion vào RAM**: {} logs/giây\n",
        ingest_rate as u64
    ));
    out.push_str(&format!(
        "- **RAM RSS Footprint**: {:.2} MB\n\n",
        mem_used_mb
    ));

    out.push_str("## 1. Phân Phối Độ Trễ & Throughput Lọc In-Memory (7 Cấp Độ)\n\n");
    out.push_str("| STT | Kịch Bản Query | Cú Pháp | p50 (ms) | p95 (ms) | p99 (ms) | Max (ms) | QPS | Throughput (Logs/s) |\n");
    out.push_str("| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |\n");

    for (i, (bq, stats, _, _, _)) in query_results.iter().enumerate() {
        let escaped_q = if bq.query.is_empty() {
            "*(rỗng)*"
        } else {
            bq.query
        };
        out.push_str(&format!(
            "| {} | **{}** | `{}` | {:.2} | {:.2} | {:.2} | {:.2} | {:.1} | {:.2}M logs/s |\n",
            i + 1,
            bq.name,
            escaped_q,
            stats.p50_ms,
            stats.p95_ms,
            stats.p99_ms,
            stats.max_ms,
            stats.qps,
            stats.throughput_m_logs_sec
        ));
    }

    out.push_str("\n## 2. Hiệu Suất Tối Ưu Đa Nhân Rayon (CPU Multi-Core Scaling)\n\n");
    out.push_str("| Số Cores / Threads | Thời Gian (ms) | Hệ Số Tăng Tốc (Speedup $S_N$) | Hiệu Suất Đa Nhân (Efficiency $E_N$) |\n");
    out.push_str("| :--- | :---: | :---: | :---: |\n");

    for res in scaling_results {
        let tag = if res.threads == logical_cores {
            " (Max System Cores)"
        } else {
            ""
        };
        out.push_str(&format!(
            "| **{}{}** | {:.2} | **{:.2}x** | {:.1}% |\n",
            res.threads, tag, res.duration_ms, res.speedup, res.efficiency
        ));
    }

    out.push_str("\n## 3. Khả Năng Chịu Tải Đồng Thời (Real-World Concurrency Stress)\n\n");
    out.push_str("| Tốc Độ Stream Log Nền | Độ Trễ Khi Idle (ms) | Độ Trễ Dưới Tải Stream (ms) | Tranh Chấp Lock Overhead |\n");
    out.push_str("| :--- | :---: | :---: | :---: |\n");

    for c in concurrency_results {
        out.push_str(&format!(
            "| **{} logs/giây** | {:.2} | {:.2} | **+{:.1}%** |\n",
            c.stream_rate, c.idle_search_ms, c.contention_search_ms, c.overhead_pct
        ));
    }

    out.push_str("\n## 4. Ngân Sách Khung Hình Desktop UI (60 FPS / < 16.6ms Target)\n\n");
    out.push_str("| Số Log Mới/Frame | Thời Gian Xử Lý Tick (ms) | Tần Số Khung Hình Đạt Được | Đánh Giá Mượt Mà (60 FPS) |\n");
    out.push_str("| :--- | :---: | :---: | :---: |\n");

    for u in ui_results {
        let status = if u.within_60fps_budget {
            "✅ Đạt chuẩn 60 FPS"
        } else {
            "⚠️ Giảm nhẹ FPS"
        };
        out.push_str(&format!(
            "| **{} logs/frame** | {:.2} ms | {:.1} FPS | {} |\n",
            u.batch_size, u.tick_time_ms, u.fps_equivalent, status
        ));
    }

    out.push_str("\n## 5. Tốc Độ Đọc File End-to-End Từ Ổ Cứng (Disk -> Parse -> Engine)\n\n");
    out.push_str("| Số Dòng Log | Dung Lượng File (MB) | Thời Gian Đọc & Parse (ms) | Throughput I/O (MB/s) | Tốc Độ Nạp (Logs/s) |\n");
    out.push_str("| :--- | :---: | :---: | :---: | :---: |\n");

    for f in file_results {
        out.push_str(&format!(
            "| **{} dòng** | {:.2} MB | {:.2} ms | **{:.2} MB/s** | **{:.0} logs/s** |\n",
            f.line_count, f.file_size_mb, f.duration_ms, f.throughput_mb_s, f.throughput_logs_s
        ));
    }

    out.push_str("\n## 6. Mức Độ Tiêu Thụ Tài Nguyên Tổng Thể\n\n");
    out.push_str(&format!("- **Peak Process CPU**: {:.1}%\n", peak_cpu));
    out.push_str(&format!("- **Average Query CPU**: {:.1}%\n", avg_cpu));
    out.push_str(&format!(
        "- **Bộ nhớ RAM tiêu thụ / log**: {:.2} bytes/log\n",
        if logs > 0 {
            (mem_used_mb * 1024.0 * 1024.0) / logs as f64
        } else {
            0.0
        }
    ));

    out
}
