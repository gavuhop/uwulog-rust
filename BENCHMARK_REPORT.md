# 📊 Báo Cáo Hiệu Năng Toàn Diện & Thực Tế (`uwulog-rust`)

- **Ngày thử nghiệm**: 2026-09-08 09:54:49
- **CPU**: 12th Gen Intel(R) Core(TM) i5-12450H (12 Logical Cores)
- **RAM**: 31.63 GB
- **Quy mô tập log in-memory**: 500000 dòng
- **Tốc độ Ingestion vào RAM**: 27495944 logs/giây
- **RAM RSS Footprint**: 436.55 MB

## 1. Phân Phối Độ Trễ & Throughput Lọc In-Memory (Chưa Warm & Đã Warm)

| STT | Kịch Bản Query | Cú Pháp | Chưa Warm (ms) | Đã Warm p50 (ms) | Đã Warm p95 (ms) | Tăng Tốc (Warm/Cold) | QPS | Throughput (Logs/s) |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | **01_empty_query** | `*(rỗng)*` | 0.86 | 0.57 | 0.95 | 1.50x | 1624.6 | 812.29M logs/s |
| 2 | **02_substring_match** | `database connection timeout` | 38.03 | 34.60 | 42.55 | 1.10x | 27.1 | 13.56M logs/s |
| 3 | **03_exact_field_match** | `level:error` | 7.11 | 7.43 | 8.41 | 0.96x | 134.8 | 67.39M logs/s |
| 4 | **04_regex_match** | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | 35.13 | 39.65 | 40.93 | 0.89x | 25.5 | 12.74M logs/s |
| 5 | **05_numeric_range** | `latency:>=500 duration:100..800` | 33.21 | 33.75 | 37.24 | 0.98x | 29.6 | 14.82M logs/s |
| 6 | **06_time_window** | `timestamp:now..15m` | 5.41 | 4.92 | 5.53 | 1.10x | 202.1 | 101.05M logs/s |
| 7 | **07_complex_ast_boolean** | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | 25.53 | 26.99 | 28.92 | 0.95x | 36.7 | 18.33M logs/s |

## 2. Hiệu Suất Tối Ưu Đa Nhân Rayon (CPU Multi-Core Scaling)

| Số Cores / Threads | Thời Gian (ms) | Hệ Số Tăng Tốc (Speedup $S_N$) | Hiệu Suất Đa Nhân (Efficiency $E_N$) |
| :--- | :---: | :---: | :---: |
| **1** | 105.21 | **1.00x** | 100.0% |
| **2** | 54.37 | **1.93x** | 96.7% |
| **4** | 26.69 | **3.94x** | 98.6% |
| **8** | 31.79 | **3.31x** | 41.4% |
| **12 (Max System Cores)** | 27.29 | **3.86x** | 32.1% |

## 3. Khả Năng Chịu Tải Đồng Thời (Real-World Concurrency Stress)

| Tốc Độ Stream Log Nền | Độ Trễ Khi Idle (ms) | Độ Trễ Dưới Tải Stream (ms) | Tranh Chấp Lock Overhead |
| :--- | :---: | :---: | :---: |
| **10000 logs/giây** | 27.29 | 27.66 | **+1.4%** |
| **50000 logs/giây** | 27.29 | 29.29 | **+7.3%** |

## 4. Ngân Sách Khung Hình Desktop UI (60 FPS / < 16.6ms Target)

| Số Log Mới/Frame | Thời Gian Xử Lý Tick (ms) | Tần Số Khung Hình Đạt Được | Đánh Giá Mượt Mà (60 FPS) |
| :--- | :---: | :---: | :---: |
| **100 logs/frame** | 0.21 ms | 4708.2 FPS | ✅ Đạt chuẩn 60 FPS |
| **500 logs/frame** | 1.02 ms | 983.2 FPS | ✅ Đạt chuẩn 60 FPS |
| **2000 logs/frame** | 4.77 ms | 209.4 FPS | ✅ Đạt chuẩn 60 FPS |
| **5000 logs/frame** | 10.89 ms | 91.8 FPS | ✅ Đạt chuẩn 60 FPS |

## 5. Tốc Độ Đọc File End-to-End Từ Ổ Cứng (Disk -> Parse -> Engine)

| Số Dòng Log | Dung Lượng File (MB) | Thời Gian Đọc & Parse (ms) | Throughput I/O (MB/s) | Tốc Độ Nạp (Logs/s) |
| :--- | :---: | :---: | :---: | :---: |
| **50000 dòng** | 6.95 MB | 258.12 ms | **26.91 MB/s** | **193710 logs/s** |
| **100000 dòng** | 13.91 MB | 494.89 ms | **28.10 MB/s** | **202066 logs/s** |

## 6. Mức Độ Tiêu Thụ Tài Nguyên Tổng Thể

- **Peak Process CPU**: 276.9%
- **Average Query CPU**: 116.7%
- **Bộ nhớ RAM tiêu thụ / log**: 915.51 bytes/log
