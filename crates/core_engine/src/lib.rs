use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use uwu_core_filter::parser::{tokenize, Expr, Parser};
use uwu_core_schema::{FieldType, LogEvent, RawLogEntry, StandardField};
use uwu_core_util::{detect_timestamp_format, now_secs, parse_with_format, TimestampFormat};
use uwu_driver_sources::{LogNormalizer, LogSource};

pub mod arrow_storage;
use arrow_storage::ArrowStorage;
pub use arrow_storage::{ColdCountTask, TwoPhaseSearchResult};
use std::path::PathBuf;

pub struct SystemEngine {
    raw_tx: mpsc::Sender<RawLogEntry>,
    storage: Arc<ArrowStorage>,
    schema: Arc<RwLock<HashMap<String, FieldType>>>,
    known_keys: Arc<RwLock<HashSet<String>>>,
    /// Trạng thái nhận diện timestamp: (key_name, format) — gộp 1 RwLock để tránh inconsistent state
    active_timestamp_info: Arc<RwLock<Option<(String, TimestampFormat)>>>,
    schema_version: Arc<AtomicU64>,
    cached_query: RwLock<Option<(String, Option<Expr>)>>,
    field_collisions: Arc<RwLock<HashMap<String, u64>>>,
    collisions_version: Arc<AtomicU64>,
}

static NEXT_ENGINE_ID: AtomicU64 = AtomicU64::new(1);

impl SystemEngine {
    pub fn new(max_capacity: usize) -> Self {
        let seq = NEXT_ENGINE_ID.fetch_add(1, Ordering::Relaxed);
        let dir =
            uwu_core_util::app_spill_dir().join(format!("engine_{}_{}", std::process::id(), seq));
        Self::new_with_spill_dir(max_capacity, dir)
    }

    pub fn new_with_session_id(max_capacity: usize, session_id: impl std::fmt::Display) -> Self {
        let dir = uwu_core_util::app_spill_dir().join(format!(
            "ws_{}_{}",
            session_id,
            std::process::id()
        ));
        Self::new_with_spill_dir(max_capacity, dir)
    }

