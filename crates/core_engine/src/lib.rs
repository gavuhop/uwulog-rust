use anyhow::Result;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use uwu_core_filter::evaluator::eval_event;
use uwu_core_filter::parser::{tokenize, Parser};
use uwu_core_schema::{FieldType, LogEvent, RawLogEntry, StandardField};
use uwu_core_util::{detect_timestamp_format, now_secs, parse_with_format, TimestampFormat};
use uwu_driver_sources::{LogNormalizer, LogSource};

pub struct SystemEngine {
    raw_tx: mpsc::Sender<RawLogEntry>,
    events: Arc<RwLock<VecDeque<LogEvent>>>,
    schema: Arc<RwLock<HashMap<String, FieldType>>>,
    known_keys: Arc<RwLock<HashSet<String>>>,
    /// Trạng thái nhận diện timestamp: (key_name, format) — gộp 1 RwLock để tránh inconsistent state
    active_timestamp_info: Arc<RwLock<Option<(String, TimestampFormat)>>>,
    schema_version: Arc<AtomicU64>,
    total_processed: Arc<AtomicU64>,
    max_timestamp: Arc<RwLock<f64>>,
    max_capacity: usize,
}

impl SystemEngine {
    pub fn new(max_capacity: usize) -> Self {
        let (raw_tx, mut raw_rx) = mpsc::channel::<RawLogEntry>(10_000);
        let events = Arc::new(RwLock::new(VecDeque::with_capacity(max_capacity)));
        let total_processed = Arc::new(AtomicU64::new(0));
        let max_timestamp = Arc::new(RwLock::new(0.0));

        let default_schema = {
            let mut map = HashMap::new();
            for field in StandardField::default_columns() {
                map.insert(field.canonical_name().to_string(), field.field_type());
            }
            map
        };
        let schema = Arc::new(RwLock::new(default_schema));

        let default_known_keys = {
            let mut set = HashSet::new();
            for field in StandardField::default_columns() {
                set.insert(field.canonical_name().to_string());
            }
            set
        };
        let known_keys = Arc::new(RwLock::new(default_known_keys));
        let active_timestamp_info: Arc<RwLock<Option<(String, TimestampFormat)>>> =
            Arc::new(RwLock::new(None));
        let schema_version = Arc::new(AtomicU64::new(1));

        let events_clone = Arc::clone(&events);
        let processed_clone = Arc::clone(&total_processed);
        let max_ts_clone = Arc::clone(&max_timestamp);
        let schema_clone = Arc::clone(&schema);
        let known_keys_clone = Arc::clone(&known_keys);
        let ts_info_clone = Arc::clone(&active_timestamp_info);
        let schema_version_clone = Arc::clone(&schema_version);

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
                let current_schema = schema_clone.read().ok();
                let current_known_keys = known_keys_clone.read().ok();
                let mut cached_ts_info = ts_info_clone.read().ok().and_then(|g| g.clone());
                let mut ts_info_dirty = false;

                for raw_entry in raw_batch.drain(..) {
                    let mut event = LogNormalizer::normalize(raw_entry);

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
                                            cached_ts_info.map(|(_, f)| f).unwrap_or(
                                                TimestampFormat::Rfc3339, // placeholder, sẽ bị ghi đè bởi detect
                                            ),
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

                // Cập nhật timestamp info nếu có thay đổi / redetect (1 write lock duy nhất)
                if ts_info_dirty {
                    if let Ok(mut lock) = ts_info_clone.write() {
                        *lock = cached_ts_info;
                    }
                }

                let batch_len = event_batch.len();

                // 2. Cập nhật max_timestamp nếu có timestamp lớn hơn
                if batch_max_ts > 0.0 {
                    if let Ok(mut max_ts) = max_ts_clone.write() {
                        if batch_max_ts > *max_ts {
                            *max_ts = batch_max_ts;
                        }
                    }
                }

                // 3. Cập nhật Schema Registry và known_keys nếu có trường mới
                if !schema_updates.is_empty() {
                    if let Ok(mut schema_write) = schema_clone.write() {
                        for (k, ft) in &schema_updates {
                            if let Some(std_field) = StandardField::from_alias(k) {
                                let canonical = std_field.canonical_name();
                                if canonical != k {
                                    schema_write.remove(canonical);
                                }
                            }
                            schema_write.insert(k.clone(), *ft);
                        }
                    }

                    if let Ok(mut known_write) = known_keys_clone.write() {
                        for k in schema_updates.keys() {
                            known_write.insert(k.clone());
                        }
                    }

                    schema_version_clone.fetch_add(1, Ordering::Release);
                }

                // 4. Acquire write lock 1 lần duy nhất cho toàn bộ batch
                if let Ok(mut evts) = events_clone.write() {
                    let current_len = evts.len();
                    let new_total = current_len + batch_len;
                    if new_total > max_capacity {
                        let overflow = new_total - max_capacity;
                        if overflow >= current_len {
                            evts.clear();
                            let skip_in_batch = overflow - current_len;
                            evts.extend(event_batch.into_iter().skip(skip_in_batch));
                        } else {
                            evts.drain(0..overflow);
                            evts.extend(event_batch);
                        }
                    } else {
                        evts.extend(event_batch);
                    }

                    // Cập nhật atomic counter bên trong write lock để reader holding read lock luôn thấy trạng thái nhất quán
                    processed_clone.fetch_add(batch_len as u64, Ordering::Release);
                } else {
                    processed_clone.fetch_add(batch_len as u64, Ordering::Relaxed);
                }
            }
        });

        Self {
            raw_tx,
            events,
            schema,
            known_keys,
            active_timestamp_info,
            schema_version,
            total_processed,
            max_timestamp,
            max_capacity,
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
        if let Ok(max_ts) = self.max_timestamp.read() {
            if *max_ts > 0.0 {
                return *max_ts;
            }
        }
        now_secs()
    }

    pub fn search(&self, query: &str) -> Vec<LogEvent> {
        self.search_limited(query, usize::MAX)
    }

    pub fn search_limited(&self, query: &str, limit: usize) -> Vec<LogEvent> {
        self.search_with_count(query, limit).1
    }

    pub fn search_with_count(&self, query: &str, limit: usize) -> (usize, Vec<LogEvent>) {
        let trimmed = query.trim();

        if let Ok(evts) = self.events.read() {
            let total_logs = evts.len();
            if total_logs == 0 {
                return (0, Vec::new());
            }

            // Fast path: Khi từ khóa rỗng, lấy trực tiếp từ In-memory RingBuffer mà không tốn CPU lọc biểu thức
            if trimmed.is_empty() {
                let skip_count = total_logs.saturating_sub(limit);
                let events = evts.iter().skip(skip_count).cloned().collect();
                return (total_logs, events);
            }

            let data_now = self.current_data_now();

            let tokens = tokenize(trimmed);
            let mut parser = Parser::new(tokens, data_now);
            let ast = match parser.parse() {
                Some(e) => e,
                None => {
                    let skip_count = total_logs.saturating_sub(limit);
                    let events = evts.iter().skip(skip_count).cloned().collect();
                    return (total_logs, events);
                }
            };

            // Index-only collect: Rayon chỉ thu thập index usize, tránh clone hàng trăm nghìn LogEvent
            let matching_indices: Vec<usize> = evts
                .par_iter()
                .enumerate()
                .filter_map(|(idx, e)| {
                    if eval_event(&ast, e, data_now) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();

            let matched_len = matching_indices.len();
            let skip_count = matched_len.saturating_sub(limit);
            let events = matching_indices[skip_count..]
                .iter()
                .filter_map(|&idx| evts.get(idx).cloned())
                .collect();

            (matched_len, events)
        } else {
            (0, Vec::new())
        }
    }

    pub fn filter_incremental(&self, query: &str, last_processed: u64) -> (usize, Vec<LogEvent>) {
        let trimmed = query.trim();

        if let Ok(evts) = self.events.read() {
            let current_total = self.total_processed.load(Ordering::Acquire);
            if current_total <= last_processed {
                return (0, Vec::new());
            }

            let new_count = (current_total - last_processed) as usize;
            let total_in_buffer = evts.len();
            let take_count = new_count.min(total_in_buffer);
            let start_idx = total_in_buffer.saturating_sub(take_count);

            if trimmed.is_empty() {
                let events: Vec<LogEvent> = evts.iter().skip(start_idx).cloned().collect();
                let matched_len = events.len();
                return (matched_len, events);
            }

            let data_now = self.current_data_now();

            let tokens = tokenize(trimmed);
            let mut parser = Parser::new(tokens, data_now);
            let ast = match parser.parse() {
                Some(e) => e,
                None => {
                    let events: Vec<LogEvent> = evts.iter().skip(start_idx).cloned().collect();
                    let matched_len = events.len();
                    return (matched_len, events);
                }
            };

            let matched_events: Vec<LogEvent> = evts
                .iter()
                .skip(start_idx)
                .filter(|e| eval_event(&ast, e, data_now))
                .cloned()
                .collect();
            let matched_len = matched_events.len();
            return (matched_len, matched_events);
        }

        (0, Vec::new())
    }

    pub fn total_logs(&self) -> usize {
        self.events.read().map(|e| e.len()).unwrap_or(0)
    }

    pub fn total_processed(&self) -> u64 {
        self.total_processed.load(Ordering::Relaxed)
    }

    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    /// Trả về toàn bộ log chưa lọc trong RingBuffer (có giới hạn limit) và index của target_id nếu có.
    /// Nếu target_id được chỉ định, trả về cửa sổ ngữ cảnh đối xứng xung quanh target_id.
    pub fn get_unfiltered_events(
        &self,
        target_id: Option<u64>,
        limit: usize,
    ) -> (Option<usize>, Vec<LogEvent>) {
        if let Ok(evts) = self.events.read() {
            let total = evts.len();
            if total == 0 {
                return (None, Vec::new());
            }

            if let Some(target_log_id) = target_id {
                if let Some(pos) = evts.iter().position(|e| e.id == target_log_id) {
                    let half = limit / 2;
                    let start_idx = pos.saturating_sub(half);
                    let end_idx = (start_idx + limit).min(total);
                    let actual_start = end_idx.saturating_sub(limit);

                    let events: Vec<LogEvent> = evts
                        .iter()
                        .skip(actual_start)
                        .take(end_idx - actual_start)
                        .cloned()
                        .collect();
                    let target_idx = events.iter().position(|e| e.id == target_log_id);
                    return (target_idx, events);
                }
            }

            let skip_count = total.saturating_sub(limit);
            let events: Vec<LogEvent> = evts.iter().skip(skip_count).cloned().collect();
            let target_idx = target_id.and_then(|id| events.iter().position(|e| e.id == id));
            (target_idx, events)
        } else {
            (None, Vec::new())
        }
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
            let mut set = HashSet::new();
            for field in StandardField::default_columns() {
                set.insert(field.canonical_name().to_string());
            }
            *known_write = set;
        }
        self.schema_version.fetch_add(1, Ordering::Release);
    }

    /// Lấy phiên bản schema hiện tại (tăng dần khi có key mới hoặc type promotion)
    pub fn get_schema_version(&self) -> u64 {
        self.schema_version.load(Ordering::Acquire)
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
        if new_events.is_empty() {
            return;
        }
        let batch_len = new_events.len();
        let mut batch_max_ts = 0.0f64;
        for event in &new_events {
            if let Some(ts) = event.timestamp_secs {
                if ts > batch_max_ts {
                    batch_max_ts = ts;
                }
            }
        }
        if batch_max_ts > 0.0 {
            if let Ok(mut max_ts) = self.max_timestamp.write() {
                if batch_max_ts > *max_ts {
                    *max_ts = batch_max_ts;
                }
            }
        }

        if let Ok(mut evts) = self.events.write() {
            let current_len = evts.len();
            let new_total = current_len + batch_len;
            if new_total > self.max_capacity {
                let overflow = new_total - self.max_capacity;
                if overflow >= current_len {
                    evts.clear();
                    let skip_in_batch = overflow - current_len;
                    evts.extend(new_events.into_iter().skip(skip_in_batch));
                } else {
                    evts.drain(0..overflow);
                    evts.extend(new_events);
                }
            } else {
                evts.extend(new_events);
            }
            self.total_processed
                .fetch_add(batch_len as u64, Ordering::Release);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut evts) = self.events.write() {
            evts.clear();
        }
        if let Ok(mut max_ts) = self.max_timestamp.write() {
            *max_ts = 0.0;
        }
        self.total_processed.store(0, Ordering::Relaxed);
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
}
