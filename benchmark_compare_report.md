# ⚖️ Báo Cáo So Sánh Hiệu Năng (Benchmark Comparison Report)

- **Baseline (Commit Trước)**: `Commit 76e9834 (feat(agent): standardize agent RPC architecture and align CLI with app)`
- **Current (Code Hiện Tại)**: `Commit a1ab4cf`
- **Thời điểm kiểm thử**: 2026-09-11 23:51:23
- **Cấu hình CPU**: 12th Gen Intel(R) Core(TM) i5-12450H (12 Logical Cores)
- **Tổng RAM hệ thống**: 31.63 GB
- **Quy mô tập log**: 500000 dòng
- **Số lượt lặp mỗi query**: 50 iterations

> [!NOTE]
> Cả commit trước (Baseline) và code hiện tại đều được biên dịch và đo lường trực tiếp tại cùng một thời điểm kiểm thử để loại bỏ hoàn toàn sai số do biến thiên nhiệt độ CPU (thermal throttling) và tải nền hệ điều hành.

## 📌 1. So Sánh Các Chỉ Số Cốt Lõi (Core Ingestion & Memory Footprint)

| Chỉ Số | Baseline (`Commit 76e9834 (feat(agent): standardize agent RPC architecture and align CLI with app)`) | Hiện Tại (`Commit a1ab4cf`) | Chênh Lệch (Δ) | Đánh Giá |
| :--- | :---: | :---: | :---: | :---: |
| **Tốc độ Ingestion vào RAM** | 17.94M logs/s | **39.15M logs/s** | **+118.2%** | 🚀 Cao hơn |
| **RAM Footprint / log** | 1238.61 bytes | **797.32 bytes** | **-35.63% (-441.3 B)** | 📉 Tiết kiệm RAM |
| **RAM RSS Tổng** | 590.62 MB | **380.19 MB** | **-35.6%** | 📉 Giảm RAM |

## 📌 2. So Sánh Phân Vị Độ Trễ Truy Vấn (Query Latency Comparison Matrix)

| STT | Kịch Bản Query | Cú Pháp | Baseline p50 | Hiện Tại Cold | Hiện Tại Warm p50 | Chênh Lệch p50 (Δ) | Throughput Hiện Tại | Đánh Giá |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | **01_empty_query** | `*(rỗng)*` | 0.56 ms | 1.51 ms | **0.52 ms** | **-6.5%** | 789.22M logs/s | 🚀 Nhanh hơn |
| 2 | **02_substring_match** | `database connection timeout` | 39.16 ms | 26.20 ms | **29.16 ms** | **-25.5%** | 16.73M logs/s | 🚀 Nhanh hơn |
| 3 | **03_exact_field_match** | `level:error` | 7.57 ms | 6.87 ms | **7.64 ms** | **+0.9%** | 54.71M logs/s | ➖ Tương đương |
| 4 | **04_regex_match** | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | 48.00 ms | 24.71 ms | **29.70 ms** | **-38.1%** | 15.82M logs/s | 🚀 Nhanh hơn |
| 5 | **05_numeric_range** | `latency:>=500 duration:100..800` | 41.10 ms | 21.76 ms | **21.82 ms** | **-46.9%** | 22.02M logs/s | 🚀 Nhanh hơn |
| 6 | **06_time_window** | `timestamp:now..15m` | 5.19 ms | 5.93 ms | **5.27 ms** | **+1.5%** | 92.47M logs/s | ➖ Tương đương |
| 7 | **07_complex_ast_boolean** | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | 30.01 ms | 29.81 ms | **31.35 ms** | **+4.5%** | 14.56M logs/s | ⚠️ Chậm hơn |

## 📌 3. Đánh Giá Kỹ Thuật & Nhận Xét Tổng Quan

- **Tốc độ Ingestion & Băng thông Nạp Log**: Phiên bản hiện tại đạt tốc độ nạp 39.15 triệu logs/s (so với 17.94 triệu logs/s ở baseline, chênh lệch +118.2%).
- **Mức Tiêu Thụ RAM**: Chiếm dụng bộ nhớ đạt 380.19 MB (797.32 bytes/log), so với 590.62 MB (1238.61 bytes/log) ở baseline.
- **Hiệu Năng Truy Vấn Phân Vị (Latency p50)**: Cả hai phiên bản đều được đo lường thực tế liên tiếp nhau trong cùng điều kiện phần cứng tức thời để phản ánh đúng thực tế tối ưu hóa mã nguồn mà không bị ảnh hưởng bởi biến thiên nhiệt độ CPU.
