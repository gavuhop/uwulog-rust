use anyhow::Result;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use uwu_core::LogEngine as CoreFilterEngine;
use uwu_normalizer::LogNormalizer;
use uwu_schema::{LogEvent, RawLogEntry};
use uwu_sources::LogSource;

pub struct SystemEngine {
    raw_tx: mpsc::Sender<RawLogEntry>,
    events: Arc<RwLock<VecDeque<LogEvent>>>,
    filter_engine: Arc<RwLock<CoreFilterEngine>>,
    total_processed: Arc<AtomicU64>,
    max_capacity: usize,
}

impl SystemEngine {
    pub fn new(max_capacity: usize) -> Self {
        let (raw_tx, mut raw_rx) = mpsc::channel::<RawLogEntry>(10_000);
        let events = Arc::new(RwLock::new(VecDeque::with_capacity(max_capacity)));
        let filter_engine = Arc::new(RwLock::new(CoreFilterEngine::new(max_capacity)));
        let total_processed = Arc::new(AtomicU64::new(0));

        let events_clone = Arc::clone(&events);
        let filter_clone = Arc::clone(&filter_engine);
        let processed_clone = Arc::clone(&total_processed);

        // Async task liên tục đọc RawLogEntry -> Normalizer -> LogEvent -> Storage & Filter Engine
        tokio::spawn(async move {
            while let Some(raw_entry) = raw_rx.recv().await {
                let event = LogNormalizer::normalize(raw_entry);

                // Chuyển LogEvent thành serde_json::Value để nạp vào uwu-core filter engine
                let mut json_val = serde_json::json!({
                    "id": event.id,
                    "timestamp": event.timestamp.to_rfc3339(),
                    "level": event.level.to_string(),
                    "source": event.source_id,
                    "message": event.message,
                    "raw": event.raw,
                });

                if let Some(obj) = json_val.as_object_mut() {
                    for (k, v) in &event.fields {
                        obj.insert(k.clone(), v.clone());
                    }
                }

                // Cập nhật Filter Engine & In-memory RingBuffer
                if let Ok(mut fe) = filter_clone.write() {
                    fe.push_value(json_val);
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
            filter_engine,
            total_processed,
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

        // Fast path: Khi từ khóa rỗng, lấy trực tiếp từ In-memory RingBuffer mà không tốn CPU lọc biểu thức
        if trimmed.is_empty() {
            if let Ok(evts) = self.events.read() {
                let total_matched = evts.len();
                let skip_count = total_matched.saturating_sub(limit);
                let events = evts.iter().skip(skip_count).cloned().collect();
                return (total_matched, events);
            } else {
                return (0, Vec::new());
            }
        }

        let matched_indices = if let Ok(fe) = self.filter_engine.read() {
            fe.filter(trimmed.to_string())
        } else {
            Vec::new()
        };

        let total_matched = matched_indices.len();
        let skip_count = total_matched.saturating_sub(limit);

        let events = if let Ok(evts) = self.events.read() {
            matched_indices
                .into_iter()
                .skip(skip_count)
                .filter_map(|i| evts.get(i as usize).cloned())
                .collect()
        } else {
            Vec::new()
        };

        (total_matched, events)
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

            if let Ok(fe) = self.filter_engine.read() {
                let json_slice: Vec<serde_json::Value> =
                    new_slice.iter().map(|e| e.to_json_value()).collect();
                let matched_indices = fe.filter_slice(&json_slice, trimmed);
                let matched_events: Vec<LogEvent> = matched_indices
                    .into_iter()
                    .filter_map(|i| new_slice.get(i).cloned())
                    .collect();
                let matched_len = matched_events.len();
                return (matched_len, matched_events);
            }
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

    pub fn clear(&self) {
        if let Ok(mut evts) = self.events.write() {
            evts.clear();
        }
        if let Ok(mut fe) = self.filter_engine.write() {
            *fe = CoreFilterEngine::new(self.max_capacity);
        }
        self.total_processed.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwu_schema::RawPayload;

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
}
