use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use uwu_core_engine::SystemEngine;
use uwu_core_schema::{RawLogEntry, RawPayload};
use uwu_driver_sources::LogNormalizer;

fn create_temp_log_file(line_count: usize) -> (NamedTempFile, u64) {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let mut total_bytes = 0u64;
    for i in 0..line_count {
        let json_line = serde_json::json!({
            "timestamp": "2026-09-03T12:00:00Z",
            "level": if i % 10 == 0 { "ERROR" } else if i % 4 == 0 { "WARN" } else { "INFO" },
            "message": format!("Disk write operation #{} completed with status OK", i),
            "tag": "sys.storage",
            "latency": 50 + (i % 500),
            "source": "worker-disk"
        });
        let line_str = json_line.to_string();
        writeln!(file, "{}", line_str).unwrap();
        total_bytes += line_str.len() as u64 + 1;
    }
    file.flush().unwrap();
    (file, total_bytes)
}

fn bench_file_ingestion_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let mut group = c.benchmark_group("file_ingestion_e2e");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    let dataset_sizes = [10_000, 50_000];

    for count in dataset_sizes {
        let (temp_file, total_bytes) = create_temp_log_file(count);
        let path = temp_file.path().to_path_buf();

        group.throughput(Throughput::Bytes(total_bytes));
        let label = format!(
            "{}_lines_{:.2}MB",
            count,
            total_bytes as f64 / (1024.0 * 1024.0)
        );

        group.bench_with_input(
            BenchmarkId::new("read_file_to_engine", label),
            &path,
            |b, file_path| {
                b.to_async(&rt).iter(|| async {
                    let engine = Arc::new(SystemEngine::new(count + 1000));
                    let file = File::open(file_path).await.unwrap();
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
                                engine.push_events(std::mem::take(&mut batch));
                            }
                        }
                        line.clear();
                    }

                    if !batch.is_empty() {
                        engine.push_events(batch);
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_file_ingestion_pipeline);
criterion_main!(benches);
