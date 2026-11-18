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

pub struct SystemEngine {
    raw_tx: mpsc::Sender<RawLogEntry>,
    storage: Arc<ArrowStorage>,
    schema: Arc<RwLock<HashMap<String, FieldType>>>,
    known_keys: Arc<RwLock<HashSet<String>>>,
    /// Trạng thái nhận diện timestamp: (key_name, format) — gộp 1 RwLock để tránh inconsistent state
    active_timestamp_info: Arc<RwLock<Option<(String, TimestampFormat)>>>,
    schema_version: Arc<AtomicU64>,
    field_collisions: Arc<RwLock<HashMap<String, u64>>>,
    collisions_version: Arc<AtomicU64>,
}

impl SystemEngine {
    pub fn new(max_capacity: usize) -> Self {
        let (raw_tx, mut raw_rx) = mpsc::channel::<RawLogEntry>(10_000);
        let storage = Arc::new(ArrowStorage::new(max_capacity));

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
            (None, data_now)
        } else {
            let tokens = tokenize(trimmed);
            let mut parser = Parser::new(tokens, data_now);
            (parser.parse(), data_now)
        }
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
mod tests {
    use super::*;
    use uwu_core_schema::RawPayload;

    #[tokio::test]
    async fn test_engine_streaming_and_search() {
        let engine = SystemEngine::new(100);
        let tx = engine.get_channel();

        let raw = RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": "ERROR",
                "message": "Critical failure in module auth"
            })),
        };

        tx.send(raw).await.unwrap();

        // Đợi async task nạp dữ liệu
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let results = engine.search("level:error");
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_system_engine_search_limits() {
        let engine = SystemEngine::new(10);
        let tx = engine.get_channel();

        for i in 0..6 {
            tx.send(RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "level": "INFO",
                    "message": format!("Message {}", i)
                })),
            })
            .await
            .unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Search with limit 3
        let limited = engine.search_limited("level:info", 3);
        assert_eq!(limited.len(), 3);

        // Search with count
        let (total_matched, logs) = engine.search_with_count("level:info", 2);
        assert_eq!(total_matched, 6);
        assert_eq!(logs.len(), 2);

        // Empty query fast path
        let (all_count, all_logs) = engine.search_with_count("", 10);
        assert_eq!(all_count, 6);
        assert_eq!(all_logs.len(), 6);
    }

    #[tokio::test]
    async fn test_system_engine_incremental_filtering() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        // 1. Send first batch
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": "ERROR",
                "message": "Error 1"
            })),
        })
        .await
        .unwrap();
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": "INFO",
                "message": "Info 1"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let last_processed = engine.total_processed();
        assert_eq!(last_processed, 2);

        // 2. Send second batch
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": "ERROR",
                "message": "Error 2"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Filter incremental for level:error since last_processed
        let (new_matched, new_logs) = engine.filter_incremental("level:error", last_processed);
        assert_eq!(new_matched, 1);
        assert_eq!(new_logs.len(), 1);
        assert!(new_logs[0].message.contains("Error 2"));
    }

    #[tokio::test]
    async fn test_system_engine_clear() {
        let engine = SystemEngine::new(10);
        let tx = engine.get_channel();

        tx.send(RawLogEntry {
            payload: RawPayload::Text("[INFO] Test log".to_string()),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        assert_eq!(engine.total_logs(), 1);

        engine.clear();
        assert_eq!(engine.total_logs(), 0);
        assert_eq!(engine.total_processed(), 0);
        assert_eq!(engine.max_capacity(), 10);
    }

    #[tokio::test]
    async fn test_system_engine_unfiltered_events() {
        let engine = SystemEngine::new(10);
        let tx = engine.get_channel();

        for i in 0..5 {
            tx.send(RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            })
            .await
            .unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let all_logs = engine.search("");
        assert_eq!(all_logs.len(), 5);
        let target_id = all_logs[2].id;

        // Query unfiltered events with target_id
        let (target_idx, unfiltered) = engine.get_unfiltered_events(Some(target_id), 10);
        assert_eq!(unfiltered.len(), 5);
        assert_eq!(target_idx, Some(2));
        assert_eq!(unfiltered[2].id, target_id);

        // Query unfiltered events without target_id
        let (no_idx, unfiltered_all) = engine.get_unfiltered_events(None, 10);
        assert_eq!(unfiltered_all.len(), 5);
        assert_eq!(no_idx, None);
    }

    #[tokio::test]
    async fn test_system_engine_unfiltered_centered_window() {
        let engine = SystemEngine::new(30);
        let tx = engine.get_channel();

        for i in 0..20 {
            tx.send(RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Log number {}", i)),
            })
            .await
            .unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;

        let all_logs = engine.search("");
        assert_eq!(all_logs.len(), 20);

        // Target log is at index 10 (out of 0..20)
        let target_id = all_logs[10].id;

        // Request limit 6 -> should return 6 logs centered around index 10 (e.g. indices 7..13)
        let (target_idx, slice) = engine.get_unfiltered_events(Some(target_id), 6);
        assert_eq!(slice.len(), 6);
        assert!(target_idx.is_some());
        let idx = target_idx.unwrap();
        assert_eq!(slice[idx].id, target_id);
        assert_eq!(idx, 3); // 3rd position in the 6-item window
    }

    #[tokio::test]
    async fn test_filtered_order_matches_raw_stream_order() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        // Ingest a sequence of mixed logs
        let sequence = [
            ("INFO", "msg 0"),
            ("WARN", "msg 1"),
            ("ERROR", "msg 2"),
            ("INFO", "msg 3"),
            ("DEBUG", "msg 4"),
            ("INFO", "msg 5"),
            ("INFO", "msg 6"),
            ("WARN", "msg 7"),
            ("INFO", "msg 8"),
        ];

        for (lvl, msg) in &sequence {
            tx.send(RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "level": lvl,
                    "message": msg
                })),
            })
            .await
            .unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;

        let (_, raw_logs) = engine.get_unfiltered_events(None, 50);
        let (_, filtered_info_logs) = engine.search_with_count("level:info", 50);

        // Filtered INFO logs must match the exact sequence of INFO logs appearing in raw_logs
        let raw_info_ids: Vec<_> = raw_logs
            .iter()
            .filter(|e| e.color == uwu_core_schema::LogColor::Green)
            .map(|e| (e.id, e.message.clone()))
            .collect();

        let filtered_info_ids: Vec<_> = filtered_info_logs
            .iter()
            .map(|e| (e.id, e.message.clone()))
            .collect();

        assert_eq!(raw_info_ids.len(), 5);
        assert_eq!(raw_info_ids, filtered_info_ids);
    }

    #[tokio::test]
    async fn test_system_engine_schema_discovery_and_promotion() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        // 1. Initial schema has defaults
        let initial_schema = engine.get_schema_map();
        assert_eq!(initial_schema.get("timestamp"), Some(&FieldType::Time));
        assert_eq!(initial_schema.get("level"), Some(&FieldType::Text));
        assert_eq!(initial_schema.get("message"), Some(&FieldType::Text));
        assert_eq!(initial_schema.get("id"), None);

        // 2. Ingest log with text field, numeric field, and alias
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "ts": "2026-08-20T10:00:00Z",
                "lvl": "INFO",
                "msg": "hello",
                "custom_txt": "null_init",
                "latency_val": 250
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let schema1 = engine.get_schema_map();
        assert_eq!(schema1.get("ts"), Some(&FieldType::Time));
        assert_eq!(schema1.get("lvl"), Some(&FieldType::Text));
        assert_eq!(schema1.get("msg"), Some(&FieldType::Text));
        assert_eq!(schema1.get("custom_txt"), Some(&FieldType::Text));
        assert_eq!(schema1.get("latency_val"), Some(&FieldType::Number));

        // 3. Promote custom_txt from Text to Number
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "custom_txt": 404
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let schema2 = engine.get_schema_map();
        assert_eq!(schema2.get("custom_txt"), Some(&FieldType::Number));
    }

    #[tokio::test]
    async fn test_system_engine_adaptive_timestamp_and_reset() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        let v0 = engine.get_schema_version();
        assert!(v0 >= 1);

        // 1. Ingest RFC3339 timestamp
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "ts": "2026-08-20T10:00:00Z",
                "level": "INFO",
                "msg": "Log 1"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let (key1, fmt1) = engine.get_detected_timestamp_info();
        assert_eq!(key1, Some("ts".to_string()));
        assert_eq!(fmt1, Some(TimestampFormat::Rfc3339));

        let (_, logs1) = engine.search_with_count("", 10);
        assert_eq!(logs1.len(), 1);
        assert!(logs1[0].timestamp_secs.is_some());

        // 2. Dev changes logger format mid-stream: Ingest EpochSeconds
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "ts": "1724148000.5",
                "level": "INFO",
                "msg": "Log 2"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let (_, fmt2) = engine.get_detected_timestamp_info();
        assert_eq!(fmt2, Some(TimestampFormat::EpochSeconds));

        let (_, logs2) = engine.search_with_count("", 10);
        assert_eq!(logs2.len(), 2);
        assert_eq!(logs2[1].timestamp_secs, Some(1724148000.5));

        // 3. Reset runtime detection on fresh run / restart
        engine.reset_runtime_detection();
        let (key3, fmt3) = engine.get_detected_timestamp_info();
        assert_eq!(key3, None);
        assert_eq!(fmt3, None);

        // 4. Ingest new format after reset: Standard space format
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "time": "2026-08-20 10:00:00.123",
                "level": "WARN",
                "msg": "Log 3"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let (key4, fmt4) = engine.get_detected_timestamp_info();
        assert_eq!(key4, Some("time".to_string()));
        assert!(matches!(fmt4, Some(TimestampFormat::NaiveDateTime(_))));
    }

    #[tokio::test]
    async fn test_substring_match_and_mask_propagation() {
        let engine = SystemEngine::new(100);
        let tx = engine.get_channel();

        let entries = vec![
            RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "msg": "database connection timeout after 30s",
                    "tag": "app.database",
                    "level": "ERROR"
                })),
            },
            RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "msg": "database connection established successfully",
                    "tag": "app.database",
                    "level": "INFO"
                })),
            },
            RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "msg": "redis connection timeout after 10s",
                    "tag": "app.cache",
                    "level": "WARN"
                })),
            },
            RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "msg": "user login succeeded",
                    "tag": "app.auth",
                    "level": "INFO"
                })),
            },
        ];

        for entry in entries {
            tx.send(entry).await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // 1. Triple word substring search (database AND connection AND timeout)
        let (count1, logs1) = engine.search_with_count("database connection timeout", 10);
        assert_eq!(count1, 1);
        assert_eq!(logs1[0].message, "database connection timeout after 30s");

        // 2. Double word substring search (database AND connection)
        let (count2, logs2) = engine.search_with_count("database connection", 10);
        assert_eq!(count2, 2);
        assert!(logs2.iter().any(|l| l.message.contains("timeout")));
        assert!(logs2.iter().any(|l| l.message.contains("established")));

        // 3. Substring across different words (connection timeout)
        let (count3, logs3) = engine.search_with_count("connection timeout", 10);
        assert_eq!(count3, 2);
        assert!(logs3.iter().any(|l| l.message.contains("database")));
        assert!(logs3.iter().any(|l| l.message.contains("redis")));

        // 4. Disjunction (database OR redis)
        let (count4, _) = engine.search_with_count("database OR redis", 10);
        assert_eq!(count4, 3);

        // 5. Negation (database -timeout)
        let (count5, logs5) = engine.search_with_count("database -timeout", 10);
        assert_eq!(count5, 1);
        assert_eq!(
            logs5[0].message,
            "database connection established successfully"
        );

        // 6. Substring match on tag
        let (count6, _) = engine.search_with_count("app.database", 10);
        assert_eq!(count6, 2);
    }

    #[tokio::test]
    async fn test_type_aware_free_text_search() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        let entries = vec![
            RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "msg": "User login OK",
                    "status": 200,
                    "latency": 150
                })),
            },
            RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "msg": "HTTP request failed with error 500",
                    "status": 500,
                    "latency": 450
                })),
            },
            RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "msg": "Database timeout after 30s",
                    "status": 504,
                    "latency": 500
                })),
            },
        ];

        for entry in entries {
            tx.send(entry).await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // 1. Free-text query "500" should match both:
        //    - Log 2 (via text "error 500" AND status: 500)
        //    - Log 3 (via numeric latency: 500, even though msg has no "500"!)
        let (count_500, logs_500) = engine.search_with_count("500", 10);
        assert_eq!(count_500, 2);
        assert!(logs_500
            .iter()
            .any(|l| l.message.contains("HTTP request failed")));
        assert!(logs_500
            .iter()
            .any(|l| l.message.contains("Database timeout")));

        // 2. Free-text query "200" should match Log 1 (via numeric status: 200)
        let (count_200, logs_200) = engine.search_with_count("200", 10);
        assert_eq!(count_200, 1);
        assert_eq!(logs_200[0].message, "User login OK");

        // 3. Free-text query "timeout" should only match Log 3 (text match, non-numeric)
        let (count_timeout, logs_timeout) = engine.search_with_count("timeout", 10);
        assert_eq!(count_timeout, 1);
        assert_eq!(logs_timeout[0].message, "Database timeout after 30s");
    }

    #[tokio::test]
    async fn test_numeric_and_boolean_field_queries() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        let user_log = serde_json::json!({
            "lv": "TRACE",
            "source": "cache_manager",
            "user_id": "user_30",
            "status": 400,
            "latency": 12.5,
            "is_active": true,
            "metadata": {
                "instance_id": "i-322492",
                "region": "us-east-1",
                "version": "v1.0.0",
                "system": {
                    "env": "test",
                    "infra": {
                        "cluster": "global-edge-01"
                    }
                }
            }
        });

        tx.send(RawLogEntry {
            payload: RawPayload::Json(user_log),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // 1. Full user complex query including status:400
        let q = "lv:TRACE source:cache_manager metadata.instance_id:i-322492 user_id:user_30 metadata.region:us-east-1 metadata.system.env:test metadata.version:v1.0.0 metadata.system.infra.cluster:global-edge-01 status:400";
        let (count, logs) = engine.search_with_count(q, 10);
        assert_eq!(
            count, 1,
            "Full complex query with status:400 should return 1 result"
        );
        assert_eq!(
            logs[0].fields.get("status").unwrap(),
            &serde_json::json!(400)
        );

        // 2. Direct status:400
        let (count_status, _) = engine.search_with_count("status:400", 10);
        assert_eq!(
            count_status, 1,
            "status:400 should match numeric status 400"
        );

        // 3. Exact equality status=400
        let (count_exact, _) = engine.search_with_count("status=400", 10);
        assert_eq!(count_exact, 1, "status=400 should match numeric status 400");

        // 4. Multi-value status:200|400
        let (count_multi, _) = engine.search_with_count("status:200|400", 10);
        assert_eq!(count_multi, 1, "status:200|400 should match");

        // 5. Substring numeric status:40
        let (count_sub, _) = engine.search_with_count("status:40", 10);
        assert_eq!(count_sub, 1, "status:40 should substring match 400");

        // 6. Non-matching status:500
        let (count_none, _) = engine.search_with_count("status:500", 10);
        assert_eq!(count_none, 0, "status:500 should not match 400");

        // 7. Non-numeric query against numeric column status:error
        let (count_err, _) = engine.search_with_count("status:error", 10);
        assert_eq!(count_err, 0, "status:error should not match numeric column");

        // 8. Numeric range status:300..500
        let (count_range, _) = engine.search_with_count("status:300..500", 10);
        assert_eq!(count_range, 1, "status:300..500 should match 400");

        // 9. Numeric comparison status >= 400
        let (count_cmp, _) = engine.search_with_count("status>=400", 10);
        assert_eq!(count_cmp, 1, "status>=400 should match 400");

        // 10. Regex match on numeric status:~^4\d\d$
        let (count_re, _) = engine.search_with_count("status:~^4\\d\\d$", 10);
        assert_eq!(count_re, 1, "status:~^4\\d\\d$ should match 400");

        // 11. Float column query latency:12.5
        let (count_float, _) = engine.search_with_count("latency:12.5", 10);
        assert_eq!(count_float, 1, "latency:12.5 should match");

        // 12. Boolean column query is_active:true and is_active=true
        let (count_bool, _) = engine.search_with_count("is_active:true", 10);
        assert_eq!(count_bool, 1, "is_active:true should match");
        let (count_bool_exact, _) = engine.search_with_count("is_active=true", 10);
        assert_eq!(count_bool_exact, 1, "is_active=true should match");
        let (count_bool_false, _) = engine.search_with_count("is_active:false", 10);
        assert_eq!(count_bool_false, 0, "is_active:false should not match true");
    }

    #[tokio::test]
    async fn test_system_engine_search_before() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        for i in 0..20 {
            tx.send(RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message index {}", i)),
            })
            .await
            .unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let all_logs = engine.search("");
        assert_eq!(all_logs.len(), 20);

        let target_id = all_logs[10].id; // Message index 10

        // 1. Unfiltered search_before: lấy 5 logs trước Message index 10 (tức là 5..10)
        let before_unfiltered = engine.search_before("", target_id, 5);
        assert_eq!(before_unfiltered.len(), 5);
        assert_eq!(before_unfiltered[0].message, "[INFO] Message index 5");
        assert_eq!(before_unfiltered[4].message, "[INFO] Message index 9");

        // 2. Unfiltered search_before tại log đầu tiên (index 0) -> phải trả về rỗng vì không có log cũ hơn
        let oldest_id = all_logs[0].id;
        let before_oldest = engine.search_before("", oldest_id, 5);
        assert_eq!(before_oldest.len(), 0);

        // 3. Filtered search_before: tìm message:~"index 1" trước Message index 15
        // Các message có "index 1" là: 1, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19
        // Trước index 15 có: 1, 10, 11, 12, 13, 14 (đúng 6 logs, không bị lẫn với ID ngẫu nhiên)
        let before_filtered = engine.search_before("message:~\"index 1\"", all_logs[15].id, 10);
        assert_eq!(before_filtered.len(), 6);
        assert_eq!(before_filtered[0].message, "[INFO] Message index 1");
        assert_eq!(before_filtered[5].message, "[INFO] Message index 14");
    }

    #[tokio::test]
    async fn test_user_id_field_discovery_and_queries() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        // Batch 1: log without "id" field (internal __id = 1)
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": "INFO",
                "message": "no id field"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(40)).await;

        // Batch 2: logs with numeric "id" field
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "id": 500,
                "level": "INFO",
                "message": "order created"
            })),
        })
        .await
        .unwrap();
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "id": 1200,
                "level": "WARN",
                "message": "slow order"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let schema = engine.get_schema_map();
        assert_eq!(schema.get("id"), Some(&FieldType::Number));

        let (_, all_logs) = engine.search_with_count("", 10);
        assert_eq!(all_logs.len(), 3);
        let no_id_internal_id = all_logs[0].id;

        // Queries on "id" must inspect the log's "id" field, not internal __id
        let exact_internal_query = format!("id={}", no_id_internal_id);
        let (c1, r1) = engine.search_with_count(&exact_internal_query, 10);
        assert_eq!(
            c1, 0,
            "id=<internal_id> must not match internal __id when field is absent"
        );
        assert!(r1.is_empty());

        let (c2, r2) = engine.search_with_count("id=500", 10);
        assert_eq!(c2, 1);
        assert_eq!(r2[0].fields.get("id"), Some(&serde_json::json!(500)));

        let (c3, r3) = engine.search_with_count("id>600", 10);
        assert_eq!(c3, 1);
        assert_eq!(r3[0].fields.get("id"), Some(&serde_json::json!(1200)));

        let (c4, r4) = engine.search_with_count("id:400..600", 10);
        assert_eq!(c4, 1);
        assert_eq!(r4[0].fields.get("id"), Some(&serde_json::json!(500)));

        // String "id" field on a fresh engine
        let str_engine = SystemEngine::new(50);
        let str_tx = str_engine.get_channel();
        str_tx
            .send(RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "id": "req-abc",
                    "level": "INFO",
                    "message": "string id log"
                })),
            })
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert_eq!(
            str_engine.get_schema_map().get("id"),
            Some(&FieldType::Text)
        );
        let (sc, sr) = str_engine.search_with_count("id:req-abc", 10);
        assert_eq!(sc, 1);
        assert_eq!(sr[0].get_field_cow("id").as_deref(), Some("req-abc"));
    }

    #[tokio::test]
    async fn test_log_field_fidelity_round_trip_and_parity() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        // Batch 1: null first, bool, __* keys, empty object, structured array, conflicting aliases
        let log1 = serde_json::json!({
            "__id": 777,
            "__message": "shadow",
            "__meta": "internal_like",
            "id": 42,
            "level": "INFO",
            "lvl": 30,
            "message": "first log",
            "status": null,
            "ok": true,
            "tags": [1, 2],
            "meta": {},
            "raw_str_arr": "[1,2]"
        });
        let log2 = serde_json::json!({
            "id": 43,
            "level": "WARN",
            "message": "second log",
            "status": 200,
            "ok": false,
            "tags": [3],
            "only_null": null
        });
        let log3 = serde_json::json!({
            "id": 44,
            "level": "ERROR",
            "message": "third log",
            "status": null
        });

        for payload in [log1, log2, log3] {
            tx.send(RawLogEntry {
                payload: RawPayload::Json(payload),
            })
            .await
            .unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;

        // Batch 2: tags switches from array to plain string across batches (unify_and_concat safety)
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "id": 45,
                "level": "INFO",
                "message": "fourth log",
                "tags": "plain_string"
            })),
        })
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // E3: Both "level" and "lvl" must remain in schema
        let schema = engine.get_schema_map();
        assert!(schema.contains_key("level"));
        assert!(schema.contains_key("lvl"));
        assert!(schema.contains_key("__id"));
        assert!(schema.contains_key("__meta"));

        let all_logs = engine.search("");
        assert_eq!(all_logs.len(), 4);

        // E1: __id / __message / __meta preserved without colliding with Arrow system columns
        assert_eq!(
            all_logs[0].fields.get("__id"),
            Some(&serde_json::json!(777))
        );
        assert_eq!(
            all_logs[0].fields.get("__message"),
            Some(&serde_json::json!("shadow"))
        );
        assert_eq!(
            all_logs[0].fields.get("__meta"),
            Some(&serde_json::json!("internal_like"))
        );
        assert_eq!(all_logs[0].message, "first log");

        // E6, E7, E8: bool, null, empty object preserved accurately
        assert_eq!(all_logs[0].fields.get("ok"), Some(&serde_json::json!(true)));
        assert_eq!(
            all_logs[0].fields.get("status"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(all_logs[0].fields.get("meta"), Some(&serde_json::json!({})));
        assert_eq!(
            all_logs[0].fields.get("raw_str_arr"),
            Some(&serde_json::json!("[1,2]"))
        );
        assert_eq!(
            all_logs[1].fields.get("status"),
            Some(&serde_json::json!(200))
        );
        assert_eq!(
            all_logs[1].fields.get("ok"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            all_logs[1].fields.get("only_null"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(
            all_logs[2].fields.get("status"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(all_logs[3].fields.get("status"), None);
        assert_eq!(
            all_logs[3].fields.get("tags"),
            Some(&serde_json::json!("plain_string"))
        );

        // E2: Free-text numeric search for internal __id must NOT match log3 (whose user fields don't have that number)
        let log3_internal_id = all_logs[2].id.to_string();
        let (internal_hits, _) = engine.search_with_count(&log3_internal_id, 10);
        let scalar_internal_hits = all_logs
            .iter()
            .filter(|e| e.matches_text(&log3_internal_id))
            .count();
        assert_eq!(internal_hits, scalar_internal_hits);

        // Parity between Arrow QueryCompiler and scalar eval_event
        let queries = [
            "true",
            "false",
            "200",
            "id=42",
            "__id=777",
            "__meta:internal_like",
            "ok:true",
            "status>100",
            "lvl=30",
            "tags:plain_string",
            "shadow",
        ];
        let now = uwu_core_util::now_secs();
        for q in queries {
            let arrow_ids: Vec<u64> = engine.search(q).into_iter().map(|e| e.id).collect();
            let tokens = uwu_core_filter::parser::tokenize(q);
            let mut parser = uwu_core_filter::parser::Parser::new(tokens, now);
            let expr = parser.parse().unwrap();
            let scalar_ids: Vec<u64> = all_logs
                .iter()
                .filter(|e| uwu_core_filter::evaluator::eval_event(&expr, e, now))
                .map(|e| e.id)
                .collect();
            assert_eq!(arrow_ids, scalar_ids, "Parity mismatch for query '{q}'");
        }
    }

    #[tokio::test]
    async fn test_edge_case_promotions_order_and_non_object_end_to_end() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        // Batch 1: E4 (JSON array text), E5 (non-object Json), E10 (spaced key), alias "lvl" BEFORE "level", all-null field
        tx.send(RawLogEntry {
            payload: RawPayload::Text("[1, 2, 3]".to_string()),
        })
        .await
        .unwrap();
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!(["x", "y"])),
        })
        .await
        .unwrap();
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "lvl": "INFO",
                " level ": "spaced_val",
                "message": "alias first",
                "cross_null": null
            })),
        })
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Batch 2: canonical "level" arrives AFTER "lvl", cross_null becomes Int64,
        // plus same-batch promotions: bool->String, Int64->null, json_array->String
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": "WARN",
                "message": "canonical second",
                "cross_null": 250,
                "bool_mix": true,
                "num_then_null": 99,
                "json_mix": [1, 2]
            })),
        })
        .await
        .unwrap();
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "level": "ERROR",
                "message": "same batch promotion",
                "bool_mix": "fallback_str",
                "num_then_null": null,
                "json_mix": "plain_after_array"
            })),
        })
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let schema = engine.get_schema_map();
        assert!(schema.contains_key("lvl"));
        assert!(schema.contains_key("level"));
        assert!(schema.contains_key(" level "));

        let logs = engine.search("");
        assert_eq!(logs.len(), 5);
        assert_eq!(logs[0].message, "[1, 2, 3]");
        assert_eq!(logs[1].message, "[\"x\",\"y\"]");
        assert_eq!(
            logs[2].fields.get(" level "),
            Some(&serde_json::json!("spaced_val"))
        );
        assert_eq!(
            logs[2].fields.get("cross_null"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(
            logs[3].fields.get("cross_null"),
            Some(&serde_json::json!(250))
        );
        assert_eq!(
            logs[3].fields.get("bool_mix"),
            Some(&serde_json::json!("true"))
        );
        assert_eq!(
            logs[4].fields.get("bool_mix"),
            Some(&serde_json::json!("fallback_str"))
        );
        assert_eq!(
            logs[3].fields.get("num_then_null"),
            Some(&serde_json::json!(99))
        );
        assert_eq!(
            logs[4].fields.get("num_then_null"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(
            logs[3].fields.get("json_mix"),
            Some(&serde_json::json!("[1,2]"))
        );
        assert_eq!(
            logs[4].fields.get("json_mix"),
            Some(&serde_json::json!("plain_after_array"))
        );

        // Queries on cross-batch null->Int64 and num_then_null
        let (c_cross, _) = engine.search_with_count("cross_null>200", 10);
        assert_eq!(c_cross, 1);
        let (c_num, _) = engine.search_with_count("num_then_null=99", 10);
        assert_eq!(c_num, 1);
    }

    #[tokio::test]
    async fn test_engine_records_and_resets_field_collisions_without_dropping_logs() {
        let engine = SystemEngine::new(50);
        let tx = engine.get_channel();

        assert_eq!(engine.get_collisions_version(), 0);
        assert!(engine.get_field_collisions().is_empty());

        for _ in 0..2 {
            tx.send(RawLogEntry {
                payload: RawPayload::Text(
                    r#"{"a.b": 1, "a": {"b": 2}, "message": "colliding"}"#.to_string(),
                ),
            })
            .await
            .unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert!(engine.get_collisions_version() >= 1);
        assert_eq!(engine.get_field_collisions(), vec![("a.b".to_string(), 2)]);
        assert_eq!(engine.search("").len(), 2);

        engine.clear();
        assert!(engine.get_field_collisions().is_empty());
    }
}
