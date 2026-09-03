//! High-performance Synthetic Log Generator for uwulog-rust Benchmarking

use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;
use uwu_core_schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};

pub struct SyntheticLogGenerator {
    state: u64,
}

impl SyntheticLogGenerator {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    #[inline]
    fn gen_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        min + (self.next_u64() as usize % (max - min))
    }

    /// Sinh n dòng LogEvent đã chuẩn hóa sẵn vào bộ nhớ RAM
    pub fn generate_events(&mut self, count: usize) -> Vec<LogEvent> {
        let mut events = Vec::with_capacity(count);
        let base_time = Utc::now().timestamp() as f64;

        let levels = [
            LogLevel::Info,
            LogLevel::Info,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Warn,
            LogLevel::Error,
            LogLevel::Fatal,
        ];

        let tags = [
            "sys.auth",
            "sys.kernel",
            "sys.network",
            "app.web",
            "app.database",
            "worker.queue",
            "heartbeat",
        ];

        let sources = [
            "server-01",
            "server-02",
            "auth-service",
            "gateway",
            "heartbeat",
        ];

        let msg_templates = [
            "database connection timeout after 30s",
            "user_1024 login failed: invalid password",
            "user_9999 session established successfully",
            "HTTP GET /api/v1/users returned 200 OK",
            "Background job processed 150 items successfully",
            "Disk usage exceeded 90% threshold on /dev/sda1",
            "heartbeat tick alive",
        ];

        for i in 0..count {
            let lvl = levels[self.gen_range(0, levels.len())];
            let tag = tags[self.gen_range(0, tags.len())];
            let source = sources[self.gen_range(0, sources.len())];
            let msg = msg_templates[self.gen_range(0, msg_templates.len())];
            let latency = self.gen_range(5, 1200) as i64;
            let duration = self.gen_range(50, 1000) as i64;

            // Timestamp lùi dần 1 giây mỗi 10 log
            let ts_secs = base_time - ((count - i) as f64 * 0.1);
            let ts_str = chrono::DateTime::from_timestamp(ts_secs as i64, 0)
                .unwrap_or_else(Utc::now)
                .to_rfc3339();

            let mut fields = HashMap::with_capacity(6);
            fields.insert(
                "tag".to_string(),
                serde_json::Value::String(tag.to_string()),
            );
            fields.insert(
                "source".to_string(),
                serde_json::Value::String(source.to_string()),
            );
            fields.insert("latency".to_string(), serde_json::json!(latency));
            fields.insert("duration".to_string(), serde_json::json!(duration));
            fields.insert(
                "req_id".to_string(),
                serde_json::json!(format!("req_{:06}", i % 100000)),
            );

            events.push(LogEvent {
                id: Uuid::new_v4(),
                timestamp: ts_str,
                timestamp_secs: Some(ts_secs),
                level: lvl,
                message: msg.to_string(),
                fields,
            });
        }

        events
    }

    /// Sinh n dòng RawLogEntry (JSON) để test Ingestion & Normalizer
    pub fn generate_raw_json_entries(&mut self, count: usize) -> Vec<RawLogEntry> {
        let mut entries = Vec::with_capacity(count);
        let base_time = Utc::now().timestamp();

        for i in 0..count {
            let ts_secs = base_time - ((count - i) as i64 / 10);
            let lvl_str = match self.gen_range(0, 5) {
                0 => "DEBUG",
                1 => "INFO",
                2 => "WARN",
                3 => "ERROR",
                _ => "FATAL",
            };

            let json_val = serde_json::json!({
                "timestamp": ts_secs,
                "level": lvl_str,
                "message": format!("Processed transaction #{} with status OK", i),
                "tag": "sys.auth",
                "latency": self.gen_range(10, 1500),
                "source": "bench-agent"
            });

            entries.push(RawLogEntry {
                payload: RawPayload::Json(json_val),
            });
        }

        entries
    }

    /// Sinh n dòng RawLogEntry dạng Plain Text (Syslog / Nginx style)
    pub fn generate_raw_text_entries(&mut self, count: usize) -> Vec<RawLogEntry> {
        let mut entries = Vec::with_capacity(count);

        for i in 0..count {
            let text = format!(
                "2026-09-03T12:00:{:02}Z [ERROR] (sys.auth) user_{:04} login timeout after {}ms",
                i % 60,
                self.gen_range(1000, 9999),
                self.gen_range(100, 2000)
            );

            entries.push(RawLogEntry {
                payload: RawPayload::Text(text),
            });
        }

        entries
    }
}
