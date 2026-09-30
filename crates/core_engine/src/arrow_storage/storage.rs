use super::builder::ActiveRecordBatchBuilder;
use super::compiler::QueryCompiler;
use arrow::array::{
    Array, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array,
    UInt8Array,
};
use arrow::datatypes::DataType;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uwu_core_filter::parser::Expr;
use uwu_core_schema::{LogColor, LogEvent, LogFields};

// ============================================================================
// BatchColumns: Zero-Lookup Column Cache
// ============================================================================

pub struct BatchColumns<'a> {
    batch: &'a RecordBatch,
    id_col: Option<&'a UInt64Array>,
    color_col: Option<&'a UInt8Array>,
    ts_col: Option<&'a StringArray>,
    ts_secs_col: Option<&'a Float64Array>,
    msg_col: Option<&'a StringArray>,
    dynamic_indices: Vec<usize>,
}

impl<'a> BatchColumns<'a> {
    pub fn extract(batch: &'a RecordBatch) -> Self {
        let id_col = batch
            .column_by_name("__id")
            .and_then(|c| c.as_any().downcast_ref::<UInt64Array>());
        let color_col = batch
            .column_by_name("__color")
            .and_then(|c| c.as_any().downcast_ref::<UInt8Array>());
        let ts_col = batch
            .column_by_name("__timestamp")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let ts_secs_col = batch
            .column_by_name("__timestamp_secs")
            .and_then(|c| c.as_any().downcast_ref::<Float64Array>());
        let msg_col = batch
            .column_by_name("__message")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());

        let mut dynamic_indices = Vec::new();
        for (col_idx, field) in batch.schema().fields().iter().enumerate() {
            if !field.name().starts_with("__") {
                dynamic_indices.push(col_idx);
            }
        }

