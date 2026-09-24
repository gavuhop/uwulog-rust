use super::builder::ActiveRecordBatchBuilder;
use super::compiler::QueryCompiler;
use arrow::array::{
    Array, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array,
    UInt8Array,
};
use arrow::datatypes::DataType;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use uwu_core_filter::parser::Expr;
use uwu_core_schema::{LogColor, LogEvent, LogFields};

pub struct ArrowStorage {
    max_capacity: usize,
    sealed_batches: RwLock<VecDeque<RecordBatch>>,
    active_builder: Mutex<ActiveRecordBatchBuilder>,
    total_stored_rows: AtomicU64,
    total_ingested: AtomicU64,
    max_timestamp: RwLock<f64>,
}

impl ArrowStorage {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            max_capacity,
            sealed_batches: RwLock::new(VecDeque::new()),
            active_builder: Mutex::new(ActiveRecordBatchBuilder::new(4096)),
            total_stored_rows: AtomicU64::new(0),
            total_ingested: AtomicU64::new(0),
            max_timestamp: RwLock::new(0.0),
        }
    }

    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    pub fn total_logs(&self) -> usize {
        let in_sealed = self.total_stored_rows.load(Ordering::Relaxed) as usize;
        let in_active = self.active_builder.lock().len();
        in_sealed + in_active
    }

    pub fn total_processed(&self) -> u64 {
        self.total_ingested.load(Ordering::Acquire)
    }

    pub fn get_max_timestamp(&self) -> f64 {
        *self.max_timestamp.read()
    }

    pub fn flush(&self) {
        let mut builder = self.active_builder.lock();
        if !builder.is_empty() {
            if let Some(batch) = builder.seal() {
                self.append_sealed_batch(batch);
            }
        }
    }

    pub fn append_sealed_batch(&self, batch: RecordBatch) {
        let batch_rows = batch.num_rows();
        if batch_rows == 0 {
            return;
        }

        let mut sealed = self.sealed_batches.write();
        sealed.push_back(batch);

        let mut current_total =
            self.total_stored_rows
                .fetch_add(batch_rows as u64, Ordering::SeqCst) as usize
                + batch_rows;

        // Ring buffer trim if exceeding max_capacity
        if current_total > self.max_capacity {
            let mut to_drop = current_total - self.max_capacity;
            while to_drop > 0 && !sealed.is_empty() {
                let front_rows = sealed.front().unwrap().num_rows();
                if front_rows <= to_drop {
                    sealed.pop_front();
                    to_drop -= front_rows;
                    current_total -= front_rows;
                } else {
                    let front = sealed.pop_front().unwrap();
                    let remaining = front_rows - to_drop;
                    let sliced = front.slice(to_drop, remaining);
                    sealed.push_front(sliced);
                    current_total -= to_drop;
                    to_drop = 0;
                }
            }
            self.total_stored_rows
                .store(current_total as u64, Ordering::SeqCst);
        }
    }

    pub fn push_event(&self, event: &LogEvent) {
        if let Some(ts) = event.timestamp_secs {
            let mut max_ts = self.max_timestamp.write();
            if ts > *max_ts {
                *max_ts = ts;
            }
        }

        let mut builder = self.active_builder.lock();
        builder.append_log(event);
        if builder.len() >= 4096 {
            if let Some(batch) = builder.seal() {
                drop(builder); // Drop builder lock before acquiring sealed_batches lock
                self.append_sealed_batch(batch);
            }
        }
        self.total_ingested.fetch_add(1, Ordering::Release);
    }

    pub fn push_events(&self, events: Vec<LogEvent>) {
        if events.is_empty() {
            return;
        }

        let mut batch_max_ts = 0.0f64;
        for ev in &events {
            if let Some(ts) = ev.timestamp_secs {
                if ts > batch_max_ts {
                    batch_max_ts = ts;
                }
            }
        }

        if batch_max_ts > 0.0 {
            let mut max_ts = self.max_timestamp.write();
            if batch_max_ts > *max_ts {
                *max_ts = batch_max_ts;
            }
        }

        let count = events.len();

        // For large bulk ingestion (>= 4096 logs, like preloading or file import):
        // Leverage Rayon parallel workers to build Arrow columnar RecordBatches concurrently!
        if count >= 4096 {
            self.flush();

            let chunk_size = 4096;
            let new_batches: Vec<RecordBatch> = events
                .par_chunks(chunk_size)
                .filter_map(|chunk| {
                    let mut b = ActiveRecordBatchBuilder::new(chunk.len());
                    for ev in chunk {
                        b.append_log(ev);
                    }
                    b.seal()
                })
                .collect();

            for batch in new_batches {
                self.append_sealed_batch(batch);
            }
        } else {
            let mut builder = self.active_builder.lock();

            for event in events {
                builder.append_log(&event);
                if builder.len() >= 4096 {
                    if let Some(batch) = builder.seal() {
                        drop(builder);
                        self.append_sealed_batch(batch);
                        builder = self.active_builder.lock();
                    }
                }
            }

            if !builder.is_empty() {
                if let Some(batch) = builder.seal() {
                    drop(builder);
                    self.append_sealed_batch(batch);
                }
            }
        }

        self.total_ingested
            .fetch_add(count as u64, Ordering::Release);
    }

    pub fn clear(&self) {
        let mut builder = self.active_builder.lock();
        let _ = builder.seal(); // Discard active rows
        let mut sealed = self.sealed_batches.write();
        sealed.clear();
        self.total_stored_rows.store(0, Ordering::SeqCst);
        self.total_ingested.store(0, Ordering::SeqCst);
        *self.max_timestamp.write() = 0.0;
    }

    pub fn search_with_count(
        &self,
        expr: Option<&Expr>,
        limit: usize,
        now: f64,
    ) -> (usize, Vec<LogEvent>) {
        self.flush();

        let sealed = self.sealed_batches.read();
        if sealed.is_empty() {
            return (0, Vec::new());
        }

        // 1. Fast-path: Empty query (no filter expr)
        if expr.is_none() {
            let total_rows: usize = sealed.iter().map(|b| b.num_rows()).sum();
            if total_rows == 0 {
                return (0, Vec::new());
            }

            let skip_count = total_rows.saturating_sub(limit);
            let take_count = total_rows - skip_count;

            let mut events = Vec::with_capacity(take_count);
            let mut skipped = 0;

            for batch in sealed.iter() {
                let b_rows = batch.num_rows();
                if skipped + b_rows <= skip_count {
                    skipped += b_rows;
                    continue;
                }

                let batch_skip = skip_count.saturating_sub(skipped);
                let available_in_batch = b_rows - batch_skip;
                let needed = take_count - events.len();
                let to_take = available_in_batch.min(needed);

                for r in batch_skip..(batch_skip + to_take) {
                    events.push(Self::materialize_row(batch, r));
                }
                skipped += b_rows;

                if events.len() >= take_count {
                    break;
                }
            }

            return (total_rows, events);
        }

        let expr_ref = expr.unwrap();

        // 2. Parallel SIMD Vectorized Evaluation across batches
        let batch_evals: Vec<(BooleanArray, usize)> = sealed
            .par_iter()
            .map(|batch| {
                let mask = QueryCompiler::eval_batch(expr_ref, batch, now);
                let true_cnt = mask.true_count();
                (mask, true_cnt)
            })
            .collect();

        let total_matched: usize = batch_evals.iter().map(|(_, c)| *c).sum();
        if total_matched == 0 {
            return (0, Vec::new());
        }

        let skip_matches = total_matched.saturating_sub(limit);
        let take_matches = total_matched - skip_matches;

        // 3. Late Materialization: Only reconstruct LogEvent for matching rows in viewport
        let mut events = Vec::with_capacity(take_matches);
        let mut current_match_idx = 0;

        for (batch_idx, batch) in sealed.iter().enumerate() {
            let (mask, true_cnt) = &batch_evals[batch_idx];
            if *true_cnt == 0 {
                continue;
            }

            if current_match_idx + true_cnt <= skip_matches {
                current_match_idx += true_cnt;
                continue;
            }

            for row_idx in 0..batch.num_rows() {
                if mask.value(row_idx) {
                    if current_match_idx >= skip_matches && events.len() < take_matches {
                        events.push(Self::materialize_row(batch, row_idx));
                    }
                    current_match_idx += 1;
                    if events.len() >= take_matches {
                        break;
                    }
                }
            }

            if events.len() >= take_matches {
                break;
            }
        }

        (total_matched, events)
    }

    pub fn filter_incremental(
        &self,
        expr: Option<&Expr>,
        last_processed: u64,
        now: f64,
    ) -> (usize, Vec<LogEvent>) {
        self.flush();

        let current_total = self.total_ingested.load(Ordering::Acquire);
        if current_total <= last_processed {
            return (0, Vec::new());
        }

        let new_count = (current_total - last_processed) as usize;
        let sealed = self.sealed_batches.read();
        let total_in_storage: usize = sealed.iter().map(|b| b.num_rows()).sum();
        if total_in_storage == 0 {
            return (0, Vec::new());
        }

        let take_rows = new_count.min(total_in_storage);
        let start_storage_idx = total_in_storage.saturating_sub(take_rows);

        // Fast path: Empty query
        if expr.is_none() {
            let mut events = Vec::with_capacity(take_rows);
            let mut passed_rows = 0;

            for batch in sealed.iter() {
                let b_rows = batch.num_rows();
                if passed_rows + b_rows <= start_storage_idx {
                    passed_rows += b_rows;
                    continue;
                }

                let batch_skip = start_storage_idx.saturating_sub(passed_rows);
                let to_take = (b_rows - batch_skip).min(take_rows - events.len());

                for r in batch_skip..(batch_skip + to_take) {
                    events.push(Self::materialize_row(batch, r));
                }
                passed_rows += b_rows;

                if events.len() >= take_rows {
                    break;
                }
            }

            let len = events.len();
            return (len, events);
        }

        let expr_ref = expr.unwrap();
        let mut matched_events = Vec::new();
        let mut passed_rows = 0;

        for batch in sealed.iter() {
            let b_rows = batch.num_rows();
            if passed_rows + b_rows <= start_storage_idx {
                passed_rows += b_rows;
                continue;
            }

            let batch_skip = start_storage_idx.saturating_sub(passed_rows);
            let mask = QueryCompiler::eval_batch(expr_ref, batch, now);

            for r in batch_skip..b_rows {
                if mask.value(r) {
                    matched_events.push(Self::materialize_row(batch, r));
                }
            }
            passed_rows += b_rows;
        }

        let matched_len = matched_events.len();
        (matched_len, matched_events)
    }

    pub fn get_unfiltered_events(
        &self,
        target_id: Option<u64>,
        limit: usize,
    ) -> (Option<usize>, Vec<LogEvent>) {
        self.flush();

        let sealed = self.sealed_batches.read();
        let total: usize = sealed.iter().map(|b| b.num_rows()).sum();
        if total == 0 {
            return (None, Vec::new());
        }

        if let Some(target_log_id) = target_id {
            // Find global position of target_id
            let mut target_global_idx = None;
            let mut global_offset = 0;

            for batch in sealed.iter() {
                if let Some(col) = batch
                    .column_by_name("__id")
                    .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
                {
                    let b_rows = batch.num_rows();
                    if b_rows > 0 {
                        let first_id = col.value(0);
                        let last_id = col.value(b_rows - 1);
                        if target_log_id >= first_id && target_log_id <= last_id {
                            // Target might be in this batch
                            for r in 0..b_rows {
                                if col.value(r) == target_log_id {
                                    target_global_idx = Some(global_offset + r);
                                    break;
                                }
                            }
                        }
                    }
                }
                if target_global_idx.is_some() {
                    break;
                }
                global_offset += batch.num_rows();
            }

            if let Some(pos) = target_global_idx {
                let half = limit / 2;
                let start_idx = pos.saturating_sub(half);
                let end_idx = (start_idx + limit).min(total);
                let actual_start = end_idx.saturating_sub(limit);
                let take_count = end_idx - actual_start;

                let mut events = Vec::with_capacity(take_count);
                let mut skipped = 0;

                for batch in sealed.iter() {
                    let b_rows = batch.num_rows();
                    if skipped + b_rows <= actual_start {
                        skipped += b_rows;
                        continue;
                    }

                    let batch_skip = actual_start.saturating_sub(skipped);
                    let to_take = (b_rows - batch_skip).min(take_count - events.len());

                    for r in batch_skip..(batch_skip + to_take) {
                        events.push(Self::materialize_row(batch, r));
                    }
                    skipped += b_rows;

                    if events.len() >= take_count {
                        break;
                    }
                }

                let target_idx = events.iter().position(|e| e.id == target_log_id);
                return (target_idx, events);
            }
        }

        // Target not specified or not found: return the last limit rows
        let skip_count = total.saturating_sub(limit);
        let take_count = total - skip_count;

        let mut events = Vec::with_capacity(take_count);
        let mut skipped = 0;

        for batch in sealed.iter() {
            let b_rows = batch.num_rows();
            if skipped + b_rows <= skip_count {
                skipped += b_rows;
                continue;
            }

            let batch_skip = skip_count.saturating_sub(skipped);
            let to_take = (b_rows - batch_skip).min(take_count - events.len());

            for r in batch_skip..(batch_skip + to_take) {
                events.push(Self::materialize_row(batch, r));
            }
            skipped += b_rows;

            if events.len() >= take_count {
                break;
            }
        }

        let target_idx = target_id.and_then(|id| events.iter().position(|e| e.id == id));
        (target_idx, events)
    }

    pub fn materialize_row(batch: &RecordBatch, row_idx: usize) -> LogEvent {
        let id = if let Some(col) = batch
            .column_by_name("__id")
            .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
        {
            col.value(row_idx)
        } else {
            0
        };

        let color = if let Some(col) = batch
            .column_by_name("__color")
            .and_then(|c| c.as_any().downcast_ref::<UInt8Array>())
        {
            match col.value(row_idx) {
                0 => LogColor::Red,
                1 => LogColor::Yellow,
                2 => LogColor::Green,
                3 => LogColor::Gray,
                _ => LogColor::Default,
            }
        } else {
            LogColor::Default
        };

        let timestamp = if let Some(col) = batch
            .column_by_name("__timestamp")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        {
            if col.is_valid(row_idx) {
                col.value(row_idx).to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let timestamp_secs = if let Some(col) = batch
            .column_by_name("__timestamp_secs")
            .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
        {
            if col.is_valid(row_idx) {
                Some(col.value(row_idx))
            } else {
                None
            }
        } else {
            None
        };

        let message = if let Some(col) = batch
            .column_by_name("__message")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        {
            if col.is_valid(row_idx) {
                col.value(row_idx).to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let schema = batch.schema();
        let fields_count = schema.fields().len().saturating_sub(5);
        let mut fields = LogFields::with_capacity(fields_count);

        for (col_idx, field) in schema.fields().iter().enumerate() {
            let name = field.name();
            if name.starts_with("__") {
                continue;
            }
            let col = batch.column(col_idx);
            if col.is_null(row_idx) {
                continue;
            }

            let val = match col.data_type() {
                DataType::Int64 => {
                    if let Some(c) = col.as_any().downcast_ref::<Int64Array>() {
                        serde_json::Value::Number(c.value(row_idx).into())
                    } else {
                        continue;
                    }
                }
                DataType::Float64 => {
                    if let Some(c) = col.as_any().downcast_ref::<Float64Array>() {
                        if let Some(n) = serde_json::Number::from_f64(c.value(row_idx)) {
                            serde_json::Value::Number(n)
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                DataType::Utf8 => {
                    if let Some(c) = col.as_any().downcast_ref::<StringArray>() {
                        serde_json::Value::String(c.value(row_idx).to_string())
                    } else {
                        continue;
                    }
                }
                DataType::Boolean => {
                    if let Some(c) = col.as_any().downcast_ref::<BooleanArray>() {
                        serde_json::Value::Bool(c.value(row_idx))
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };

            fields.insert(name.clone(), val);
        }

        LogEvent {
            id,
            color,
            timestamp,
            timestamp_secs,
            message,
            fields,
        }
    }
}
