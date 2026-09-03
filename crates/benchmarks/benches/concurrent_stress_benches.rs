use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uwu_benchmarks::SyntheticLogGenerator;
use uwu_core_engine::SystemEngine;

fn bench_concurrent_stream_and_search(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let mut group = c.benchmark_group("concurrent_streaming_stress");
    group.sample_size(15);
    group.measurement_time(Duration::from_secs(8));

    // Kịch bản: Bơm log liên tục ở tốc độ cao (50k logs/giây) và đo độ trễ tìm kiếm
    let capacity = 500_000;
    let rates = [10_000, 50_000, 100_000]; // logs per second

    for rate in rates {
        let engine = Arc::new(SystemEngine::new(capacity));

        // Nạp trước 100k log để engine có dữ liệu
        let mut gen = SyntheticLogGenerator::new(123);
        let pre_events = gen.generate_events(100_000);
        engine.push_events(pre_events);

        let is_running = Arc::new(AtomicBool::new(true));
        let tx = engine.get_channel();

        let is_running_clone = Arc::clone(&is_running);
        // Producer task bắn log nền liên tục theo rate
        let producer_handle = rt.spawn(async move {
            let mut p_gen = SyntheticLogGenerator::new(999);
            let batch_size = (rate / 100).max(10);
            let sleep_interval = Duration::from_millis(10);

            while is_running_clone.load(Ordering::Relaxed) {
                let raw_entries = p_gen.generate_raw_json_entries(batch_size);
                for entry in raw_entries {
                    if tx.send(entry).await.is_err() {
                        break;
                    }
                }
                sleep(sleep_interval).await;
            }
        });

        let query = "(level:error OR level:warn) AND tag:auth AND latency:>200";
        let label = format!("{}_logs_per_sec_stream", rate);

        group.throughput(Throughput::Elements(100_000));
        group.bench_with_input(
            BenchmarkId::new("search_with_write_contention", label),
            &query,
            |b, q| {
                b.iter(|| {
                    let res = engine.search_limited(black_box(q), 1000);
                    black_box(res);
                });
            },
        );

        is_running.store(false, Ordering::Relaxed);
        let _ = rt.block_on(producer_handle);
    }

    group.finish();
}

criterion_group!(benches, bench_concurrent_stream_and_search);
criterion_main!(benches);