    pub fn new_with_spill_dir(max_capacity: usize, spill_dir: PathBuf) -> Self {
        let (raw_tx, mut raw_rx) = mpsc::channel::<RawLogEntry>(10_000);
        let storage = Arc::new(ArrowStorage::new_with_spill_dir(max_capacity, spill_dir));

        let default_schema = {
            let mut map = HashMap::new();
            for field in StandardField::default_columns() {
                map.insert(field.canonical_name().to_string(), field.field_type());
            }
            map
        };
        let schema = Arc::new(RwLock::new(default_schema));

        // Chỉ chứa các key thực sự xuất hiện trong log (không nạp sẵn canonical defaults)
        // để tránh xóa nhầm canonical key thật khi log có đồng thời cả canonical và alias.
        let known_keys = Arc::new(RwLock::new(HashSet::new()));
        let active_timestamp_info: Arc<RwLock<Option<(String, TimestampFormat)>>> =
            Arc::new(RwLock::new(None));
        let schema_version = Arc::new(AtomicU64::new(1));
        let field_collisions = Arc::new(RwLock::new(HashMap::new()));
        let collisions_version = Arc::new(AtomicU64::new(0));

        let storage_clone = Arc::clone(&storage);
        let schema_clone = Arc::clone(&schema);
        let known_keys_clone = Arc::clone(&known_keys);
        let ts_info_clone = Arc::clone(&active_timestamp_info);
        let schema_version_clone = Arc::clone(&schema_version);
        let collisions_clone = Arc::clone(&field_collisions);
        let collisions_version_clone = Arc::clone(&collisions_version);

        // Async task liên tục đọc RawLogEntry -> Normalizer -> Adaptive Timestamp & Change-Driven Schema -> Storage
        tokio::spawn(async move {
            let mut raw_batch = Vec::with_capacity(512);

            while let Some(first_entry) = raw_rx.recv().await {
                raw_batch.push(first_entry);

                // Drain non-blocking các phần tử sẵn có trong channel (tối đa 512)
                while raw_batch.len() < 512 {
                    match raw_rx.try_recv() {
                        Ok(entry) => raw_batch.push(entry),
                        Err(_) => break,
                    }
                }

                // 1. Chuẩn hóa (Normalize) + Adaptive Timestamp + Change-Driven Schema Discovery
                let mut batch_max_ts = 0.0f64;
                let mut event_batch = Vec::with_capacity(raw_batch.len());
                let mut schema_updates: HashMap<String, FieldType> = HashMap::new();
                let mut batch_collisions: HashMap<String, u64> = HashMap::new();
                let current_schema = schema_clone.read().ok();
                let current_known_keys = known_keys_clone.read().ok();
                let mut cached_ts_info = ts_info_clone.read().ok().and_then(|g| g.clone());
                let mut ts_info_dirty = false;

                for raw_entry in raw_batch.drain(..) {
                    let normalized = LogNormalizer::normalize_report(raw_entry);
                    for col_key in normalized.key_collisions {
                        *batch_collisions.entry(col_key).or_insert(0) += 1;
                    }
                    let mut event = normalized.event;

                    // A. Fast-Path / Adaptive Timestamp: Tự thích ứng format động
                    if !event.timestamp.is_empty() {
                        if let Some((_, fmt)) = cached_ts_info.as_ref() {
                            if let Some(ts) = parse_with_format(&event.timestamp, *fmt) {
                                event.timestamp_secs = Some(ts);
                            } else if let Some((new_fmt, new_ts)) =
                                detect_timestamp_format(&event.timestamp)
                            {
                                event.timestamp_secs = Some(new_ts);
                                cached_ts_info = Some((
                                    cached_ts_info.map(|(k, _)| k).unwrap_or_default(),
                                    new_fmt,
                                ));
                                ts_info_dirty = true;
                            }
                        } else if let Some((new_fmt, new_ts)) =
                            detect_timestamp_format(&event.timestamp)
                        {
                            event.timestamp_secs = Some(new_ts);
                            cached_ts_info = Some((String::new(), new_fmt));
                            ts_info_dirty = true;
                        }
                    }

                    if let Some(ts) = event.timestamp_secs {
                        if ts > batch_max_ts {
                            batch_max_ts = ts;
                        }
                    }

                    // B. Change-Driven Schema Discovery: Chỉ duyệt sâu khi có key mới hoặc Type Promotion
                    if let Some(ref known) = current_known_keys {
                        for (k, v) in &event.fields {
                            if !known.contains(k) {
                                // Key hoàn toàn mới — HashMap O(1) lookup thay vì Vec O(n)
                                let ft = if StandardField::from_alias(k)
                                    == Some(StandardField::Timestamp)
                                {
                                    if cached_ts_info
                                        .as_ref()
                                        .is_none_or(|(key, _)| key.is_empty())
                                    {
                                        cached_ts_info = Some((
                                            k.clone(),
                                            cached_ts_info
                                                .map(|(_, f)| f)
                                                .unwrap_or(TimestampFormat::Rfc3339),
                                        ));
                                        ts_info_dirty = true;
                                    }
                                    FieldType::Time
                                } else if v.is_number() {
                                    FieldType::Number
                                } else {
                                    FieldType::Text
                                };
                                schema_updates
                                    .entry(k.clone())
                                    .and_modify(|existing_ft| {
                                        if *existing_ft == FieldType::Text && v.is_number() {
                                            *existing_ft = FieldType::Number;
                                        }
                                    })
                                    .or_insert(ft);
                            } else if v.is_number() {
                                // Key đã biết nhưng có thể cần Type Promotion từ Text -> Number
                                if let Some(ref schema_ref) = current_schema {
                                    if schema_ref.get(k) == Some(&FieldType::Text) {
                                        schema_updates
                                            .entry(k.clone())
                                            .and_modify(|ft| *ft = FieldType::Number)
                                            .or_insert(FieldType::Number);
                                    }
                                }
                            }
                        }
                    }

                    event_batch.push(event);
                }
                drop(current_schema);
                drop(current_known_keys);

                // Cập nhật timestamp info nếu có thay đổi / redetect
                if ts_info_dirty {
                    if let Ok(mut lock) = ts_info_clone.write() {
                        *lock = cached_ts_info;
                    }
                }

                // Cập nhật Schema Registry và known_keys nếu có trường mới (không giữ 2 lock lồng nhau)
                if !schema_updates.is_empty() {
                    let known_snapshot: HashSet<String> =
                        if let Ok(mut known_write) = known_keys_clone.write() {
                            for k in schema_updates.keys() {
                                known_write.insert(k.clone());
                            }
                            known_write.clone()
                        } else {
                            HashSet::new()
                        };

                    if let Ok(mut schema_write) = schema_clone.write() {
                        for (k, ft) in &schema_updates {
                            if let Some(std_field) = StandardField::from_alias(k) {
                                let canonical = std_field.canonical_name();
                                if canonical != k && !known_snapshot.contains(canonical) {
                                    schema_write.remove(canonical);
                                }
                            }
                            schema_write.insert(k.clone(), *ft);
                        }
                    }

                    schema_version_clone.fetch_add(1, Ordering::Release);
                }

                if !batch_collisions.is_empty() {
                    if let Ok(mut col_write) = collisions_clone.write() {
                        for (k, count) in batch_collisions {
                            *col_write.entry(k).or_insert(0) += count;
                        }
                    }
                    collisions_version_clone.fetch_add(1, Ordering::Release);
                }

                // Push vào Arrow Storage
                storage_clone.push_events(event_batch);

                // Dual-Trigger: Nếu kênh nhận rỗng (log về chậm hoặc kết thúc đợt burst),
                // flush ngay để đóng gói các log còn lại trong active_builder, đảm bảo real-time.
                if raw_rx.is_empty() {
                    storage_clone.flush();
                }
            }
        });

        Self {
            raw_tx,
            storage,
            schema,
            known_keys,
            active_timestamp_info,
            schema_version,
            cached_query: RwLock::new(None),
            field_collisions,
            collisions_version,
        }
    }

