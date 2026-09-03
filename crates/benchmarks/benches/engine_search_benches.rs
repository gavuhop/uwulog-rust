use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use uwu_benchmarks::{SyntheticLogGenerator, BENCHMARK_QUERIES};
use uwu_core_engine::SystemEngine;

fn bench_engine_search_10k(c: &mut Criterion) {
    let _rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = _rt.enter();

    let count = 10_000;
    let mut gen = SyntheticLogGenerator::new(101);
    let events = gen.generate_events(count);

    let engine = SystemEngine::new(count);
    engine.push_events(events);

    let mut group = c.benchmark_group("engine_search_10k");
    group.throughput(Throughput::Elements(count as u64));

    for bq in BENCHMARK_QUERIES {
        group.bench_with_input(BenchmarkId::new("query", bq.name), &bq.query, |b, q| {
            b.iter(|| {
                let res = engine.search(black_box(q));
                black_box(res);
            });
        });
    }

    group.finish();
}

fn bench_engine_search_500k(c: &mut Criterion) {
    let _rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = _rt.enter();

    let count = 500_000;
    let mut gen = SyntheticLogGenerator::new(202);
    let events = gen.generate_events(count);

    let engine = SystemEngine::new(count);
    engine.push_events(events);

    let mut group = c.benchmark_group("engine_search_500k");
    group.sample_size(15);
    group.measurement_time(Duration::from_secs(8));
    group.throughput(Throughput::Elements(count as u64));

    for bq in BENCHMARK_QUERIES {
        group.bench_with_input(BenchmarkId::new("query", bq.name), &bq.query, |b, q| {
            b.iter(|| {
                let res = engine.search(black_box(q));
                black_box(res);
            });
        });
    }

    group.finish();
}

fn bench_engine_search_5m(c: &mut Criterion) {
    let _rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = _rt.enter();

    let count = 5_000_000;
    let mut gen = SyntheticLogGenerator::new(303);
    let events = gen.generate_events(count);

    let engine = SystemEngine::new(count);
    engine.push_events(events);

    let mut group = c.benchmark_group("engine_search_5m");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(12));
    group.throughput(Throughput::Elements(count as u64));

    // Test a selected subset of queries for 5M logs to keep total bench time reasonable
    let representative_queries = [
        &BENCHMARK_QUERIES[0], // Empty query fast-path
        &BENCHMARK_QUERIES[1], // Substring
        &BENCHMARK_QUERIES[2], // Exact field
        &BENCHMARK_QUERIES[4], // Numeric range
        &BENCHMARK_QUERIES[6], // Complex AST
    ];

    for bq in representative_queries {
        group.bench_with_input(BenchmarkId::new("query", bq.name), &bq.query, |b, q| {
            b.iter(|| {
                let res = engine.search_limited(black_box(q), 1000);
                black_box(res);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_engine_search_10k,
    bench_engine_search_500k,
    bench_engine_search_5m
);
criterion_main!(benches);
