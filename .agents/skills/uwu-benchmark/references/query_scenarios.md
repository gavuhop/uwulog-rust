# 7 Kịch Bản Truy Vấn Chuẩn Hóa (Benchmark Query Scenarios)

Hệ thống Benchmark của `uwulog-rust` đo kiểm hiệu năng dựa trên 7 kịch bản truy vấn đại diện cho các mức độ phức tạp từ đơn giản đến nâng cao:

| STT | Kịch Bản Query | Cú Pháp Minh Họa | Mô Tả Kỹ Thuật | Đặc Điểm Xử Lý |
| :--- | :--- | :--- | :--- | :--- |
| **01** | `01_empty_query` | `*(rỗng)*` | Fast-path in-memory RingBuffer retrieval | Lấy trực tiếp từ RingBuffer mà không kích hoạt AST Parser và Rayon filtering |
| **02** | `02_substring_match` | `database connection timeout` | Full-text substring search | Quét toàn văn (Case-insensitive) trên toàn bộ bản ghi log (message, level, timestamp, fields) |
| **03** | `03_exact_field_match` | `level:error` | Exact matching trên Canonical Field | So khớp trực tiếp enum `LogLevel` với chi phí O(1) byte comparison |
| **04** | `04_regex_match` | `tag:~"^sys\.[a-z]+" message:~"user_\d{4}"` | Multi-field Regular Expression matching | Biên dịch và so khớp Regex đa luồng trên nhiều trường |
| **05** | `05_numeric_range` | `latency:>=500 duration:100..800` | So sánh số học và khoảng giá trị | So khớp số thực `f64` (hoặc trường `id: u64`), không cần parse lại chuỗi |
| **06** | `06_time_window` | `timestamp:now..15m` | Cửa sổ thời gian tương đối động | Sử dụng `timestamp_secs` (pre-computed epoch) để tính toán khoảng thời gian |
| **07** | `07_complex_ast_boolean` | `(level:error OR level:warn) AND tag:auth AND latency:>200 -source:heartbeat` | Cây AST Boolean lồng nhau phức tạp | Đánh giá cây biểu thức lồng nhau gồm `AND`, `OR`, nhóm ngoặc và phủ định (`NOT`) |