    pub fn get_channel(&self) -> mpsc::Sender<RawLogEntry> {
        self.raw_tx.clone()
    }

    pub async fn add_source(&self, source: Box<dyn LogSource>) -> Result<()> {
        let tx = self.get_channel();
        source.start_stream(tx).await
    }

    #[inline]
    fn current_data_now(&self) -> f64 {
        let max_ts = self.storage.get_max_timestamp();
        if max_ts > 0.0 {
            return max_ts;
        }
        now_secs()
    }

    fn parse_query_expr(&self, query: &str) -> (Option<Expr>, f64) {
        let trimmed = query.trim();
        let data_now = self.current_data_now();
        if trimmed.is_empty() {
            return (None, data_now);
        }

        // Các biểu thức không chứa mốc tương đối `now` có AST & Regex bất biến -> tái sử dụng cache giữa các nhịp 150ms
        let uses_now = trimmed
            .as_bytes()
            .windows(3)
            .any(|w| w.eq_ignore_ascii_case(b"now"));

        if !uses_now {
            if let Ok(guard) = self.cached_query.read() {
                if let Some((cached_str, cached_expr)) = guard.as_ref() {
                    if cached_str == trimmed {
                        return (cached_expr.clone(), data_now);
                    }
                }
            }
        }

        let tokens = tokenize(trimmed);
        let mut parser = Parser::new(tokens, data_now);
        let parsed = parser.parse();

        if !uses_now {
            if let Ok(mut guard) = self.cached_query.write() {
                *guard = Some((trimmed.to_string(), parsed.clone()));
            }
        }

        (parsed, data_now)
    }

    pub fn search(&self, query: &str) -> Vec<LogEvent> {
        self.search_limited(query, usize::MAX)
    }

    pub fn search_limited(&self, query: &str, limit: usize) -> Vec<LogEvent> {
        self.search_with_count(query, limit).1
    }

    pub fn search_with_count(&self, query: &str, limit: usize) -> (usize, Vec<LogEvent>) {
        let (expr, now) = self.parse_query_expr(query);
        self.storage.search_with_count(expr.as_ref(), limit, now)
    }

    /// Pha 1 của Two-Phase Search (không khóa UI, hủy sớm theo `cancel_token`).
    pub fn search_phase1(
        &self,
        query: &str,
        limit: usize,
        cancel_token: &AtomicU64,
        query_id: u64,
    ) -> Option<(TwoPhaseSearchResult, Option<ColdCountTask>)> {
        let (expr, now) = self.parse_query_expr(query);
        self.storage
            .search_phase1(expr.as_ref(), limit, now, cancel_token, query_id)
    }

    /// Tìm kiếm phân trang ngược (Reverse Pagination): lấy tối đa `limit` logs nằm ngay trước `before_id`.
    /// Nếu `query` rỗng, trả về các bản ghi liên tục trước `before_id`.
    /// Nếu có `query`, trả về các bản ghi khớp bộ lọc nằm trước `before_id`.
    pub fn search_before(&self, query: &str, before_id: u64, limit: usize) -> Vec<LogEvent> {
        let (expr, now) = self.parse_query_expr(query);
        self.storage
            .search_before(expr.as_ref(), before_id, limit, now)
    }

    pub fn filter_incremental(&self, query: &str, last_processed: u64) -> (usize, Vec<LogEvent>) {
        let (expr, now) = self.parse_query_expr(query);
        self.storage
            .filter_incremental(expr.as_ref(), last_processed, now)
    }

