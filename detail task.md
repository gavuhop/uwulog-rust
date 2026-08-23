# Kế Hoạch Chi Tiết Các Nhiệm Vụ Cần Thực Hiện (`detail task.md`)

> **Ghi chú về định hướng thiết kế:**
> Một số cơ chế hiện tại đã được thiết kế có chủ đích để phục vụ trải nghiệm người dùng đặc thù:
> - **Data-driven `now` (`max_timestamp`):** Tính toán mốc thời gian tương đối dựa trên log timestamp lớn nhất thay vì đồng hồ máy tính (cho phép query `now..10m` chính xác trên cả log offline/replay).
> - **`Singleline TextEdit` trên từng Table Cell:** Cho phép người dùng bôi đen từng cụm từ để click chuột phải chọn *Filter by selection* / *Highlight term*.
> - **Frozen Snapshot khi mở Unfiltered Context (`target_id`):** Đóng băng khung nhìn quanh log lỗi mục tiêu để người dùng phân tích ngữ cảnh mà không bị log mới làm trôi màn hình.
> - **`WeakSender` + Windows Job Object trong `ProcessSource`:** Đảm bảo khi GUI/TUI đóng channel, toàn bộ cây tiến trình con (process tree) bị hủy tức thì qua `taskkill`.
>
> Dưới đây là danh sách các nhiệm vụ cần tối ưu hóa, sửa lỗi và hoàn thiện tính năng dựa trên đánh giá kiến trúc toàn diện.

---

## 1. Core Engine & Ingestion Pipeline (`crates/core`)

- [x] **1.1. Batching Ingestion & Giảm Lock Contention trong `SystemEngine`**
  - *Hiện trạng:* Mỗi log đơn lẻ đều tranh chấp 2 `RwLock::write` (`max_timestamp` và `events`), gây nghẽn channel khi UI đang thực hiện Rayon search.
  - *Giải pháp:* Đọc batch (100 - 1,000 log) từ channel, normalize bên ngoài lock, sau đó acquire `events.write()` 1 lần cho cả batch.
  - *File:* [`crates/core/src/engine.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/engine.rs)

- [x] **1.2. Tối ưu bộ nhớ `SystemEngine::search_with_count` (Index-only Collect)**
  - *Hiện trạng:* Rayon `.cloned()` toàn bộ hàng trăm nghìn `LogEvent` khớp bộ lọc rồi mới `.skip(skip_count)` để lấy `limit` log cho UI.
  - *Giải pháp:* Rayon chỉ đếm tổng số lượng và thu thập `Vec<usize>` (index), sau đó chỉ clone đúng số lượng `limit` phần tử cần thiết.
  - *File:* [`crates/core/src/engine.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/engine.rs)

- [x] **1.3. Sửa Race Condition trong `filter_incremental`**
  - *Hiện trạng:* `total_processed` được đọc trước khi lấy read lock `events`, khiến các log mới nạp ở giữa bị bỏ qua và mất trên UI.
  - *Giải pháp:* Đọc sequence ID hoặc đồng bộ snapshot counter bên trong read lock.
  - *File:* [`crates/core/src/engine.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/engine.rs)

- [x] **1.4. Chuẩn hóa thứ tự enum `LogLevel`**
  - *Hiện trạng:* `LogLevel::Unknown` nằm cuối enum nên `Unknown > Fatal > Error` trong phép so sánh `Ord`.
  - *Giải pháp:* Đặt `Unknown` lên vị trí đầu tiên (index = 0) hoặc implement `Ord`/`PartialOrd` thủ công để phép so sánh `level >= warn` hoạt động chuẩn xác.
  - *File:* [`crates/core/src/schema.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/schema.rs)

- [x] **1.5. Tối ưu `LogNormalizer` & Fast-check JSON**
  - *Hiện trạng:* Gọi `serde_json::from_str` trên mọi plain text log gây lãng phí CPU; double serialization trên `RawPayload::Json`.
  - *Giải pháp:* Fast-check ký tự đầu `{` / `[` trước khi parse JSON; tái sử dụng `serde_json::Value` mà không convert qua String; trích xuất timestamp cơ bản cho plain text.
  - *File:* [`crates/core/src/normalizer.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/normalizer.rs)

- [x] **1.6. Cải tiến `FileSource` & Log Rotation Support**
  - *Hiện trạng:* Khi file log bị rotate hoặc truncate (`len < offset`), tailer bị kẹt ở EOF; watcher thư mục cha gây spurious wakeups.
  - *Giải pháp:* Kiểm tra kích thước file để `seek(SeekFrom::Start(0))` khi bị truncate; lọc `event.paths` khớp chính xác file cần theo dõi.
  - *File:* [`crates/core/src/sources/file_tailer.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/sources/file_tailer.rs)

