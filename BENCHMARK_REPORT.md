# 📊 Báo Cáo Hiệu Năng Toàn Diện & Thực Tế (`uwulog-rust`)

- **Ngày thử nghiệm**: 2026-09-03 23:45:45
- **CPU**: 12th Gen Intel(R) Core(TM) i5-12450H (12 Logical Cores)
- **RAM**: 31.63 GB
- **Quy mô tập log in-memory**: 500000 dòng
- **Tốc độ Ingestion vào RAM**: 31896502 logs/giây
- **RAM RSS Footprint**: 440.62 MB

## 1. Phân Phối Độ Trễ & Throughput Lọc In-Memory (7 Cấp Độ)

| STT | Kịch Bản Query | Cú Pháp | p50 (ms) | p95 (ms) | p99 (ms) | Max (ms) | QPS | Throughput (Logs/s) |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | **01_empty_query** | `*(rỗng)*` | 0.81 | 1.53 | 1.58 | 1.58 | 1188.3 | 594.14M logs/s |
| 2 | **02_substring_match** | `database connection timeout` | 34.81 | 41.81 | 46.73 | 46.73 | 28.2 | 14.11M logs/s |
| 3 | **03_exact_field_match** | `level:error` | 3.79 | 5.05 | 12.75 | 12.75 | 245.5 | 122.74M logs/s |
| 4 | **04_regex_match** | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | 16.36 | 20.56 | 23.45 | 23.45 | 60.7 | 30.34M logs/s |
| 5 | **05_numeric_range** | `latency:>=500 duration:100..800` | 11.80 | 12.75 | 15.87 | 15.87 | 84.3 | 42.13M logs/s |
| 6 | **06_time_window** | `timestamp:now..15m` | 2.37 | 2.78 | 4.45 | 4.45 | 411.2 | 205.58M logs/s |
| 7 | **07_complex_ast_boolean** | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | 9.96 | 11.46 | 15.17 | 15.17 | 98.1 | 49.06M logs/s |

## 2. Hiệu Suất Tối Ưu Đa Nhân Rayon (CPU Multi-Core Scaling)

| Số Cores / Threads | Thời Gian (ms) | Hệ Số Tăng Tốc (Speedup $S_N$) | Hiệu Suất Đa Nhân (Efficiency $E_N$) |
| :--- | :---: | :---: | :---: |
| **1** | 57.00 | **1.00x** | 100.0% |
| **2** | 31.53 | **1.81x** | 90.4% |
| **4** | 17.20 | **3.31x** | 82.8% |
| **8** | 11.85 | **4.81x** | 60.1% |
| **12 (Max System Cores)** | 9.65 | **5.91x** | 49.2% |

## 3. Khả Năng Chịu Tải Đồng Thời (Real-World Concurrency Stress)

| Tốc Độ Stream Log Nền | Độ Trễ Khi Idle (ms) | Độ Trễ Dưới Tải Stream (ms) | Tranh Chấp Lock Overhead |
| :--- | :---: | :---: | :---: |
| **10000 logs/giây** | 9.65 | 10.66 | **+10.4%** |
| **50000 logs/giây** | 9.65 | 10.60 | **+9.9%** |

## 4. Ngân Sách Khung Hình Desktop UI (60 FPS / < 16.6ms Target)

| Số Log Mới/Frame | Thời Gian Xử Lý Tick (ms) | Tần Số Khung Hình Đạt Được | Đánh Giá Mượt Mà (60 FPS) |
| :--- | :---: | :---: | :---: |
| **100 logs/frame** | 0.24 ms | 4138.4 FPS | ✅ Đạt chuẩn 60 FPS |
| **500 logs/frame** | 0.73 ms | 1374.1 FPS | ✅ Đạt chuẩn 60 FPS |
| **2000 logs/frame** | 2.32 ms | 430.5 FPS | ✅ Đạt chuẩn 60 FPS |
| **5000 logs/frame** | 6.08 ms | 164.4 FPS | ✅ Đạt chuẩn 60 FPS |

## 5. Tốc Độ Đọc File End-to-End Từ Ổ Cứng (Disk -> Parse -> Engine)

| Số Dòng Log | Dung Lượng File (MB) | Thời Gian Đọc & Parse (ms) | Throughput I/O (MB/s) | Tốc Độ Nạp (Logs/s) |
| :--- | :---: | :---: | :---: | :---: |
| **50000 dòng** | 6.95 MB | 157.64 ms | **44.07 MB/s** | **317174 logs/s** |
| **100000 dòng** | 13.91 MB | 284.42 ms | **48.89 MB/s** | **351597 logs/s** |

## 6. Mức Độ Tiêu Thụ Tài Nguyên Tổng Thể

- **Peak Process CPU**: 200.0%
- **Average Query CPU**: 57.1%
- **Bộ nhớ RAM tiêu thụ / log**: 924.05 bytes/log
