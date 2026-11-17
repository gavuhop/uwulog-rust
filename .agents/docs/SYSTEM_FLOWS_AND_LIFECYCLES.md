# Luồng Dữ liệu & Vòng đời Hệ thống (`SYSTEM_FLOWS_AND_LIFECYCLES.md`)

Tài liệu này mô tả 4 luồng vận hành cốt lõi của `uwulog-rust` kèm các điểm chạm mã nguồn chính xác.

---

## 1. Flow A — Luồng Nạp Log & Dual-Trigger Arrow Flush

Nguồn log bất kỳ (`FileSource`, `ProcessSource`, `WslSource`, `RemoteSource`) đều đẩy `RawLogEntry` qua một đường ống bất đồng bộ trước khi vào `ArrowStorage`:

```mermaid
flowchart LR
    Src["LogSource\n(File / Process / WSL / Remote)"] -->|"RawLogEntry"| Ch["tokio::sync::mpsc\n(capacity = 10_000)"]
    Ch -->|"Drain <= 512 items\n(non-blocking)"| Norm["LogNormalizer::normalize\n+ Adaptive Timestamp\n+ Schema Diff (Outside Lock)"]
    Norm -->|"Vec<LogEvent>"| Builder["ArrowStorage::push_events\n(ActiveRecordBatchBuilder)"]
    Builder -->|"Trigger 1: rows >= TARGET_BATCH_SIZE (4096)\nTrigger 2: raw_rx.is_empty() -> flush()"| Sealed["Sealed RecordBatch Chunks\n(FIFO Capacity Eviction)"]
```

### Chi tiết thực thi
1. **Channel Drain ([crates/core_engine/src/lib.rs](../../crates/core_engine/src/lib.rs))**: Task nền của `SystemEngine::new` chờ phần tử đầu tiên bằng `raw_rx.recv().await`, sau đó rút không chặn tối đa `512` phần tử (`try_recv()`).
2. **Xử lý Ngoài Khóa Lưu trữ**:
   - `LogNormalizer::normalize` ([normalizer.rs](../../crates/driver_sources/src/normalizer.rs)) bỏ mã màu ANSI qua `strip_ansi` (`Cow<'_, str>`), parse JSON nếu chuỗi bắt đầu/kết thúc bằng `{}`/`[]`, làm phẳng object lồng nhau bằng dấu `.`, và trích `timestamp`/`level`/`message` trong 1 lượt duyệt qua `StandardField::from_alias`.
   - `SystemEngine` áp dụng **Adaptive Timestamp** (`parse_with_format` fast-path, tự dò lại bằng `detect_timestamp_format` khi đổi định dạng) và **Change-Driven Schema Discovery** (chỉ xin write lock `schema` / `known_keys` khi có key mới hoặc promote `Text -> Number`).
3. **Dual-Trigger Flush ([storage.rs](../../crates/core_engine/src/arrow_storage/storage.rs))**:
   - Đẩy batch vào `ActiveRecordBatchBuilder` (gộp nếu tổng dòng $\le$ `TARGET_BATCH_SIZE = 4096`).
   - Khi `raw_rx.is_empty()`, gọi ngay `storage_clone.flush()` để đóng gói batch đang mở thành `RecordBatch` bất biến, đảm bảo log đến thưa vẫn hiển thị tức thì trên UI.

---

## 2. Flow B — Luồng Biên dịch Truy vấn Vector hóa & 4 Chế độ Viewport