    pub fn total_logs(&self) -> usize {
        self.storage.total_logs()
    }

    pub fn total_processed(&self) -> u64 {
        self.storage.total_processed()
    }

    pub fn total_spilled(&self) -> u64 {
        self.storage.total_spilled()
    }

    pub fn flush_spill(&self) {
        self.storage.flush_spill();
    }

    pub fn max_capacity(&self) -> usize {
        self.storage.max_capacity()
    }

    pub fn sealed_batches_count(&self) -> usize {
        self.storage.sealed_batches_count()
    }

    /// Trả về toàn bộ log chưa lọc trong RingBuffer (có giới hạn limit) và index của target_id nếu có.
    /// Nếu target_id được chỉ định, trả về cửa sổ ngữ cảnh đối xứng xung quanh target_id.
    pub fn get_unfiltered_events(
        &self,
        target_id: Option<u64>,
        limit: usize,
    ) -> (Option<usize>, Vec<LogEvent>) {
        self.storage.get_unfiltered_events(target_id, limit)
    }

    /// Lấy danh sách toàn bộ các trường đã phát hiện cùng kiểu dữ liệu tương ứng (O(1) read lock)
    pub fn get_schema(&self) -> Vec<(String, FieldType)> {
        if let Ok(schema) = self.schema.read() {
            schema.iter().map(|(k, v)| (k.clone(), *v)).collect()
        } else {
            Vec::new()
        }
    }

    /// Lấy bản sao HashMap của Schema Registry
    pub fn get_schema_map(&self) -> HashMap<String, FieldType> {
        if let Ok(schema) = self.schema.read() {
            schema.clone()
        } else {
            HashMap::new()
        }
    }

    /// Làm mới biến nhận diện timestamp format và known keys khi bắt đầu phiên chạy mới (Fresh State on Run/Restart).
    /// Precondition: Hàm này nên được gọi từ thread điều khiển UI khi stream vừa restart hoặc tạm dừng để tránh xung đột ghi đồng thời.
    pub fn reset_runtime_detection(&self) {
        if let Ok(mut info_lock) = self.active_timestamp_info.write() {
            *info_lock = None;
        }
        if let Ok(mut schema_write) = self.schema.write() {
            let mut map = HashMap::new();
            for field in StandardField::default_columns() {
                map.insert(field.canonical_name().to_string(), field.field_type());
            }
            *schema_write = map;
        }
        if let Ok(mut known_write) = self.known_keys.write() {
            known_write.clear();
        }
        if let Ok(mut col_write) = self.field_collisions.write() {
            if !col_write.is_empty() {
                col_write.clear();
                self.collisions_version.fetch_add(1, Ordering::Release);
            }
        }
        self.schema_version.fetch_add(1, Ordering::Release);
    }

    /// Lấy phiên bản schema hiện tại (tăng dần khi có key mới hoặc type promotion)
    pub fn get_schema_version(&self) -> u64 {
        self.schema_version.load(Ordering::Acquire)
    }

    /// Phiên bản cảnh báo va chạm key JSON (tăng khi phát hiện va chạm mới hoặc reset)
    pub fn get_collisions_version(&self) -> u64 {
        self.collisions_version.load(Ordering::Acquire)
    }

    /// Trả về danh sách các key bị va chạm khi flatten JSON (kèm số lượng log bị ảnh hưởng), sắp xếp ổn định
    pub fn get_field_collisions(&self) -> Vec<(String, u64)> {
        if let Ok(col_read) = self.field_collisions.read() {
            let mut list: Vec<(String, u64)> =
                col_read.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            list
        } else {
            Vec::new()
        }
    }

    /// Lấy thông tin định dạng timestamp đang hoạt động (key_name, TimestampFormat)
    pub fn get_detected_timestamp_info(&self) -> (Option<String>, Option<TimestampFormat>) {
        if let Ok(info_lock) = self.active_timestamp_info.read() {
            if let Some((ref key, fmt)) = *info_lock {
                let key_opt = if key.is_empty() {
                    None
                } else {
                    Some(key.clone())
                };
                return (key_opt, Some(fmt));
            }
        }
        (None, None)
    }

    /// (Benchmark) Nạp trực tiếp một tập LogEvent vào SystemEngine (hữu ích cho khởi tạo nhanh & benchmark)
    pub fn push_events(&self, new_events: Vec<LogEvent>) {
        self.storage.push_events(new_events);
    }

    pub fn clear(&self) {
        self.storage.clear();
        self.reset_runtime_detection();
    }
}

#[cfg(test)]
mod tests;
