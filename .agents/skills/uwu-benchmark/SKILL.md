---
name: uwu-benchmark
description: >-
  Chạy benchmark hiệu năng, đo kiểm CPU/RAM profiler, và báo cáo tổng hợp kết quả benchmark cho dự án uwulog-rust.
  Kích hoạt skill này khi người dùng yêu cầu chạy benchmark, kiểm tra tốc độ xử lý log, đo độ trễ truy vấn (latency), kiểm thử đa luồng Rayon, hoặc tạo báo cáo hiệu năng.
---

# 🚀 uwulog-rust Benchmark & Performance Profiler Skill

Skill này hướng dẫn quy trình tiêu chuẩn để thực thi các bộ đo kiểm hiệu năng (Benchmarks), trích xuất các chỉ số thời gian thực (CPU, RAM, Throughput, Latency phân vị), và tổng hợp báo cáo kết quả chi tiết cho dự án `uwulog-rust`.

---

## 1. Các Công Cụ Benchmark Trong Dự Án

Dự án `uwulog-rust` sở hữu hai hệ thống đo kiểm bổ trợ cho nhau:

1. **Live End-to-End Profiler CLI ([`bench_profile`](file:///D:/Learn/Go/uwulog-rust/crates/benchmarks/src/bin/bench_profile.rs))**:
   * Chạy nhanh, trực quan, đo toàn diện 6 khía cạnh:
     1. Ingestion throughput vào RAM RingBuffer & RAM RSS footprint trên mỗi log.
     2. Ma trận phân vị độ trễ (Cold Run, Warm p50, Warm p95, Max, QPS) qua [7 kịch bản truy vấn](./references/query_scenarios.md).
     3. Hệ số tăng tốc đa nhân Rayon (Speedup $S_N$ & Efficiency $E_N$ từ 1 đến 12 cores).
     4. Khả năng chịu tải đồng thời (tranh chấp Lock khi luồng nền nạp 10k - 50k logs/giây).
     5. Ngân sách khung hình Desktop UI (kiểm tra chuẩn 60 FPS / < 16.6ms).
     6. Tốc độ đọc File end-to-end từ ổ cứng (`MB/s` và `logs/s`).
   * Tự động xuất 2 file báo cáo Markdown chuẩn:
     * [`BENCHMARK_REPORT.md`](file:///D:/Learn/Go/uwulog-rust/BENCHMARK_REPORT.md): Kết quả benchmark toàn diện của **code hiện tại**.
     * [`benchmark_compare_report.md`](file:///D:/Learn/Go/uwulog-rust/benchmark_compare_report.md): Báo cáo **so sánh đối chiếu** chi tiết giữa commit trước và code hiện tại.

2. **Criterion Micro & Integration Benches ([`crates/benchmarks/benches/`](file:///D:/Learn/Go/uwulog-rust/crates/benchmarks/benches))**:
   * Đo kiểm thống kê micro-benchmarks với độ chính xác nano-giây:
     * `micro_benches`: Tokenizer, AST Parser, Evaluator, Normalizer JSON & Plain Text.
     * `engine_search_benches`: Tìm kiếm trên quy mô 10k, 500k, 5M logs.
     * `cpu_scaling_benches`: Đánh giá scaling 1..12 luồng Rayon.
     * `concurrent_stress_benches`: Stream background write + query read.
     * `file_ingestion_benches`: Đọc file 10k, 50k dòng.
     * `ui_simulation_benches`: Tick render 100, 500, 2000, 10000 logs/frame.

---

## 2. Các Lệnh Thực Thi Tiêu Chuẩn

### A. Chế Độ So Sánh Hiệu Năng Với Commit Trước (Khuyên Dùng / Mặc Định)
Tự động dựng Git worktree tạm để đo tươi mới commit baseline (`HEAD~1` nếu clean, hoặc `HEAD` nếu dirty) tại cùng thời điểm, sau đó đo code hiện tại và đối chiếu:
```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/uwu-benchmark/scripts/run_bench.ps1 -Mode compare
```
*Tùy chọn so sánh với một commit cụ thể:*
```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/uwu-benchmark/scripts/run_bench.ps1 -Mode compare -BaselineCommit "HEAD~2" -Logs 500000 -Queries 50
```

*Hoặc chạy trực tiếp binary `bench_profile` với cờ `--compare` và `--compare-md`:*
```bash
cargo run --release --bin bench_profile -- --logs 500000 --queries 50 --output-md BENCHMARK_REPORT.md --compare-md benchmark_compare_report.md --compare target/baseline_snapshot.json --baseline-label "Commit cũ" --current-label "Code mới"
```

### B. Chế Độ Profiler Nhanh (Quick Profile)
Dùng để kiểm tra nhanh hiệu năng sau các đợt refactor nhỏ:
```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/uwu-benchmark/scripts/run_bench.ps1 -Mode quick
# hoặc:
cargo run --release --bin bench_profile -- --logs 100000 --queries 50
```

### C. Chế Độ Profiler Đơn Lẻ & Xuất Báo Cáo Không So Sánh (Standalone Full)
```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/uwu-benchmark/scripts/run_bench.ps1 -Mode full
# hoặc:
cargo run --release --bin bench_profile -- --logs 500000 --queries 100 --output-md BENCHMARK_REPORT.md
```

### D. Chạy Criterion Micro-Benchmarks
```bash
# Chạy riêng bộ micro benchmarks
cargo bench -p uwu-benchmarks --bench micro_benches

# Chạy toàn bộ Criterion suite
cargo bench -p uwu-benchmarks
```

---

## 3. Quy Trình Phân Tích & Báo Cáo So Sánh Kết Quả

Khi người dùng yêu cầu chạy benchmark, **luôn ưu tiên chế độ so sánh** (Compare Mode) để chỉ rõ tác động của các thay đổi code đối với hiệu năng:

### Bước 1: Thực Thi Đo Kiểm So Sánh
Chạy helper script `run_bench.ps1`:
```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/uwu-benchmark/scripts/run_bench.ps1 -Mode compare -Logs 500000 -Queries 50
```
Script sẽ tự động:
1. Dựng git worktree tạm thời cho commit baseline và biên dịch/đo kiểm trực tiếp tại thời điểm chạy (bảo đảm môi trường nhiệt độ CPU và tải hệ thống là công bằng, không dùng kết quả lưu cũ).
2. Chạy benchmark trên code hiện tại.
3. Tính toán chênh lệch tỷ lệ phần trăm (Delta $\Delta$\%), gán nhãn trạng thái (🚀 Nhanh hơn, 📉 Tiết kiệm RAM, ⚠️ Chậm hơn, ➖ Tương đương).
4. Xuất kết quả của code hiện tại vào [`BENCHMARK_REPORT.md`](file:///D:/Learn/Go/uwulog-rust/BENCHMARK_REPORT.md).
5. Xuất báo cáo so sánh đối chiếu chi tiết vào [`benchmark_compare_report.md`](file:///D:/Learn/Go/uwulog-rust/benchmark_compare_report.md).

### Bước 2: Phân Tích Các Chỉ Số Chênh Lệch (Delta Analysis)
Đánh giá mức độ cải thiện/suy giảm dựa trên các tiêu chí:
* **Ingestion Throughput (Logs/s)**: $\Delta > +3\%$ là cải thiện rõ rệt (🚀).
* **RAM Footprint / log (bytes)**: Mọi sự sụt giảm $\Delta < 0\%$ đều là tối ưu bộ nhớ tích cực (📉).
* **Độ trễ p50 từng kịch bản truy vấn**:
  * Giảm $\Delta < -3\%$: 🚀 Nhanh hơn.
  * Tăng $\Delta > +5\%$: ⚠️ Cảnh báo suy giảm hiệu năng, cần phân tích nguyên nhân (ví dụ: cấp phát heap thêm, regex clone, khóa lock).
* **Đa luồng Rayon & Lock Contention**: Đảm bảo hệ số tăng tốc $\ge 4.0x$ và lock overhead $\le 15\%$.

### Bước 3: Định Dạng Báo Cáo Tổng Hợp Gửi Người Dùng
Báo cáo gửi người dùng cần cấu trúc như sau:
1. **Thông tin so sánh**: Commit Baseline vs Code hiện tại, cấu hình phần cứng và quy mô log.
2. **Bảng tổng hợp Core Metrics**: Ingestion rate, RAM/log, RAM RSS tổng kèm cột Delta $\Delta$\% và đánh giá.
3. **Bảng so sánh p50 từng kịch bản query**: Đối chiếu p50 cũ vs mới, chênh lệch \% và throughput hiện tại.
4. **Phân tích nguyên nhân & Nhận xét**: Lý giải tại sao chỉ số tăng/giảm (ví dụ: đổi từ `Uuid` 128-bit sang `u64` giúp giảm 8.7 B/log và tăng Ingestion 7.9%).

---

## 4. Tài Liệu Tham Khảo

* File báo cáo hiệu năng hiện tại: [`BENCHMARK_REPORT.md`](file:///D:/Learn/Go/uwulog-rust/BENCHMARK_REPORT.md)
* File báo cáo so sánh đối chiếu: [`benchmark_compare_report.md`](file:///D:/Learn/Go/uwulog-rust/benchmark_compare_report.md)
* File cấu hình tác vụ Zed Editor: [`.zed/tasks.json`](file:///D:/Learn/Go/uwulog-rust/.zed/tasks.json)
* Script tự động hóa runner: [run_bench.ps1](./scripts/run_bench.ps1)

