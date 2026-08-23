# Tài Liệu Kiến Trúc & Lộ Trình Phát Triển `uwu-log` (`archi.md`)

`uwu-log` là một công cụ xem và phân tích log thời gian thực (Real-time Log Viewer & Processor) siêu nhanh, hỗ trợ đa nền tảng (Windows, Linux, WSL, macOS) được thiết kế theo kiến trúc phân tầng chuẩn hóa (Layered Architecture) và động cơ lọc song song (Parallel Filtering Engine).

---

## 1. Triết Lý Kiến Trúc & Luồng Hoạt Động Cốt Lõi (Core Platform Architecture)

Kiến trúc tổng thể của `uwu-log` được xây dựng dựa trên nguyên tắc **Trích xuất Đa Nền Tảng ➔ Trừu Tượng Hóa Nguồn ➔ Chuẩn Hóa Dữ Liệu ➔ Động Cơ Lọc Song Song & Tăng Tiến**. Tất cả các nguồn log hiện tại lẫn các tính năng mở rộng trong tương lai **bắt buộc phải tuân theo luồng kiến trúc 4 tầng chuẩn này**:

```text
                  Log Viewer (uwu-gui / uwu-tui)
                              │
                       ┌──────┴──────┐
                       │ Log Source  │  (LogSource Trait Abstraction)
                       └──────┬──────┘
                              │
        ┌─────────────────────┼─────────────────────┬──────────────────┐
        ▼                     ▼                     ▼                  ▼
     Linux                   WSL                 Windows       (Mở rộng tương lai)
        │                     │                     │                  │
    journald              journald*          Event / Process     gRPC/Docker/K8s
        │                     │                     │                  │
        └─────────────────────┼─────────────────────┴──────────────────┘
                              ▼
                         Normalizer         (uwu-normalizer)
                              │
                              ▼
                          LogEvent          (uwu-schema chuẩn hóa)
                              │
                              ▼
                      Core System Engine    (uwu-core Storage RingBuffer & Rayon Engine)
```

---

### 1.1 Nguyên Tắc Trích Xuất Key Tự Động (Dynamic Key-Value Extraction / Zero-Rigid Schema)

