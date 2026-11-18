# Lõi Apache Arrow & Ngôn ngữ Truy vấn Filter DSL (`ARROW_ENGINE_AND_QUERY_DSL.md`)

Tài liệu này mô tả chi tiết cấu trúc lưu trữ cột trong `uwu-core-engine` và ngữ pháp/cơ chế biên dịch truy vấn giữa `uwu-core-filter` với `QueryCompiler`.

---

## 1. Kiến trúc Lưu trữ `ArrowStorage` (`crates/core_engine/src/arrow_storage/`)

| Module | Thành phần chính | Vai trò |
| :--- | :--- | :--- |
| [mod.rs](../../crates/core_engine/src/arrow_storage/mod.rs) | Re-exports | Xuất `ActiveRecordBatchBuilder`, `ColumnView`, `QueryCompiler`, `ArrowStorage`, `BatchSliceIterator`. |
| [builder.rs](../../crates/core_engine/src/arrow_storage/builder.rs) | `ActiveRecordBatchBuilder`, `DynamicColumnBuilder` | Thu thập log vào 5 cột hệ thống (`__id`, `__color`, `__timestamp`, `__timestamp_secs`, `__message`), cột dữ liệu user (`f:<key>`, kèm metadata `uwu.json="1"` cho array/object thuần), và cột đánh dấu null (`n:<key>`). |
| [storage.rs](../../crates/core_engine/src/arrow_storage/storage.rs) | `ArrowStorage`, `BatchColumns`, `BatchSliceIterator` | Quản lý `sealed_batches: RwLock<Arc<Vec<RecordBatch>>>`, gộp batch nhỏ qua `unify_and_concat` (`TARGET_BATCH_SIZE = 4096`), và FIFO eviction theo `max_capacity`. |
| [compiler.rs](../../crates/core_engine/src/arrow_storage/compiler.rs) | `QueryCompiler`, `ColumnView` | Biên dịch cây `Expr` thành `BooleanArray` mask có short-circuit và quét song song qua Rayon `par_iter`. |

### 1.1. Thăng cấp Kiểu Dữ liệu (Type Promotion)
Trong hệ thống tồn tại hai lớp nhận diện kiểu dữ liệu bổ trợ cho nhau:
1. **`DynamicColumnBuilder` (Arrow Storage, [builder.rs](../../crates/core_engine/src/arrow_storage/builder.rs#L10-L135))**:
   - Bắt đầu ở kiểu hẹp nhất của giá trị khác-null đầu tiên: `Boolean`, `Int64` $\rightarrow$ nếu gặp số thực thì thăng cấp lên `Float64` $\rightarrow$ nếu gặp chuỗi hoặc giá trị hỗn hợp thì thăng cấp sang `String` (`Utf8`), đồng thời bảo toàn toàn bộ giá trị cũ đã nạp. Giá trị `null` tường minh được ghi vào cột `n:<key>` (1 bit/dòng) để không ép cột số/boolean thăng cấp sang `String`.
2. **`SystemEngine` Schema Registry ([crates/core_engine/src/lib.rs](../../crates/core_engine/src/lib.rs#L114-L158))**:
   - Lưu `HashMap<String, FieldType>` (`Time`, `Number`, `Text`) phục vụ UI autocomplete và gợi ý toán tử; hỗ trợ thăng cấp `Text -> Number` khi gặp trường số mới.

### 1.2. Zero-Lookup `BatchColumns` & Hợp nhất Batch
- Khi đọc hoặc materialize `LogEvent`, `BatchColumns::extract(&batch)` tra cứu chỉ mục của 5 cột `__*`, các cột user `f:*`, và các cột `n:*` tương ứng **một lần duy nhất cho mỗi `RecordBatch`**.
- `unify_and_concat(a, b)` hợp nhất các `RecordBatch` liền kề nếu tổng dòng $\le$ `4096` và không xung đột kiểu dữ liệu (tự điền `null` cho các cột động vắng mặt, chỉ giữ `uwu.json="1"` khi cả hai cùng thuần JSON, và sắp xếp `__*` $\rightarrow$ `f:*` $\rightarrow$ `n:*`).

---

## 2. Đặc tả Filter DSL (`crates/core_filter/src/parser.rs`)

### 2.1. Ngữ pháp Tổng quát
- **Độ ưu tiên toán tử** (từ cao xuống thấp):
  1. Nhóm ngoặc `(...)` và Atom đơn lẻ
  2. Toán tử phủ định `NOT` hoặc tiền tố `-` (ví dụ `-timeout`, `NOT level:debug`)
  3. Toán tử `AND` (kết hợp ngầm định khi hai biểu thức đứng cạnh nhau bằng khoảng trắng)
  4. Toán tử `OR`
- **Các dạng Atom hỗ trợ**:

| Cú pháp | Biến thể `Expr` | Ngữ nghĩa |
| :--- | :--- | :--- |
| `term`, `"exact phrase"` | `Text(Vec<String>)` | Tìm kiếm tự do không phân biệt hoa thường trên `__message`, cột text và cột số (qua SIMD numeric `eq`). |
| `field:v1\|v2` | `FieldContainsAny { field, values }` | Trường `field` chứa ít nhất một giá trị (không phân biệt hoa thường). |
| `field=val` | `FieldExact { field, value }` | Khớp chính xác toàn bộ giá trị (case-insensitive). |
| `field:~regex` / `field~:regex` | `FieldRegex { field, pattern, re }` | Khớp biểu thức chính quy (`regex::Regex`). |
| `field:-val`, `field:-~regex` | `Not(Box<Expr>)` | Phủ định điều kiện trường tương ứng. |
| `field:>N`, `>=`, `<`, `<=` | `FieldCmp { field, op, value }` | So sánh số hoặc thời gian (`__timestamp_secs`, `__id`, cột số/utf8 parse được). |
| `field:A..B` | `FieldRange { field, lo, hi }` | Khoảng đóng `[min(A, B), max(A, B)]` không phụ thuộc thứ tự `A`/`B`. |

### 2.2. Đơn vị Thời gian Tương đối (`core_util`)
Hàm `parse_numeric_value(s, now)` trong [crates/core_util/src/lib.rs](../../crates/core_util/src/lib.rs#L96-L158) phân giải:
- `now` $\rightarrow$ mốc `data_now` (timestamp lớn nhất trong storage, fallback về `now_secs()`).
- `now-5m`, `now+30s` hoặc duration trần `10m` ($\rightarrow$ `now - secs`).
- Đơn vị hỗ trợ: `s` (giây), `m` (phút = 60s), `h` (giờ = 3600s), `d` (ngày = 86400s), `w` (tuần = 604800s), `M` (tháng $\approx$ 2629746s), `y` (năm = 31536000s).

---

## 3. Quy tắc Đồng bộ Tuyệt đối (`Expr` Parity)

Mọi biến thể `Expr` mới thêm vào [parser.rs](../../crates/core_filter/src/parser.rs) **bắt buộc** phải được cài đặt tương đương ở cả:
1. [evaluator.rs](../../crates/core_filter/src/evaluator.rs) (`eval_event`)
2. [compiler.rs](../../crates/core_engine/src/arrow_storage/compiler.rs) (`QueryCompiler::eval_batch_with_mask`)

Kiểm tra tự động bằng `./.agents/scripts/harness.sh check-arch`.
