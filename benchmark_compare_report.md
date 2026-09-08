# ⚖️ Báo Cáo So Sánh Hiệu Năng (Benchmark Comparison Report)

- **Baseline (Commit Trước)**: `Commit ca8d3ae (create benchmark report and benchmark skill)`
- **Current (Code Hiện Tại)**: `Code Hien Tai (Working Tree)`
- **Thời điểm kiểm thử**: 2026-09-08 09:54:49
- **Cấu hình CPU**: 12th Gen Intel(R) Core(TM) i5-12450H (12 Logical Cores)
- **Tổng RAM hệ thống**: 31.63 GB
- **Quy mô tập log**: 500000 dòng
- **Số lượt lặp mỗi query**: 50 iterations

> [!NOTE]
> Cả commit trước (Baseline) và code hiện tại đều được biên dịch và đo lường trực tiếp tại cùng một thời điểm kiểm thử để loại bỏ hoàn toàn sai số do biến thiên nhiệt độ CPU (thermal throttling) và tải nền hệ điều hành.

## 📌 1. So Sánh Các Chỉ Số Cốt Lõi (Core Ingestion & Memory Footprint)

| Chỉ Số | Baseline (`Commit ca8d3ae (create benchmark report and benchmark skill)`) | Hiện Tại (`Code Hien Tai (Working Tree)`) | Chênh Lệch (Δ) | Đánh Giá |
| :--- | :---: | :---: | :---: | :---: |
| **Tốc độ Ingestion vào RAM** | 34.28M logs/s | **27.50M logs/s** | **-19.8%** | ⚠️ Thấp hơn |
| **RAM Footprint / log** | 915.64 bytes | **915.51 bytes** | **-0.01%** | ➖ Không đổi |
| **RAM RSS Tổng** | 436.61 MB | **436.55 MB** | **-0.0%** | ➖ Tương đương |

## 📌 2. So Sánh Phân Vị Độ Trễ Truy Vấn (Query Latency Comparison Matrix)

| STT | Kịch Bản Query | Cú Pháp | Baseline p50 | Hiện Tại Cold | Hiện Tại Warm p50 | Chênh Lệch p50 (Δ) | Throughput Hiện Tại | Đánh Giá |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | **01_empty_query** | `*(rỗng)*` | 0.52 ms | 0.86 ms | **0.57 ms** | **+11.0%** | 812.29M logs/s | ⚠️ Chậm hơn |
| 2 | **02_substring_match** | `database connection timeout` | 42.83 ms | 38.03 ms | **34.60 ms** | **-19.2%** | 13.56M logs/s | 🚀 Nhanh hơn |
| 3 | **03_exact_field_match** | `level:error` | 7.44 ms | 7.11 ms | **7.43 ms** | **-0.1%** | 67.39M logs/s | ➖ Tương đương |
| 4 | **04_regex_match** | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | 37.75 ms | 35.13 ms | **39.65 ms** | **+5.0%** | 12.74M logs/s | ⚠️ Chậm hơn |
| 5 | **05_numeric_range** | `latency:>=500 duration:100..800` | 32.73 ms | 33.21 ms | **33.75 ms** | **+3.1%** | 14.82M logs/s | ⚠️ Chậm hơn |
| 6 | **06_time_window** | `timestamp:now..15m` | 5.31 ms | 5.41 ms | **4.92 ms** | **-7.3%** | 101.05M logs/s | 🚀 Nhanh hơn |
| 7 | **07_complex_ast_boolean** | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | 27.85 ms | 25.53 ms | **26.99 ms** | **-3.1%** | 18.33M logs/s | 🚀 Nhanh hơn |

## 📌 3. Đánh Giá Kỹ Thuật & Nhận Xét Tổng Quan

- **Tốc độ Ingestion & Băng thông Nạp Log**: Phiên bản hiện tại đạt tốc độ nạp 27.50 triệu logs/s (so với 34.28 triệu logs/s ở baseline, chênh lệch -19.8%).
- **Mức Tiêu Thụ RAM**: Chiếm dụng bộ nhớ đạt 436.55 MB (915.51 bytes/log), so với 436.61 MB (915.64 bytes/log) ở baseline.
- **Hiệu Năng Truy Vấn Phân Vị (Latency p50)**: Cả hai phiên bản đều được đo lường thực tế liên tiếp nhau trong cùng điều kiện phần cứng tức thời để phản ánh đúng thực tế tối ưu hóa mã nguồn mà không bị ảnh hưởng bởi biến thiên nhiệt độ CPU.
