# 📊 Báo Cáo Hiệu Năng Toàn Diện & Thực Tế (`uwulog-rust`)

- **Ngày thử nghiệm**: 2026-09-07 18:06:07
- **CPU**: 12th Gen Intel(R) Core(TM) i5-12450H (12 Logical Cores)
- **RAM**: 31.63 GB
- **Quy mô tập log in-memory**: 500000 dòng
- **Tốc độ Ingestion vào RAM**: 33496348 logs/giây
- **RAM RSS Footprint**: 436.41 MB

## 🔍 So Sánh Hiệu Năng Với Baseline (`Commit d99036a (change uuid to u64)` vs `Code Hien Tai (Working Tree)`)

| Chỉ Số | Baseline (`Commit d99036a (change uuid to u64)`) | Hiện Tại (`Code Hien Tai (Working Tree)`) | Chênh Lệch (Δ) | Đánh Giá |
| :--- | :---: | :---: | :---: | :---: |
| **Tốc độ Ingestion vào RAM** | 31.90M logs/s | **33.50M logs/s** | **+5.0%** | 🚀 Cao hơn |
| **RAM Footprint / log** | 924.05 bytes | **915.23 bytes** | **-0.95% (-8.8 B)** | 📉 Tiết kiệm RAM |
| **RAM RSS Tổng** | 440.62 MB | **436.41 MB** | **-1.0%** | 📉 Giảm RAM |

### Bảng So Sánh Độ Trễ p50 Từng Kịch Bản Query

| STT | Kịch Bản Query | p50 Baseline (ms) | p50 Hiện Tại (ms) | Chênh Lệch p50 (Δ) | Throughput Hiện Tại | Đánh Giá |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| 1 | **01_empty_query** | 0.81 ms | **0.48 ms** | **-40.5%** | 826.23M logs/s | 🚀 Nhanh hơn |
| 2 | **02_substring_match** | 34.81 ms | **30.74 ms** | **-11.7%** | 15.98M logs/s | 🚀 Nhanh hơn |
| 3 | **03_exact_field_match** | 3.79 ms | **3.14 ms** | **-17.1%** | 157.51M logs/s | 🚀 Nhanh hơn |
| 4 | **04_regex_match** | 16.36 ms | **14.04 ms** | **-14.2%** | 36.05M logs/s | 🚀 Nhanh hơn |
| 5 | **05_numeric_range** | 11.80 ms | **10.89 ms** | **-7.7%** | 45.84M logs/s | 🚀 Nhanh hơn |
| 6 | **06_time_window** | 2.37 ms | **2.28 ms** | **-3.6%** | 218.10M logs/s | 🚀 Nhanh hơn |
| 7 | **07_complex_ast_boolean** | 9.96 ms | **9.18 ms** | **-7.8%** | 53.62M logs/s | 🚀 Nhanh hơn |

---

## 1. Phân Phối Độ Trễ & Throughput Lọc In-Memory (7 Cấp Độ)

| STT | Kịch Bản Query | Cú Pháp | p50 (ms) | p95 (ms) | p99 (ms) | Max (ms) | QPS | Throughput (Logs/s) |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | **01_empty_query** | `*(rỗng)*` | 0.48 | 1.15 | 2.64 | 2.64 | 1652.5 | 826.23M logs/s |
| 2 | **02_substring_match** | `database connection timeout` | 30.74 | 35.44 | 38.31 | 38.31 | 32.0 | 15.98M logs/s |
| 3 | **03_exact_field_match** | `level:error` | 3.14 | 3.63 | 3.67 | 3.67 | 315.0 | 157.51M logs/s |
| 4 | **04_regex_match** | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | 14.04 | 15.10 | 15.30 | 15.30 | 72.1 | 36.05M logs/s |
| 5 | **05_numeric_range** | `latency:>=500 duration:100..800` | 10.89 | 11.35 | 11.61 | 11.61 | 91.7 | 45.84M logs/s |
| 6 | **06_time_window** | `timestamp:now..15m` | 2.28 | 2.61 | 2.70 | 2.70 | 436.2 | 218.10M logs/s |
| 7 | **07_complex_ast_boolean** | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | 9.18 | 10.08 | 10.11 | 10.11 | 107.2 | 53.62M logs/s |

## 2. Hiệu Suất Tối Ưu Đa Nhân Rayon (CPU Multi-Core Scaling)

| Số Cores / Threads | Thời Gian (ms) | Hệ Số Tăng Tốc (Speedup $S_N$) | Hiệu Suất Đa Nhân (Efficiency $E_N$) |
| :--- | :---: | :---: | :---: |
| **1** | 54.47 | **1.00x** | 100.0% |
| **2** | 29.54 | **1.84x** | 92.2% |
| **4** | 16.44 | **3.31x** | 82.8% |
| **8** | 11.15 | **4.88x** | 61.0% |
| **12 (Max System Cores)** | 9.51 | **5.73x** | 47.8% |

## 3. Khả Năng Chịu Tải Đồng Thời (Real-World Concurrency Stress)

| Tốc Độ Stream Log Nền | Độ Trễ Khi Idle (ms) | Độ Trễ Dưới Tải Stream (ms) | Tranh Chấp Lock Overhead |
| :--- | :---: | :---: | :---: |
| **10000 logs/giây** | 9.51 | 9.27 | **+-2.5%** |
| **50000 logs/giây** | 9.51 | 9.14 | **+-3.9%** |

## 4. Ngân Sách Khung Hình Desktop UI (60 FPS / < 16.6ms Target)

| Số Log Mới/Frame | Thời Gian Xử Lý Tick (ms) | Tần Số Khung Hình Đạt Được | Đánh Giá Mượt Mà (60 FPS) |
| :--- | :---: | :---: | :---: |
| **100 logs/frame** | 0.11 ms | 9304.9 FPS | ✅ Đạt chuẩn 60 FPS |
| **500 logs/frame** | 0.45 ms | 2231.3 FPS | ✅ Đạt chuẩn 60 FPS |
| **2000 logs/frame** | 1.93 ms | 517.7 FPS | ✅ Đạt chuẩn 60 FPS |
| **5000 logs/frame** | 4.96 ms | 201.7 FPS | ✅ Đạt chuẩn 60 FPS |

## 5. Tốc Độ Đọc File End-to-End Từ Ổ Cứng (Disk -> Parse -> Engine)

| Số Dòng Log | Dung Lượng File (MB) | Thời Gian Đọc & Parse (ms) | Throughput I/O (MB/s) | Tốc Độ Nạp (Logs/s) |
| :--- | :---: | :---: | :---: | :---: |
| **50000 dòng** | 6.95 MB | 131.87 ms | **52.68 MB/s** | **379173 logs/s** |
| **100000 dòng** | 13.91 MB | 269.33 ms | **51.63 MB/s** | **371290 logs/s** |

## 6. Mức Độ Tiêu Thụ Tài Nguyên Tổng Thể

- **Peak Process CPU**: 200.0%
- **Average Query CPU**: 85.7%
- **Bộ nhớ RAM tiêu thụ / log**: 915.23 bytes/log