        Self {
            batch,
            id_col,
            color_col,
            ts_col,
            ts_secs_col,
            msg_col,
            dynamic_indices,
        }
    }

    pub fn materialize_row(&self, row_idx: usize) -> LogEvent {
        let id = self.id_col.map(|c| c.value(row_idx)).unwrap_or(0);
        let color = self
            .color_col
            .map(|c| match c.value(row_idx) {
                0 => LogColor::Red,
                1 => LogColor::Yellow,
                2 => LogColor::Green,
                3 => LogColor::Gray,
                _ => LogColor::Default,
            })
            .unwrap_or(LogColor::Default);

        let timestamp = self
            .ts_col
            .filter(|c| c.is_valid(row_idx))
            .map(|c| c.value(row_idx).to_string())
            .unwrap_or_default();

        let timestamp_secs = self
            .ts_secs_col
            .filter(|c| c.is_valid(row_idx))
            .map(|c| c.value(row_idx));

        let message = self
            .msg_col
            .filter(|c| c.is_valid(row_idx))
            .map(|c| c.value(row_idx).to_string())
            .unwrap_or_default();

        let mut fields = LogFields::with_capacity(self.dynamic_indices.len());
        let schema = self.batch.schema();
        for &col_idx in &self.dynamic_indices {
            let col = self.batch.column(col_idx);
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
                DataType::UInt64 => {
                    if let Some(c) = col.as_any().downcast_ref::<UInt64Array>() {
                        serde_json::Value::Number(c.value(row_idx).into())
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };
            fields.insert(schema.field(col_idx).name().clone(), val);
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

// ============================================================================
// ArrowStorage: Concurrent Columnar In-Memory Store
// ============================================================================

pub struct ArrowStorage {
    max_capacity: usize,
    sealed_batches: RwLock<Arc<Vec<RecordBatch>>>,
    active_builder: Mutex<ActiveRecordBatchBuilder>,
    total_stored_rows: AtomicU64,
    total_ingested: AtomicU64,
    max_timestamp: AtomicU64,
}

impl ArrowStorage {
    // ------------------------------------------------------------------------
    // Lifecycle & Metrics
    // ------------------------------------------------------------------------

    pub fn new(max_capacity: usize) -> Self {
        Self {
            max_capacity,
            sealed_batches: RwLock::new(Arc::new(Vec::new())),
            active_builder: Mutex::new(ActiveRecordBatchBuilder::new(4096)),
            total_stored_rows: AtomicU64::new(0),
            total_ingested: AtomicU64::new(0),
            max_timestamp: AtomicU64::new(0),
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
        f64::from_bits(self.max_timestamp.load(Ordering::Acquire))
    }

    #[inline]
    pub fn update_max_timestamp(&self, ts: f64) {
        if ts <= 0.0 {
            return;
        }
        let mut current = self.max_timestamp.load(Ordering::Relaxed);
        while ts > f64::from_bits(current) {
            match self.max_timestamp.compare_exchange_weak(
                current,
                ts.to_bits(),
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    // ------------------------------------------------------------------------
    // Ingestion & Batch Sealing
    // ------------------------------------------------------------------------

    pub fn flush(&self) {
        let mut builder = self.active_builder.lock();
        if !builder.is_empty() {
            if let Some(batch) = builder.seal() {
                self.append_sealed_batch(batch);
            }
        }
    }

    pub fn append_sealed_batch(&self, batch: RecordBatch) {
        self.append_sealed_batches(vec![batch]);
    }

    pub fn append_sealed_batches(&self, batches: Vec<RecordBatch>) {
        if batches.is_empty() {
            return;
        }

        let total_added: usize = batches.iter().map(|b| b.num_rows()).sum();
        if total_added == 0 {
            return;
        }

        let mut guard = self.sealed_batches.write();
        let mut new_list = (**guard).clone();
        new_list.extend(batches);

        let mut current_total =
            self.total_stored_rows
                .fetch_add(total_added as u64, Ordering::SeqCst) as usize
                + total_added;

        // Ring buffer trim if exceeding max_capacity
        if current_total > self.max_capacity {
            let mut to_drop = current_total - self.max_capacity;
            let mut remove_front_count = 0;
            for batch in &new_list {
                let rows = batch.num_rows();
                if rows <= to_drop {
                    to_drop -= rows;
                    current_total -= rows;
                    remove_front_count += 1;
                } else {
                    break;
                }
            }
            if remove_front_count > 0 {
                new_list.drain(0..remove_front_count);
            }
            if to_drop > 0 && !new_list.is_empty() {
                let front = new_list.remove(0);
                let remaining = front.num_rows() - to_drop;
                let sliced = front.slice(to_drop, remaining);
                new_list.insert(0, sliced);
                current_total -= to_drop;
            }
            self.total_stored_rows
                .store(current_total as u64, Ordering::SeqCst);
        }

        *guard = Arc::new(new_list);
    }

    pub fn push_event(&self, event: &LogEvent) {
        if let Some(ts) = event.timestamp_secs {
            self.update_max_timestamp(ts);
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
            self.update_max_timestamp(batch_max_ts);
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

            self.append_sealed_batches(new_batches);
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
        }

        self.total_ingested
            .fetch_add(count as u64, Ordering::Release);
    }

    pub fn clear(&self) {
        let mut builder = self.active_builder.lock();
        let _ = builder.seal(); // Discard active rows
        let mut sealed = self.sealed_batches.write();
        *sealed = Arc::new(Vec::new());
        self.total_stored_rows.store(0, Ordering::SeqCst);
        self.total_ingested.store(0, Ordering::SeqCst);
        self.max_timestamp.store(0, Ordering::SeqCst);
    }

    // ------------------------------------------------------------------------
    // Search & Filtering
    // ------------------------------------------------------------------------

    pub fn search_with_count(
        &self,
        expr: Option<&Expr>,
        limit: usize,
        now: f64,
    ) -> (usize, Vec<LogEvent>) {
        self.flush();

        // 1. Snapshot Read: Clone Arc in ~10 nanoseconds, releasing the lock immediately!
        let sealed = {
            let guard = self.sealed_batches.read();
            Arc::clone(&*guard)
        };
        if sealed.is_empty() {
            return (0, Vec::new());
        }

        match expr {
            None => self.search_unfiltered(&sealed, limit),
            Some(expr_ref) => self.search_filtered(&sealed, expr_ref, limit, now),
        }
    }

    fn search_unfiltered(&self, sealed: &[RecordBatch], limit: usize) -> (usize, Vec<LogEvent>) {
        let total_rows = self.total_stored_rows.load(Ordering::Acquire) as usize;
        if total_rows == 0 {
            return (0, Vec::new());
        }

        let take_count = limit.min(total_rows);
        let mut events = Vec::with_capacity(take_count);

        // Reverse scan from newest batch backwards for viewport!
        for batch in sealed.iter().rev() {
            let b_rows = batch.num_rows();
            if b_rows == 0 {
                continue;
            }
            let needed = take_count - events.len();
            let to_take = b_rows.min(needed);
            let start_idx = b_rows - to_take;

            let cols = BatchColumns::extract(batch);
            for r in (start_idx..b_rows).rev() {
                events.push(cols.materialize_row(r));
            }

            if events.len() >= take_count {
                break;
            }
        }

        events.reverse();
        (total_rows, events)
    }

    fn search_filtered(
        &self,
        sealed: &[RecordBatch],
        expr: &Expr,
        limit: usize,
        now: f64,
    ) -> (usize, Vec<LogEvent>) {
        // Parallel SIMD Vectorized Evaluation across batches outside of ANY lock
        let batch_evals: Vec<(BooleanArray, usize)> = sealed
            .par_iter()
            .map(|batch| {
                let mask = QueryCompiler::eval_batch(expr, batch, now);
                let true_cnt = mask.true_count();
                (mask, true_cnt)
            })
            .collect();

        let total_matched: usize = batch_evals.iter().map(|(_, c)| *c).sum();
        if total_matched == 0 {
            return (0, Vec::new());
        }

        let take_matches = limit.min(total_matched);
        let mut events = Vec::with_capacity(take_matches);

        // Late Materialization: Reverse scan from newest batch backwards!
        for (batch_idx, batch) in sealed.iter().enumerate().rev() {
            let (mask, true_cnt) = &batch_evals[batch_idx];
            if *true_cnt == 0 {
                continue;
            }

            let cols = BatchColumns::extract(batch);
            let b_rows = batch.num_rows();

            for row_idx in (0..b_rows).rev() {
                if mask.value(row_idx) {
                    events.push(cols.materialize_row(row_idx));
                    if events.len() >= take_matches {
                        break;
                    }
                }
            }

            if events.len() >= take_matches {
                break;
            }
        }

        events.reverse();
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
        let sealed = {
            let guard = self.sealed_batches.read();
            Arc::clone(&*guard)
        };
        let total_in_storage: usize = sealed.iter().map(|b| b.num_rows()).sum();
        if total_in_storage == 0 {
            return (0, Vec::new());
        }

        let take_rows = new_count.min(total_in_storage);
        let start_storage_idx = total_in_storage.saturating_sub(take_rows);

        // Fast path: Empty query (trả về các bản ghi mới nhất vừa nạp)
        if expr.is_none() {
            let events = Self::extract_row_slice(&sealed, start_storage_idx, take_rows);
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
            let cols = BatchColumns::extract(batch);

            for r in batch_skip..b_rows {
                if mask.value(r) {
                    matched_events.push(cols.materialize_row(r));
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

        let sealed = {
            let guard = self.sealed_batches.read();
            Arc::clone(&*guard)
        };
        let total: usize = sealed.iter().map(|b| b.num_rows()).sum();
        if total == 0 {
            return (None, Vec::new());
        }

        if let Some(target_log_id) = target_id {
            if let Some(pos) = Self::find_log_index_by_id(&sealed, target_log_id) {
                let half = limit / 2;
                let start_idx = pos.saturating_sub(half);
                let end_idx = (start_idx + limit).min(total);
                let actual_start = end_idx.saturating_sub(limit);
                let take_count = end_idx - actual_start;

                let events = Self::extract_row_slice(&sealed, actual_start, take_count);
                let target_idx = events.iter().position(|e| e.id == target_log_id);
                return (target_idx, events);
            }
        }

        // Target not specified or not found: return the last limit rows
        let skip_count = total.saturating_sub(limit);
        let take_count = total - skip_count;
        let events = Self::extract_row_slice(&sealed, skip_count, take_count);
        let target_idx = target_id.and_then(|id| events.iter().position(|e| e.id == id));
        (target_idx, events)
    }

    pub fn materialize_row(batch: &RecordBatch, row_idx: usize) -> LogEvent {
        BatchColumns::extract(batch).materialize_row(row_idx)
    }

    // ------------------------------------------------------------------------
    // Internal Helpers
    // ------------------------------------------------------------------------

    /// Trích xuất và chuyển đổi (materialize) một lát cắt dòng liên tục `[start_idx..start_idx + take_count]`
    /// trên danh sách RecordBatches mà không cấp phát thừa bộ nhớ.
    fn extract_row_slice(
        batches: &[RecordBatch],
        start_idx: usize,
        take_count: usize,
    ) -> Vec<LogEvent> {
        let mut events = Vec::with_capacity(take_count);
        let mut passed_rows = 0;

        for batch in batches {
            let b_rows = batch.num_rows();
            if passed_rows + b_rows <= start_idx {
                passed_rows += b_rows;
                continue;
            }

            let batch_skip = start_idx.saturating_sub(passed_rows);
            let to_take = (b_rows - batch_skip).min(take_count - events.len());
            let cols = BatchColumns::extract(batch);

            for r in batch_skip..(batch_skip + to_take) {
                events.push(cols.materialize_row(r));
            }
            passed_rows += b_rows;

            if events.len() >= take_count {
                break;
            }
        }

        events
    }

    /// Tìm chỉ số toàn cục (global row index) của một log ID chỉ định trong danh sách batch.
    fn find_log_index_by_id(batches: &[RecordBatch], target_id: u64) -> Option<usize> {
        let mut global_offset = 0;
        for batch in batches {
            let b_rows = batch.num_rows();
            if b_rows == 0 {
                continue;
            }
            if let Some(col) = batch
                .column_by_name("__id")
                .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
            {
                let first_id = col.value(0);
                let last_id = col.value(b_rows - 1);
                if target_id >= first_id && target_id <= last_id {
                    for r in 0..b_rows {
                        if col.value(r) == target_id {
                            return Some(global_offset + r);
                        }
                    }
                }
            }
            global_offset += b_rows;
        }
        None
    }
}
