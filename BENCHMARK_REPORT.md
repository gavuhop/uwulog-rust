# 📊 Báo Cáo Hiệu Năng Toàn Diện & Thực Tế (`uwulog-rust`)

- **Ngày thử nghiệm**: 2026-09-11 23:51:23
- **CPU**: 12th Gen Intel(R) Core(TM) i5-12450H (12 Logical Cores)
- **RAM**: 31.63 GB
- **Quy mô tập log in-memory**: 500000 dòng
- **Tốc độ Ingestion vào RAM**: 39152428 logs/giây
- **RAM RSS Footprint**: 380.19 MB

## 1. Phân Phối Độ Trễ & Throughput Lọc In-Memory (Chưa Warm & Đã Warm)

| STT | Kịch Bản Query | Cú Pháp | Chưa Warm (ms) | Đã Warm p50 (ms) | Đã Warm p95 (ms) | Tăng Tốc (Warm/Cold) | QPS | Throughput (Logs/s) |
| :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | **01_empty_query** | `*(rỗng)*` | 1.51 | 0.52 | 1.20 | 2.87x | 1578.4 | 789.22M logs/s |
| 2 | **02_substring_match** | `database connection timeout` | 26.20 | 29.16 | 34.91 | 0.90x | 33.5 | 16.73M logs/s |
| 3 | **03_exact_field_match** | `level:error` | 6.87 | 7.64 | 18.46 | 0.90x | 109.4 | 54.71M logs/s |
| 4 | **04_regex_match** | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | 24.71 | 29.70 | 49.43 | 0.83x | 31.6 | 15.82M logs/s |
| 5 | **05_numeric_range** | `latency:>=500 duration:100..800` | 21.76 | 21.82 | 26.95 | 1.00x | 44.0 | 22.02M logs/s |
| 6 | **06_time_window** | `timestamp:now..15m` | 5.93 | 5.27 | 6.56 | 1.13x | 184.9 | 92.47M logs/s |
| 7 | **07_complex_ast_boolean** | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | 29.81 | 31.35 | 51.68 | 0.95x | 29.1 | 14.56M logs/s |

## 2. Hiệu Suất Tối Ưu Đa Nhân Rayon (CPU Multi-Core Scaling)

| Số Cores / Threads | Thời Gian (ms) | Hệ Số Tăng Tốc (Speedup $S_N$) | Hiệu Suất Đa Nhân (Efficiency $E_N$) |
| :--- | :---: | :---: | :---: |
| **1** | 106.81 | **1.00x** | 100.0% |
| **2** | 53.73 | **1.99x** | 99.4% |
| **4** | 31.76 | **3.36x** | 84.1% |
| **8** | 33.43 | **3.19x** | 39.9% |
| **12 (Max System Cores)** | 30.39 | **3.51x** | 29.3% |

## 3. Khả Năng Chịu Tải Đồng Thời (Real-World Concurrency Stress)

| Tốc Độ Stream Log Nền | Độ Trễ Khi Idle (ms) | Độ Trễ Dưới Tải Stream (ms) | Tranh Chấp Lock Overhead |
| :--- | :---: | :---: | :---: |
| **10000 logs/giây** | 30.39 | 32.73 | **+7.7%** |
| **50000 logs/giây** | 30.39 | 35.71 | **+17.5%** |

## 4. Ngân Sách Khung Hình Desktop UI (60 FPS / < 16.6ms Target)

| Số Log Mới/Frame | Thời Gian Xử Lý Tick (ms) | Tần Số Khung Hình Đạt Được | Đánh Giá Mượt Mà (60 FPS) |
| :--- | :---: | :---: | :---: |
| **100 logs/frame** | 0.30 ms | 3333.8 FPS | ✅ Đạt chuẩn 60 FPS |
| **500 logs/frame** | 1.28 ms | 781.7 FPS | ✅ Đạt chuẩn 60 FPS |
| **2000 logs/frame** | 3.96 ms | 252.7 FPS | ✅ Đạt chuẩn 60 FPS |
| **5000 logs/frame** | 10.91 ms | 91.6 FPS | ✅ Đạt chuẩn 60 FPS |

## 5. Tốc Độ Đọc File End-to-End Từ Ổ Cứng (Disk -> Parse -> Engine)

| Số Dòng Log | Dung Lượng File (MB) | Thời Gian Đọc & Parse (ms) | Throughput I/O (MB/s) | Tốc Độ Nạp (Logs/s) |
| :--- | :---: | :---: | :---: | :---: |
| **50000 dòng** | 6.95 MB | 272.96 ms | **25.45 MB/s** | **183179 logs/s** |
| **100000 dòng** | 13.91 MB | 500.29 ms | **27.79 MB/s** | **199884 logs/s** |

## 6. Mức Độ Tiêu Thụ Tài Nguyên Tổng Thể

- **Peak Process CPU**: 400.0%
- **Average Query CPU**: 130.6%
- **Bộ nhớ RAM tiêu thụ / log**: 797.32 bytes/log
