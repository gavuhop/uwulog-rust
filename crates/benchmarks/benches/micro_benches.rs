use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use uwu_benchmarks::{SyntheticLogGenerator, BENCHMARK_QUERIES};
use uwu_core_filter::evaluator::eval_event;
use uwu_core_filter::parser::{tokenize, Parser};
use uwu_core_util::now_secs;
use uwu_driver_sources::LogNormalizer;

fn bench_tokenizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_tokenizer");

    for bq in BENCHMARK_QUERIES {
        if bq.query.is_empty() {
            continue;
        }
        group.bench_with_input(BenchmarkId::new("tokenize", bq.name), &bq.query, |b, q| {
            b.iter(|| tokenize(black_box(q)));
        });
    }

    group.finish();
}

fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_parser");
    let now = now_secs();

    for bq in BENCHMARK_QUERIES {
        if bq.query.is_empty() {
            continue;
        }
        let tokens = tokenize(bq.query);
        group.bench_with_input(
            BenchmarkId::new("parse_ast", bq.name),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    let mut parser = Parser::new(toks.clone(), black_box(now));
                    parser.parse()
                });
            },
        );
    }

    group.finish();
}

fn bench_ast_evaluator(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_evaluator");
    let now = now_secs();
    let mut gen = SyntheticLogGenerator::new(42);
    let sample_events = gen.generate_events(100);

    for bq in BENCHMARK_QUERIES {
        if bq.query.is_empty() {
            continue;
        }
        let tokens = tokenize(bq.query);
        let mut parser = Parser::new(tokens, now);
        if let Some(ast) = parser.parse() {
            group.bench_with_input(
                BenchmarkId::new("eval_100_events", bq.name),
                &ast,
                |b, ast_node| {
                    b.iter(|| {
                        let mut match_count = 0;
                        for event in &sample_events {
                            if eval_event(ast_node, black_box(event), black_box(now)) {
                                match_count += 1;
                            }
                        }
                        black_box(match_count);
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_log_normalizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_normalizer");
    let mut gen = SyntheticLogGenerator::new(12345);

    let raw_json_batch = gen.generate_raw_json_entries(1000);
    let raw_text_batch = gen.generate_raw_text_entries(1000);

    group.bench_function("normalize_1000_json_logs", |b| {
        b.iter(|| {
            for entry in &raw_json_batch {
                let ev = LogNormalizer::normalize(black_box(entry.clone()));
                black_box(ev);
            }
        });
    });

    group.bench_function("normalize_1000_plain_text_logs", |b| {
        b.iter(|| {
            for entry in &raw_text_batch {
                let ev = LogNormalizer::normalize(black_box(entry.clone()));
                black_box(ev);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tokenizer,
    bench_parser,
    bench_ast_evaluator,
    bench_log_normalizer
);
criterion_main!(benches);