`uwu-log` **không ép buộc bất kỳ schema cố định nào**. Mỗi ngôn ngữ (Go, Python, Node.js, Rust, C#) hoặc ứng dụng đều có kiểu khai báo key và cấu trúc log riêng. Động cơ `Normalizer` hoạt động theo nguyên tắc:

1. **Fast-Check & Trích xuất tự động 100% Key-Value**: Kiểm tra nhanh `{` / `[` để parse JSON (bao gồm cả trường của Systemd Journald), trích xuất động toàn bộ trường vào `fields: HashMap<String, serde_json::Value>`.
2. **Lọc Động Không Schema (Dynamic Field Query)**: Động cơ `uwu-core` cho phép người dùng gõ lệnh lọc theo **BẤT KỲ KEY NÀO** xuất hiện trong log (ví dụ: `user_id:user_123`, `latency > 500`, `status:500`, `region:"us-west-1"`) mà không cần định nghĩa trước schema.
3. **Giữ nguyên log thô (Raw Retention)**: Nội dung chuỗi log thô và mã màu ANSI luôn được bảo toàn nguyên vẹn để hiển thị sắc nét trên GUI / TUI.

```mermaid
flowchart TD
    Viewer["1. Log Viewer App and UI Layer (uwu-gui / uwu-tui)"]
    Viewer --> LogSourceTrait["2. Log Source Trait Abstraction (uwu-sources)"]
    
    subgraph Drivers["3. Cross-Platform Native Drivers Layer"]
        LinuxDriver["Linux Driver (systemd-journald / Syslog / File Tailer)"]
        WSLDriver["WSL Driver (journald* / WSL2 Stream / File Tailer)"]
        WinDriver["Windows Driver (WinEvent Live Subscribe / Process Runner + JobObject RAII)"]
        FutureDriver["Future Expansion Drivers (gRPC Agent / Docker / K8s / Cloud)"]
    end
    
    LogSourceTrait --> LinuxDriver
    LogSourceTrait --> WSLDriver
    LogSourceTrait --> WinDriver
    LogSourceTrait --> FutureDriver
    
    LinuxDriver --> Normalizer["4. Log Normalizer Engine (uwu-normalizer)<br/>(JSON Fast-Check / Zero-Alloc Level / PlainText Timestamp)"]
    WSLDriver --> Normalizer
    WinDriver --> Normalizer
    FutureDriver --> Normalizer
    
    Normalizer --> LogEventSchema["5. Unified LogEvent Schema (uwu-schema)<br/>(id, timestamp, level, source_id, message, fields, raw)"]
    
    LogEventSchema --> CoreEngine["6. uwu-core SystemEngine<br/>(Batch Ingestion RingBuffer + Index-only Rayon Search + Incremental Filtering)"]
```

---

### 1.2 Mô Hình Concurrency & Multi-Threading (Thread Model)

Mô hình xử lý bất đồng bộ phối hợp giữa **Tokio Async Runtime** (cho I/O thu thập log đa luồng) và **Rayon Parallel ThreadPool** (cho động cơ lọc log song song):

```mermaid
sequenceDiagram
    autonumber
    actor Driver as OS Native Driver (File / Process / WinEvent / Journald)
    participant TokioRx as Batch Ingestion Task (Tokio)
    participant Normalizer as Log Normalizer (uwu-normalizer)
    participant RingBuf as Storage RingBuffer (VecDeque + State Lock)
    participant RayonEngine as Core Rayon Engine (Parallel Index Collect)
    participant UIThread as UI Render Loop (Desktop GUI / Ratatui TUI)

    Driver->>TokioRx: Gửi RawLogEntry qua async channel (mpsc)
    Note over TokioRx: Non-blocking drain gom batch (tối đa 512 log)
    TokioRx->>Normalizer: Chuẩn hóa log & trích xuất timestamp ngoài lock
    Normalizer-->>TokioRx: Trả về batch Vec<LogEvent>
    TokioRx->>RingBuf: Acquire write lock 1 lần cho cả batch (extend + drain overflow)
    TokioRx->>RingBuf: total_processed.fetch_add(batch_len, Release) trong lock

    loop UI Frame Loop (Mỗi frame render / tick 150-200ms)
        UIThread->>RingBuf: Đọc total_processed (Acquire)
        alt Query thay đổi (Người dùng gõ tìm kiếm mới)
            UIThread->>RayonEngine: Chạy search_with_count(query, limit)
            RayonEngine->>RayonEngine: Rayon par_iter thu thập Vec<usize> (Index-only)
            RayonEngine-->>UIThread: Chỉ clone đúng số lượng limit log mới nhất
            UIThread->>UIThread: Cập nhật cached_logs và hiển thị
        else is_auto_scroll == true và có log mới (Live Tail)
            UIThread->>RingBuf: Gọi filter_incremental(query, last_processed)
            RingBuf-->>UIThread: Trả về chỉ các log mới khớp bộ lọc
            UIThread->>UIThread: Append vào cached_logs và cuộn xuống đáy
        else is_auto_scroll == false (Frozen View / Pause)
            UIThread->>UIThread: Đóng băng cached_logs 100% (Không chạy filter)
        end
        UIThread->>UIThread: Render GUI (egui) / TUI (ratatui)
    end
```

---

### 1.3 Sơ Đồ Trạng Thái Giao Diện (UI State Machine)

Sự chuyển đổi linh hoạt giữa chế độ **Live Tail** (cuộn tự động theo log mới) và **Frozen View** (đóng băng giao diện khi cuộn đọc log cũ hoặc click vào hàng log):

```mermaid
stateDiagram-v2
    [*] --> LiveTailMode: Khởi động Ứng dụng (Mặc định)

    state LiveTailMode {
        [*] --> ScrollBottom
        ScrollBottom: Dòng log mới liên tục nạp qua filter_incremental
        ScrollBottom: Tự động cuộn theo log mới nhất ở đáy bảng
    }

    state FrozenViewMode {
        [*] --> LockView
        LockView: cached_logs hoàn toàn ĐÓNG BẰNG
        LockView: Không re-search khi log mới đổ về
        LockView: Vị trí dòng log đang chọn đứng yên 100%
    }

    LiveTailMode --> FrozenViewMode: Cuộn chuột lên / Bấm chọn dòng / Phím 'Up' / 'PageUp'
    FrozenViewMode --> LiveTailMode: Bấm nút 'Latch to Bottom' / Phím 'End' / 'G' / Phím Space (TUI)
    FrozenViewMode --> FrozenViewMode: Di chuyển xem ngữ cảnh (Unfiltered Context View)
```

---

## 2. Chi Tiết Phân Tầng Các Module Hiện Tại (Current Component Architecture)

Hệ thống được tổ chức dạng **Cargo Workspace** gồm 3 crates chuẩn Idiomatic Rust:

| Crate | Đường Dẫn | Vai Trò & Các Tối Ưu Cốt Lõi |
| :--- | :--- | :--- |
| **`uwu-core`** | [`crates/core`](file:///D:/Learn/Go/uwulog-rust/crates/core) | **Thư Viện Logic Cốt Lõi**:<br/>• `schema`: Định nghĩa `LogLevel` (Unknown=0), `LogEvent`, `RawLogEntry`.<br/>• `sources`: Trừu tượng hóa driver `FileSource` (log rotation / truncate seek), `ProcessSource` (RAII JobObject & pipe drain), `WinEventSource` (`EvtSubscribe` live stream), `JournaldSource` (stderr drain & systemd field mapping).<br/>• `normalizer`: Fast-check JSON, trích xuất timestamp plain text, zero-allocation level detection.<br/>• `filter`: Động cơ lọc AST với Rayon song song.<br/>• `engine`: `SystemEngine` quản lý Batch Ingestion, RingBuffer `VecDeque`, Atomic Sequence và Incremental Filtering không race-condition. |
| **`uwu-gui`** | [`crates/gui`](file:///D:/Learn/Go/uwulog-rust/crates/gui) | **Tầng Giao Diện Desktop Native**:<br/>Ứng dụng Desktop UI (`eframe` / `egui`) hỗ trợ Auto-scroll, Incremental Live Filtering, Dynamic Key Discovery, Unfiltered Snapshot Context, Autocomplete, History, DWM Dark Title Bar. |
| **`uwu-tui`** | [`crates/tui`](file:///D:/Learn/Go/uwulog-rust/crates/tui) | **Tầng Giao Diện Terminal UI**:<br/>Ứng dụng Terminal UI (`ratatui`) hỗ trợ Live Tail, Frozen View, ô tìm kiếm cú pháp đầy đủ và phím tắt điều hướng nhanh. |

---

## 3. Nguyên Tắc Mở Rộng Tính Năng Tương Lai (Future Architecture Expansion)

Mọi tính năng mở rộng trong tương lai **bắt buộc phải tuân theo giao thức phân tầng**:

```text
       ┌────────────────────────────────────────────────────────┐
       │             Future Expansion Source Drivers            │
       └──────────────────────────┬─────────────────────────────┘
                                  │
       ┌──────────────────────────┴─────────────────────────────┐
       ▼                                                        ▼
[Remote gRPC / Protobuf]                             [Docker Container API]
       │                                                        │
[CloudWatch / Loki Agent]                            [Kubernetes Pod Stream]
       │                                                        │
       └──────────────────────────┬─────────────────────────────┘
                                  ▼
                     LogSource Trait Abstraction
                                  │
                                  ▼
                         uwu-normalizer
                                  │
                                  ▼
                         Unified LogEvent
                                  │
                                  ▼
                         uwu-core Storage
```

### 🚀 Lộ Trình Phát Triển Chi Tiết:

#### 🐧 Giai Đoạn 1: Native Linux & WSL `journald` Driver
- **Trạng thái**: ✅ **ĐÃ HOÀN THÀNH (Implemented)**
- **Hiện thực**: `JournaldSource` impl `LogSource` trait, stream trực tiếp qua `journalctl -f -o json`, drain stderr chống deadlock và chuẩn hóa các trường đặc thù của systemd (`PRIORITY`, `__REALTIME_TIMESTAMP`, `MESSAGE`).

#### 💻 Giai Đoạn 2: Desktop Native GUI
- **Trạng thái**: ✅ **ĐÃ HOÀN THÀNH (Implemented)**
- **Hiện thực**: `crates/gui` xây dựng trên nền `eframe` / `egui` (Pure Rust), tích hợp trực tiếp `SystemEngine` với Live Incremental Filtering, Unfiltered View ngữ cảnh lỗi và tùy biến hiển thị cột động.

#### 📡 Giai Đoạn 3: Remote Log Agent & Distributed Streaming
- **Mục tiêu**: Hỗ trợ thu thập log từ xa qua mạng bảo mật không làm nghẽn I/O.
- **Tính năng**: `gRPC / WebSockets Collector Agent` đẩy log nén real-time từ remote server/container về `uwu-log`.
- **Tích hợp**: Cài đặt `GrpcSource` impl `LogSource` trait ➔ Đẩy `RawLogEntry` vào `uwu-normalizer`.

#### 💾 Giai Đoạn 4: Lưu Trữ Đĩa Cứng High-Performance (Columnar Persistence & Indexing)
- **Mục tiêu**: Ghi log xuống đĩa dạng nén cột (Parquet / DuckDB / mmap) và đánh chỉ mục bằng Roaring Bitmaps để truy vấn hàng chục triệu log mà không làm tràn RAM.
- **Tích hợp**: Đặt phía sau `SystemEngine` Ingestion Pipeline làm tầng lưu trữ thứ cấp (Cold Storage).

#### 📊 Giai Đoạn 5: Analytics, Log Rate Histogram & Alerting System
- **Mục tiêu**: Vẽ biểu đồ tần suất log (Log Rate Histogram) và tự động phát cảnh báo qua Telegram, Slack, Webhook khi phát hiện đột biến lỗi (`ERROR`/`CRITICAL` spike).
