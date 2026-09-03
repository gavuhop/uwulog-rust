use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rayon::ThreadPoolBuilder;
use std::time::Duration;
use uwu_benchmarks::SyntheticLogGenerator;
use uwu_core_engine::SystemEngine;

fn bench_rayon_cpu_scaling_500k(c: &mut Criterion) {
    let _rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = _rt.enter();

    let count = 500_000;
    let mut gen = SyntheticLogGenerator::new(777);
    let events = gen.generate_events(count);

    let engine = SystemEngine::new(count);
    engine.push_events(events);

    let max_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut thread_configs = vec![1, 2, 4];
    if max_cores >= 8 {
        thread_configs.push(8);
    }
    if !thread_configs.contains(&max_cores) {
        thread_configs.push(max_cores);
    }
    thread_configs.sort_unstable();

    let complex_query =
        "(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat";

    let mut group = c.benchmark_group("cpu_scaling_500k_complex_query");
    group.sample_size(15);
    group.measurement_time(Duration::from_secs(8));
    group.throughput(Throughput::Elements(count as u64));

    for &num_threads in &thread_configs {
        let pool = ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .expect("Failed to build rayon thread pool");

        let bench_label = format!("{}_threads", num_threads);

        group.bench_with_input(
            BenchmarkId::new("rayon_scaling", bench_label),
            &num_threads,
            |b, _| {
                b.iter(|| {
                    pool.install(|| {
                        let res = engine.search(black_box(complex_query));
                        black_box(res);
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_rayon_cpu_scaling_5m(c: &mut Criterion) {
    let _rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = _rt.enter();

    let count = 5_000_000;
    let mut gen = SyntheticLogGenerator::new(888);
    let events = gen.generate_events(count);

    let engine = SystemEngine::new(count);
    engine.push_events(events);

    let max_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut thread_configs = vec![1, 2, 4];
    if max_cores >= 8 {
        thread_configs.push(8);
    }
    if !thread_configs.contains(&max_cores) {
        thread_configs.push(max_cores);
    }
    thread_configs.sort_unstable();

    let complex_query =
        "(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat";

    let mut group = c.benchmark_group("cpu_scaling_5m_complex_query");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(12));
    group.throughput(Throughput::Elements(count as u64));

    for &num_threads in &thread_configs {
        let pool = ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .expect("Failed to build rayon thread pool");

        let bench_label = format!("{}_threads", num_threads);

        group.bench_with_input(
            BenchmarkId::new("rayon_scaling", bench_label),
            &num_threads,
            |b, _| {
                b.iter(|| {
                    pool.install(|| {
                        let res = engine.search_limited(black_box(complex_query), 1000);
                        black_box(res);
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_rayon_cpu_scaling_500k,
    bench_rayon_cpu_scaling_5m
);
criterion_main!(benches);