```mermaid
sequenceDiagram
    participant UI as UI Thread (egui)
    participant Worker as Rayon / Tokio Worker
    participant Eng as SystemEngine
    participant Store as ArrowStorage + QueryCompiler

    alt Chế độ 1: User đổi Query / Search mới
        UI->>Worker: spawn_search()
        Worker->>Eng: search_with_count(query, limit)
        Eng->>Store: Parse Expr + Rayon par_iter trên RecordBatches
        Store-->>UI: AppEvent::SearchResultsReady
    else Chế độ 2: Live Tail (is_auto_scroll == true)
        UI->>Eng: Đọc AtomicU64 total_processed()
        opt total_processed > last_processed
            UI->>Worker: spawn_incremental_filter()
            Worker->>Eng: filter_incremental(query, last_processed)
            Store-->>UI: AppEvent::IncrementalLogsReady
        end
    else Chế độ 3: Cuộn ngược lên đỉnh (Reverse Pagination)
        UI->>Worker: spawn_reverse_pagination(oldest_id)
        Worker->>Eng: search_before(query, oldest_id, page_size)
        Store-->>UI: AppEvent::ReversePaginationReady
    else Chế độ 4: Xem ngữ cảnh nguyên bản quanh lỗi
        UI->>Eng: get_unfiltered_events(Some(center_id), limit)
        Note over Store: Trả về cửa sổ đối xứng quanh target_id không áp filter
    end
```

---

## 3. Flow C — Luồng Điều phối Giao diện Một Chiều (`ui_gui`)

```mermaid
flowchart LR
    Key["KeymapManager\n(4-Layer Context)"] -->|"KeyAction -> KeyActionExt"| Act["AppAction\n(actions.rs)"]
    View["egui Views & Modals"] -->|"User Interaction"| Act
    Act -->|"UwuGuiApp::dispatch_action\n(app/mod.rs)"| Branch{"Đồng bộ hay\nBất đồng bộ?"}
    Branch -->|"Đồng bộ"| Mut["Cập nhật OverlayManager /\nWorkspaceManager / GuiSession"]
    Branch -->|"Bất đồng bộ (Tokio/Rayon)"| Bg["Worker Thread\n(SystemEngine / Transport)"]
    Bg -->|"event_tx.send(AppEvent)"| Red["reducer::reduce\n(reducer.rs - O(1))"]
    Red --> Mut
    Mut --> Render["Frame tiếp theo (< 16.6ms)"]
```

- **Ngữ cảnh phím tắt (`resolve_active_key_context`)**: `OverlayLayer::RemoteServersModal` $\rightarrow$ `RemoteServers`; các modal/picker khác $\rightarrow$ `Modal`; khi `autocomplete.is_open` $\rightarrow$ `Autocomplete`; focus ô tìm kiếm $\rightarrow$ `SearchInput`; mặc định $\rightarrow$ `Table` (kế thừa từ `Global`).

---

## 4. Flow D — Luồng Bắt tay & Truyền tải Remote Agent (`uwu-agent`)

```mermaid
sequenceDiagram
    participant Host as Local Client (RemoteSource)
    participant Trn as SshTransport / WslTransport
    participant Agent as Remote `uwu-agent proxy`
    participant Src as Remote FileSource / ProcessSource

    Host->>Trn: spawn_proxy()
    Trn->>Agent: Khởi chạy `uwu-agent proxy` qua stdio pipe
    Note over Agent,Trn: Bỏ qua SSH MOTD / Banner / .bashrc noise
    Agent->>Host: stdout.write_all(b"__UWU_AGENT_READY__\n")
    Host->>Host: wait_for_ready_marker() (timeout 15s)
    Note over Host,Agent: Chuyển sang khung nhị phân 4-byte Big-Endian (MAX_FRAME_SIZE = 64 MB)
    Host->>Agent: ClientEnvelope::StartStream { spec, filter_pushdown }
    Agent->>Src: start_stream(mpsc(10_000))
    loop Stream + Heartbeat
        Host->>Agent: ClientEnvelope::Ping { seq } (mỗi 5s)
        Agent-->>Host: ServerEnvelope::Pong { seq }
        Src-->>Agent: RawLogEntry (batch <= 256 hoặc tick 50ms)
        Agent-->>Host: ServerEnvelope::LogBatch(Vec<RawLogEntry>)
    end
    Host->>Agent: ClientEnvelope::StopStream
```

- **Quy tắc tách kênh**: Ở chế độ `proxy`, `uwu-agent` chỉ ghi `AGENT_READY_MARKER` và các frame nhị phân ra `stdout`; mọi diagnostic/panic trace đều ghi ra `stderr`.
