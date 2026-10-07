use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use uwu_benchmarks::SyntheticLogGenerator;
use uwu_core_engine::SystemEngine;

/// Giả lập chu kỳ 1 Frame của Desktop UI (egui) ở tần số quét 60 FPS (ngân sách < 16.6ms)
fn bench_ui_frame_cycle(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let mut group = c.benchmark_group("ui_frame_budget_60fps");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(8));

    let initial_logs = 200_000;
    let mut gen = SyntheticLogGenerator::new(404);
    let events = gen.generate_events(initial_logs);

    let engine = SystemEngine::new(initial_logs + 50_000);
    engine.push_events(events);

    // Các kịch bản số lượng log mới ập về trong 1 frame (16.6ms)
    let incoming_batch_sizes = [100, 500, 2_000, 10_000];
    let active_query = "(level:error OR level:warn) AND tag:auth";
    let visible_rows_limit = 60; // Số hàng render trong viewport của Virtual Table

    for &batch_size in &incoming_batch_sizes {
        let label = format!("{}_new_logs_per_frame", batch_size);

        group.bench_with_input(
            BenchmarkId::new("ui_tick_processing", label),
            &batch_size,
            |b, &b_size| {
                let mut last_processed = engine.total_processed();
                let mut p_gen = SyntheticLogGenerator::new(808);

                b.iter(|| {
                    // 1. Giả lập log mới được nạp vào engine
                    let new_events = p_gen.generate_events(b_size);
                    engine.push_events(new_events);

                    // 2. UI Tick: Incremental filtering khi đang stream
                    let (_matched_len, new_matched_logs) = engine
                        .filter_incremental(black_box(active_query), black_box(last_processed));

                    // 3. Virtual Scrolling: Lấy 60 dòng mới nhất để vẽ lên màn hình
                    let skip_count = new_matched_logs.len().saturating_sub(visible_rows_limit);
                    let visible_rows: Vec<_> =
                        new_matched_logs.into_iter().skip(skip_count).collect();

                    // 4. Schema Registry sync O(1)
                    let schema = engine.get_schema_map();

                    last_processed = engine.total_processed();

                    black_box((visible_rows, schema));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_ui_frame_cycle);
criterion_main!(benches);