- [x] **1.7. Hoàn thiện Flush Buffer trong `ProcessSource` & Tránh rò rỉ JobObject**
  - *Hiện trạng:* `weak_tx` có thể bị drop trước khi stdout reader đọc hết các dòng cuối cùng khi tiến trình kết thúc tự nhiên; rò rỉ `JobObject` handle trên Windows.
  - *Giải pháp:* Đảm bảo stdout/stderr drain hết pipe trước khi shutdown; đóng gói `JobObject` vào RAII struct quản lý vòng đời.
  - *File:* [`crates/core/src/sources/process.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/sources/process.rs)

- [x] **1.8. Cải tiến Windows Event Source & Journald Source**
  - *Hiện trạng:* `WinEventSource` dùng `EvtQuery` tĩnh không stream được log mới; `JournaldSource` pipe stderr có nguy cơ deadlock và bỏ sót trường `MESSAGE`/`PRIORITY`.
  - *Giải pháp:* Nâng cấp subscription cho WinEvent; xử lý async stderr và map trường chữ hoa của systemd journal.
  - *File:* [`crates/core/src/sources/windows_event.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/sources/windows_event.rs), [`crates/core/src/sources/journald.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/sources/journald.rs)

---

## 2. Filter Engine & Query Evaluator (`crates/core/src/filter`)

- [x] **2.1. Zero-Allocation String Matching trong Rayon Parallel Loop**
  - *Hiện trạng:* Gọi `.to_lowercase()` cấp phát hàng triệu `String` heap cho mỗi từ khóa và mỗi dòng log trong `eval_event`.
  - *Giải pháp:* Viết hàm helper `contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool` kiểm tra trực tiếp byte không cấp phát bộ nhớ; chuyển `get_event_field_str` sang `Option<Cow<'a, str>>`.
  - *File:* [`crates/core/src/filter/evaluator.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/filter/evaluator.rs)

- [x] **2.2. Loại bỏ Hằng Số Epsilon `0.0001` Tùy Tiện**
  - *Hiện trạng:* Epsilon `0.0001` làm sai lệch so sánh số thực nhỏ (nuốt mất latency sub-millisecond `< 0.1ms`).
  - *Giải pháp:* Sử dụng so sánh số thực chuẩn IEEE-754.
  - *File:* [`crates/core/src/filter/evaluator.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/filter/evaluator.rs)

- [x] **2.3. Hoàn thiện Tokenizer: Phân biệt URL, IPv6 và Dấu `-` trong chuỗi ngoặc kép**
  - *Hiện trạng:* Chuỗi `"-500ms"` bị hiểu nhầm thành `NOT "500ms"`; URL `https://api.com` bị tách nhầm thành field `https`.
  - *Giải pháp:* Giữ nguyên dấu `-` bên trong ngoặc kép; chỉ nhận diện `field:value` khi `field` là identifier hợp lệ và không chứa `//`.
  - *File:* [`crates/core/src/filter/parser.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/filter/parser.rs)

- [x] **2.4. Hỗ trợ cú pháp `now-5m`, `now-1h` & Tối ưu Datetime Parsing**
  - *Hiện trạng:* Không parse được tiền tố `now-`; thử 16 định dạng datetime trong hot-path Rayon.
  - *Giải pháp:* Parse tường minh `now-` / `now+`; trích xuất và cache sẵn timestamp dạng giây `f64` trong `LogEvent`.
  - *File:* [`crates/core/src/filter/utils.rs`](file:///D:/Learn/Go/uwulog-rust/crates/core/src/filter/utils.rs)

---

## 3. Desktop GUI (`crates/gui`)

- [x] **3.1. Sửa Đồng Bộ `last_processed_count` cho Tab Unfiltered Live**
  - *Hiện trạng:* `last_processed_count` bị ghi đè ở nhánh Main Stream khiến nhánh Raw Stream luôn nhận `new_count = 0`.
  - *Giải pháp:* Dùng biến `prev_processed` chung cho cả hai nhánh trong `app.rs:tick()`.
  - *File:* [`crates/gui/src/app.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/app.rs)

- [x] **3.2. Dời `filter_incremental` của Log Counter ra khỏi Render Loop**
  - *Hiện trạng:* `render_log_counter` gọi filter và clone hàng nghìn log mỗi frame khi đang pause.
  - *Giải pháp:* Tính toán `paused_new_matched_count` trong `tick()` khi có log mới; UI chỉ việc đọc biến có sẵn.
  - *File:* [`crates/gui/src/ui/header.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/header.rs), [`crates/gui/src/app.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/app.rs)

- [x] **3.3. Cache Key / Field Discovery (Tránh O(N*M) mỗi frame)**
  - *Hiện trạng:* `sync_discovered_keys` và `get_available_log_fields` quét toàn bộ 50,000 log mỗi frame/mỗi phím gõ.
  - *Giải pháp:* Duy trì cache `known_keys: HashSet<String>` và `known_fields: BTreeMap<String, FieldType>`, chỉ cập nhật khi nạp log mới.
  - *File:* [`crates/gui/src/app.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/app.rs), [`crates/gui/src/ui/columns_modal.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/columns_modal.rs)

- [x] **3.4. Sửa Rung Giật (Jitter) khi Kéo Thả Cột Header**
  - *Hiện trạng:* Tráo đổi cột ngay khi di chuột qua mép cột bên cạnh gây đảo vị trí liên tục giữa các frame.
  - *Giải pháp:* Chỉ thực hiện swap vị trí cột khi người dùng thả chuột (`drop`).
  - *File:* [`crates/gui/src/ui/table/header.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/table/header.rs), [`crates/gui/src/ui/table/mod.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/table/mod.rs)

- [x] **3.5. Xử lý Click-Outside cho Autocomplete & Search History Popup**
  - *Hiện trạng:* Popup nổi lơ lửng đè lên bảng khi click ra ngoài mà không tự đóng.
  - *Giải pháp:* Kiểm tra pointer click bên ngoài vùng popup để tự động đóng dropdown.
  - *File:* [`crates/gui/src/ui/autocomplete.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/autocomplete.rs), [`crates/gui/src/ui/history.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/history.rs)

- [x] **3.6. ANSI-Aware Matcher & Tô Màu ANSI 500 Dòng Hiển Thị**
  - *Yêu cầu:* Phân tích màu ANSI cho tối đa 500 dòng hiển thị trên bảng; loại bỏ mã ANSI khi copy/filter.
  - *File:* [`crates/gui/src/ui/table/cell.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/table/cell.rs), [`crates/gui/src/ui/theme.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/theme.rs)

- [x] **3.7. Dọn dẹp RAM khi đóng Tab Unfiltered bằng phím Escape**
  - *Hiện trạng:* Ấn Escape chỉ chuyển tab mà không gọi `close_unfiltered_stream()`, làm 500 log vẫn kẹt trong RAM.
  - *File:* [`crates/gui/src/ui/mod.rs`](file:///D:/Learn/Go/uwulog-rust/crates/gui/src/ui/mod.rs)

---

## 4. Terminal TUI (`crates/tui`)

- [ ] **4.1. Đăng ký Panic Hook Khôi Phục Terminal Raw Mode**
  - *Hiện trạng:* Nếu ứng dụng panic/crash, terminal bị kẹt ở Raw Mode và Alternate Screen (mất echo, kẹt phím).
  - *Giải pháp:* Đăng ký `std::panic::set_hook` gọi `disable_raw_mode` và `LeaveAlternateScreen`.
  - *File:* [`crates/tui/src/main.rs`](file:///D:/Learn/Go/uwulog-rust/crates/tui/src/main.rs)

- [ ] **4.2. Xử lý Giữ Phím (`KeyEventKind::Repeat`) & Bắt sự kiện `Event::Resize`**
  - *Hiện trạng:* Giữ phím cuộn/xóa trên Windows bị nuốt phím; resize cửa sổ không kích hoạt redraw.
  - *Giải pháp:* Chấp nhận cả `KeyEventKind::Repeat`; đặt `should_redraw = true` khi nhận `Event::Resize`.
  - *File:* [`crates/tui/src/main.rs`](file:///D:/Learn/Go/uwulog-rust/crates/tui/src/main.rs)

- [ ] **4.3. Sửa Lỗi Parse CLI Args `-r "command args"`**
  - *Hiện trạng:* Lệnh `-r "go run gen_logs.go" -n 1000` bị parse nhầm thành tên binary `"go run gen_logs.go"` khiến source chết im lặng.
  - *Giải pháp:* Sử dụng `shlex` hoặc tách lệnh tôn trọng dấu ngoặc kép.
  - *File:* [`crates/tui/src/main.rs`](file:///D:/Learn/Go/uwulog-rust/crates/tui/src/main.rs)

- [ ] **4.4. Hỗ trợ Thoát Bằng `Ctrl+C` & Hiển Thị Con Trỏ Trong Ô Tìm Kiếm**
  - *Hiện trạng:* `Ctrl+C` không thoát; ô nhập liệu không hiển thị con trỏ nhấp nháy.
  - *File:* [`crates/tui/src/app.rs`](file:///D:/Learn/Go/uwulog-rust/crates/tui/src/app.rs), [`crates/tui/src/ui/input.rs`](file:///D:/Learn/Go/uwulog-rust/crates/tui/src/ui/input.rs)