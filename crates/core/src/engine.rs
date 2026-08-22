use crate::normalizer::LogNormalizer;
use crate::schema::{LogEvent, RawLogEntry};
use crate::sources::LogSource;
use anyhow::Result;
use rayon::prelude::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

pub struct SystemEngine {
    raw_tx: mpsc::Sender<RawLogEntry>,
    events: Arc<RwLock<VecDeque<LogEvent>>>,
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

        let events_clone = Arc::clone(&events);
        let processed_clone = Arc::clone(&total_processed);
        let max_ts_clone = Arc::clone(&max_timestamp);

        // Async task liên tục đọc RawLogEntry -> Normalizer -> LogEvent -> Storage
        tokio::spawn(async move {
            while let Some(raw_entry) = raw_rx.recv().await {
                let event = LogNormalizer::normalize(raw_entry);

                if let Some(ts) = crate::filter::utils::parse_iso_to_secs(&event.timestamp) {
                    if let Ok(mut max_ts) = max_ts_clone.write() {
                        if ts > *max_ts {
                            *max_ts = ts;
                        }
                    }
                }

                if let Ok(mut evts) = events_clone.write() {
                    evts.push_back(event);
                    if evts.len() > max_capacity {
                        evts.pop_front();
                    }
                }

                processed_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        Self {
            raw_tx,
            events,
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

            let data_now = if let Ok(max_ts) = self.max_timestamp.read() {
                if *max_ts > 0.0 {
                    *max_ts
                } else {
                    crate::filter::utils::now_secs()
                }
            } else {
                crate::filter::utils::now_secs()
            };

            let tokens = crate::filter::parser::tokenize(trimmed);
            let mut parser = crate::filter::parser::Parser::new(tokens, data_now);
            let ast = match parser.parse() {
                Some(e) => e,
                None => {
                    let skip_count = total_logs.saturating_sub(limit);
                    let events = evts.iter().skip(skip_count).cloned().collect();
                    return (total_logs, events);
                }
            };

            // Lọc trực tiếp trên evts - Bảo đảm thứ tự 100% trùng khớp và không bao giờ lệch index
            let matching: Vec<LogEvent> = evts
                .par_iter()
                .filter(|e| crate::filter::evaluator::eval_event(&ast, e, data_now))
                .cloned()
                .collect();

            let matched_len = matching.len();
            let skip_count = matched_len.saturating_sub(limit);
            let events = matching.into_iter().skip(skip_count).collect();

            (matched_len, events)
        } else {
            (0, Vec::new())
        }
    }

    pub fn filter_incremental(&self, query: &str, last_processed: u64) -> (usize, Vec<LogEvent>) {
        let trimmed = query.trim();
        let current_total = self.total_processed.load(Ordering::Relaxed);
        if current_total <= last_processed {
            return (0, Vec::new());
        }

        let new_count = (current_total - last_processed) as usize;

        if let Ok(evts) = self.events.read() {
            let total_in_buffer = evts.len();
            let take_count = new_count.min(total_in_buffer);
            let start_idx = total_in_buffer.saturating_sub(take_count);

            let new_slice: Vec<LogEvent> = evts.iter().skip(start_idx).cloned().collect();

            if trimmed.is_empty() {
                let matched_len = new_slice.len();
                return (matched_len, new_slice);
            }

            let data_now = if let Ok(max_ts) = self.max_timestamp.read() {
                if *max_ts > 0.0 {
                    *max_ts
                } else {
                    crate::filter::utils::now_secs()
                }
            } else {
                crate::filter::utils::now_secs()
            };

            let tokens = crate::filter::parser::tokenize(trimmed);
            let mut parser = crate::filter::parser::Parser::new(tokens, data_now);
            let ast = match parser.parse() {
                Some(e) => e,
                None => return (new_slice.len(), new_slice),
            };

            let matched_events: Vec<LogEvent> = new_slice
                .into_iter()
                .filter(|e| crate::filter::evaluator::eval_event(&ast, e, data_now))
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
        target_id: Option<uuid::Uuid>,
        limit: usize,
    ) -> (Option<usize>, Vec<LogEvent>) {
        if let Ok(evts) = self.events.read() {
            let total = evts.len();
            if total == 0 {
                return (None, Vec::new());
            }

            if let Some(target_uuid) = target_id {
                if let Some(pos) = evts.iter().position(|e| e.id == target_uuid) {
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
                    let target_idx = events.iter().position(|e| e.id == target_uuid);
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

    pub fn clear(&self) {
        if let Ok(mut evts) = self.events.write() {
            evts.clear();
        }
        if let Ok(mut max_ts) = self.max_timestamp.write() {
            *max_ts = 0.0;
        }
        self.total_processed.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::RawPayload;

    #[tokio::test]
    async fn test_engine_streaming_and_search() {
        let engine = SystemEngine::new(100);
        let tx = engine.get_channel();

        let raw = RawLogEntry {
            source_id: "test:stream".to_string(),
            payload: RawPayload::Text("[ERROR] Critical failure in module auth".to_string()),
        };

        tx.send(raw).await.unwrap();

        // Đợi async task nạp dữ liệu
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let results = engine.search("level:error");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_id, "test:stream");
    }

    #[tokio::test]
    async fn test_system_engine_search_limits() {
        let engine = SystemEngine::new(10);
        let tx = engine.get_channel();

        for i in 0..6 {
            tx.send(RawLogEntry {
                source_id: format!("src_{}", i),
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
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
            source_id: "s1".to_string(),
            payload: RawPayload::Text("[ERROR] Error 1".to_string()),
        })
        .await
        .unwrap();
        tx.send(RawLogEntry {
            source_id: "s2".to_string(),
            payload: RawPayload::Text("[INFO] Info 1".to_string()),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let last_processed = engine.total_processed();
        assert_eq!(last_processed, 2);

        // 2. Send second batch
        tx.send(RawLogEntry {
            source_id: "s3".to_string(),
            payload: RawPayload::Text("[ERROR] Error 2".to_string()),
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
            source_id: "s1".to_string(),
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
                source_id: format!("s{}", i),
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
                source_id: "test".to_string(),
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
                source_id: "test".to_string(),
                payload: RawPayload::Text(format!("[{}] {}", lvl, msg)),
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
            .filter(|e| e.level == crate::schema::LogLevel::Info)
            .map(|e| (e.id, e.message.clone()))
            .collect();

        let filtered_info_ids: Vec<_> = filtered_info_logs
            .iter()
            .map(|e| (e.id, e.message.clone()))
            .collect();

        assert_eq!(raw_info_ids.len(), 5);
        assert_eq!(raw_info_ids, filtered_info_ids);
    }
}
