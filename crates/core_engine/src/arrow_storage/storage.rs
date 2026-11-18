use super::builder::ActiveRecordBatchBuilder;
use super::compiler::{ColumnView, QueryCompiler};
use super::spill::{
    align_batch_to_schema, build_unified_schema, can_segment_match, collect_projection_plan,
    scan_cold_segment, schemas_compatible, ColdCountTask, ColdSegmentMeta, ColdSpillManager,
};
use arrow::array::{
    Array, BooleanArray, Float64Array, RecordBatch, StringArray, UInt64Array, UInt8Array,
};
use arrow::compute::concat_batches;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uwu_core_filter::parser::Expr;
use uwu_core_schema::{LogColor, LogEvent, LogFields};

static NEXT_SPILL_DIR_SEQ: AtomicU64 = AtomicU64::new(1);

/// Kết quả Pha 1 cho Two-Phase Search (ưu tiên hiển thị ngay từ RAM + đếm phần đĩa ở nền).
pub struct TwoPhaseSearchResult {
    pub total_matched: usize,
    pub logs: Vec<LogEvent>,
    pub is_count_exact: bool,
}

// ============================================================================
// BatchSliceIterator: Cursor Iterator across RecordBatches
// ============================================================================

pub struct BatchSliceIterator<'a> {
    batches: &'a [RecordBatch],
    batch_idx: usize,
    passed_rows: usize,
    start_idx: usize,
    remaining_count: usize,
}

impl<'a> BatchSliceIterator<'a> {
    pub fn new(batches: &'a [RecordBatch], start_idx: usize, take_count: usize) -> Self {
        Self {
            batches,
            batch_idx: 0,
            passed_rows: 0,
            start_idx,
            remaining_count: take_count,
        }
    }
}

impl<'a> Iterator for BatchSliceIterator<'a> {
    type Item = (&'a RecordBatch, std::ops::Range<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        while self.remaining_count > 0 && self.batch_idx < self.batches.len() {
            let batch = &self.batches[self.batch_idx];
            self.batch_idx += 1;

            let b_rows = batch.num_rows();
            if self.passed_rows + b_rows <= self.start_idx {
                self.passed_rows += b_rows;
                continue;
            }

            let batch_skip = self.start_idx.saturating_sub(self.passed_rows);
            let to_take = (b_rows - batch_skip).min(self.remaining_count);
            self.passed_rows += b_rows;
            self.remaining_count -= to_take;

            return Some((batch, batch_skip..(batch_skip + to_take)));
        }
        None
    }
}

// ============================================================================
// BatchColumns: Zero-Lookup Column Cache
// ============================================================================

pub struct BatchColumns<'a> {
    batch: &'a RecordBatch,
    pub(crate) id_col: Option<&'a UInt64Array>,
    color_col: Option<&'a UInt8Array>,
    ts_col: Option<&'a StringArray>,
    pub(crate) ts_secs_col: Option<&'a Float64Array>,
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
            let val = match ColumnView::from_array(col.as_ref()) {
                ColumnView::Int64(c) => serde_json::Value::Number(c.value(row_idx).into()),
                ColumnView::Float64(c) => {
                    if let Some(n) = serde_json::Number::from_f64(c.value(row_idx)) {
                        serde_json::Value::Number(n)
                    } else {
                        continue;
                    }
                }
                ColumnView::Utf8(c) => serde_json::Value::String(c.value(row_idx).to_string()),
                ColumnView::Boolean(c) => serde_json::Value::Bool(c.value(row_idx)),
                ColumnView::UInt64(c) => serde_json::Value::Number(c.value(row_idx).into()),
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

const TARGET_BATCH_SIZE: usize = 1024;

/// Hợp nhất hai RecordBatches nếu tổng số dòng <= TARGET_BATCH_SIZE.
/// - Nếu cùng Schema: gộp trực tiếp qua Arrow concat_batches kernel.
/// - Nếu Schema khác nhau do thiếu cột động: đồng bộ hóa schema (bổ sung cột null) rồi gộp.
/// - Nếu xung đột kiểu dữ liệu hoặc quá TARGET_BATCH_SIZE: trả về None để lưu thành batch riêng.
fn unify_and_concat(a: &RecordBatch, b: &RecordBatch) -> Option<RecordBatch> {
    if a.num_rows() + b.num_rows() > TARGET_BATCH_SIZE {
        return None;
    }

    if a.schema() == b.schema() {
        return concat_batches(&a.schema(), [a, b]).ok();
    }

    let schema_a = a.schema();
    let schema_b = b.schema();
    if !schemas_compatible(&schema_a, &schema_b) {
        return None;
    }

    let unified_schema = build_unified_schema([schema_a.as_ref(), schema_b.as_ref()]);
    let aligned_a = align_batch_to_schema(a, &unified_schema)?;
    let aligned_b = align_batch_to_schema(b, &unified_schema)?;

    concat_batches(&unified_schema, [&aligned_a, &aligned_b]).ok()
}

pub struct ArrowStorage {
    max_capacity: usize,
    sealed_batches: RwLock<Arc<Vec<RecordBatch>>>,
    active_builder: Mutex<ActiveRecordBatchBuilder>,
    total_stored_rows: AtomicU64,
    total_ingested: AtomicU64,
    max_timestamp: AtomicU64,
    spill: ColdSpillManager,
}

impl ArrowStorage {
    // ------------------------------------------------------------------------
    // Lifecycle & Metrics
    // ------------------------------------------------------------------------

    pub fn new(max_capacity: usize) -> Self {
        let seq = NEXT_SPILL_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir_name = format!("sess_{}_{}", std::process::id(), seq);
        let spill_dir = uwu_core_util::app_spill_dir().join(dir_name);
        Self::new_with_spill_dir(max_capacity, spill_dir)
    }

    pub fn new_with_spill_dir(max_capacity: usize, spill_dir: PathBuf) -> Self {
        Self {
            max_capacity,
            sealed_batches: RwLock::new(Arc::new(Vec::new())),
            active_builder: Mutex::new(ActiveRecordBatchBuilder::new(TARGET_BATCH_SIZE)),
            total_stored_rows: AtomicU64::new(0),
            total_ingested: AtomicU64::new(0),
            max_timestamp: AtomicU64::new(0),
            spill: ColdSpillManager::new(spill_dir, max_capacity),
        }
    }

    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    pub fn sealed_batches_count(&self) -> usize {
        self.sealed_batches.read().len()
    }

    pub fn total_logs(&self) -> usize {
        let in_sealed = self.total_stored_rows.load(Ordering::Relaxed) as usize;
        let in_active = self.active_builder.lock().len();
        in_sealed + in_active
    }

    pub fn total_processed(&self) -> u64 {
        self.total_ingested.load(Ordering::Acquire)
    }

    pub fn total_spilled(&self) -> u64 {
        self.spill.total_spilled()
    }

    pub fn flush_spill(&self) {
        self.flush();
        self.spill.flush_to_disk();
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

        // Gộp vi batch (Micro-Batch Compaction) vào batch cuối nếu batch cuối chưa đầy (TARGET_BATCH_SIZE)
        for batch in batches {
            if batch.num_rows() == 0 {
                continue;
            }
            if let Some(last) = new_list.last_mut() {
                if let Some(merged) = unify_and_concat(last, &batch) {
                    *last = merged;
                    continue;
                }
            }
            new_list.push(batch);
        }

        let mut current_total =
            self.total_stored_rows
                .fetch_add(total_added as u64, Ordering::SeqCst) as usize
                + total_added;

        // Spill oldest batches into Cold Tier if exceeding max_capacity
        let mut evicted_batches = Vec::new();
        if current_total > self.max_capacity {
            let mut to_spill = current_total - self.max_capacity;
            let mut remove_front_count = 0;
            for batch in &new_list {
                let rows = batch.num_rows();
                if rows <= to_spill {
                    to_spill -= rows;
                    current_total -= rows;
                    remove_front_count += 1;
                } else {
                    break;
                }
            }
            if remove_front_count > 0 {
                evicted_batches.extend(new_list.drain(0..remove_front_count));
            }
            if to_spill > 0 && !new_list.is_empty() {
                let front = new_list.remove(0);
                let remaining = front.num_rows() - to_spill;
                evicted_batches.push(front.slice(0, to_spill));
                new_list.insert(0, front.slice(to_spill, remaining));
                current_total -= to_spill;
            }
            self.total_stored_rows
                .store(current_total as u64, Ordering::SeqCst);
        }

        if !evicted_batches.is_empty() {
            // Đưa vào pending_batches trước khi nhả write lock `sealed_batches` để reader không bao giờ thấy khoảng trống
            self.spill.enqueue_evicted_batches(evicted_batches);
        }

        *guard = Arc::new(new_list);
    }

    pub fn push_event(&self, event: &LogEvent) {
        if let Some(ts) = event.timestamp_secs {
            self.update_max_timestamp(ts);
        }

        let mut builder = self.active_builder.lock();
        builder.append_log(event);
        if builder.len() >= TARGET_BATCH_SIZE {
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

        // For large bulk ingestion (>= TARGET_BATCH_SIZE logs, like preloading or file import):
        // Leverage Rayon parallel workers to build Arrow columnar RecordBatches concurrently!
        if count >= TARGET_BATCH_SIZE {
            self.flush();

            let chunk_size = TARGET_BATCH_SIZE;
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
                if builder.len() >= TARGET_BATCH_SIZE {
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
        self.spill.clear();
    }

    // ------------------------------------------------------------------------
    // Search & Filtering
    // ------------------------------------------------------------------------

    fn snapshot_tiers(&self) -> (Vec<ColdSegmentMeta>, Vec<RecordBatch>) {
        self.flush();
        // Thứ tự lock cố định: sealed_batches -> spill.state để tránh deadlock và giữ nhất quán dữ liệu
        let sealed = {
            let guard = self.sealed_batches.read();
            Arc::clone(&*guard)
        };
        let (segments, mut in_mem) = {
            let st = self.spill.state.read();
            (st.segments.clone(), st.pending_batches.clone())
        };
        in_mem.extend(sealed.iter().cloned());
        (segments, in_mem)
    }

    fn eval_in_memory_window(
        in_mem: &[RecordBatch],
        expr: Option<&Expr>,
        before_id: Option<u64>,
        limit: usize,
        now: f64,
    ) -> (usize, Vec<LogEvent>) {
        let total_rows: usize = in_mem.iter().map(|b| b.num_rows()).sum();
        if total_rows == 0 || limit == 0 {
            return (0, Vec::new());
        }

        let pos = match before_id {
            Some(id) => Self::find_log_index_or_lower_bound(in_mem, id).unwrap_or(total_rows),
            None => total_rows,
        };
        if pos == 0 {
            return (0, Vec::new());
        }

        match expr {
            None => {
                let take_count = limit.min(pos);
                let start_idx = pos - take_count;
                (pos, Self::extract_row_slice(in_mem, start_idx, take_count))
            }
            Some(expr_ref) => {
                let batch_evals: Vec<(BooleanArray, usize)> = in_mem
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

                let take_matches = limit.min(total_matched);
                let mut events = Vec::with_capacity(take_matches);
                let mut current_offset = total_rows;

                for (idx, batch) in in_mem.iter().enumerate().rev() {
                    let b_rows = batch.num_rows();
                    let b_start = current_offset - b_rows;
                    current_offset = b_start;

                    if b_start >= pos {
                        continue;
                    }

                    let (mask, true_cnt) = &batch_evals[idx];
                    if *true_cnt == 0 {
                        continue;
                    }

                    let end_in_batch = (pos - b_start).min(b_rows);
                    let cols = BatchColumns::extract(batch);

                    for r in (0..end_in_batch).rev() {
                        if mask.value(r) {
                            events.push(cols.materialize_row(r));
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
        }
    }

    /// Pha 1 tìm kiếm Two-Phase (hủy sớm theo `cancel_token` / `query_id`):
    /// - Quét Hot RAM trước; nếu đủ `limit` dòng khớp, trả bảng hiển thị ngay + `ColdCountTask` đếm nền.
    /// - Nếu thiếu dòng khớp, quét ngược các Cold Segments có lọc Zone Map + Projection và dừng sớm khi đủ `limit`.
    pub fn search_phase1(
        &self,
        expr: Option<&Expr>,
        limit: usize,
        now: f64,
        cancel_token: &AtomicU64,
        query_id: u64,
    ) -> Option<(TwoPhaseSearchResult, Option<ColdCountTask>)> {
        if cancel_token.load(Ordering::Relaxed) != query_id {
            return None;
        }
        let (segments, in_mem) = self.snapshot_tiers();
        if limit == 0 || (segments.is_empty() && in_mem.is_empty()) {
            return Some((
                TwoPhaseSearchResult {
                    total_matched: 0,
                    logs: Vec::new(),
                    is_count_exact: true,
                },
                None,
            ));
        }

        let (ram_matched, ram_events) =
            Self::eval_in_memory_window(&in_mem, expr, None, limit, now);
        if cancel_token.load(Ordering::Relaxed) != query_id {
            return None;
        }

        let Some(expr_ref) = expr else {
            let cold_rows: usize = segments.iter().map(|s| s.row_count).sum();
            let mut logs = ram_events;
            if logs.len() < limit && !segments.is_empty() {
                logs = self.prepend_cold_unfiltered(&segments, None, limit, logs);
            }
            return Some((
                TwoPhaseSearchResult {
                    total_matched: cold_rows + ram_matched,
                    logs,
                    is_count_exact: true,
                },
                None,
            ));
        };

        let plan = collect_projection_plan(expr_ref);
        if ram_events.len() >= limit || segments.is_empty() {
            let is_exact = segments.is_empty();
            let task = if is_exact {
                None
            } else {
                Some(ColdCountTask {
                    segments,
                    expr: expr_ref.clone(),
                    now,
                    already_matched: ram_matched,
                    plan,
                })
            };
            return Some((
                TwoPhaseSearchResult {
                    total_matched: ram_matched,
                    logs: ram_events,
                    is_count_exact: is_exact,
                },
                task,
            ));
        }

        let mut total_matched = ram_matched;
        let mut collected_count = ram_events.len();
        let mut older_chunks: Vec<Vec<LogEvent>> = Vec::new();
        let mut remaining_end = 0usize;

        for (idx, seg) in segments.iter().enumerate().rev() {
            if cancel_token.load(Ordering::Relaxed) != query_id {
                return None;
            }
            if !can_segment_match(expr_ref, seg, now) {
                continue;
            }
            let needed = limit.saturating_sub(collected_count);
            let (seg_matched, seg_events) =
                scan_cold_segment(seg, expr_ref, now, None, needed, &plan, &self.spill.cache);
            total_matched += seg_matched;
            if !seg_events.is_empty() {
                collected_count += seg_events.len();
                older_chunks.push(seg_events);
            }
            if collected_count >= limit {
                remaining_end = idx;
                break;
            }
        }

        let mut logs = Vec::with_capacity(collected_count);
        for chunk in older_chunks.into_iter().rev() {
            logs.extend(chunk);
        }
        logs.extend(ram_events);

        let is_exact = remaining_end == 0;
        let task = if is_exact {
            None
        } else {
            Some(ColdCountTask {
                segments: segments[..remaining_end].to_vec(),
                expr: expr_ref.clone(),
                now,
                already_matched: total_matched,
                plan,
            })
        };

        Some((
            TwoPhaseSearchResult {
                total_matched,
                logs,
                is_count_exact: is_exact,
            },
            task,
        ))
    }

    fn prepend_cold_unfiltered(
        &self,
        segments: &[ColdSegmentMeta],
        before_id: Option<u64>,
        limit: usize,
        newer_events: Vec<LogEvent>,
    ) -> Vec<LogEvent> {
        let mut collected = newer_events.len();
        let mut older_chunks: Vec<Vec<LogEvent>> = Vec::new();

        for seg in segments.iter().rev() {
            if collected >= limit {
                break;
            }
            if before_id.is_some_and(|id| seg.min_id >= id) {
                continue;
            }
            let Some(batches) = self.spill.cache.get_or_load(seg) else {
                continue;
            };
            let need = limit - collected;
            let (_, seg_events) =
                Self::eval_in_memory_window(batches.as_ref(), None, before_id, need, 0.0);
            if !seg_events.is_empty() {
                collected += seg_events.len();
                older_chunks.push(seg_events);
            }
        }

        let mut result = Vec::with_capacity(collected);
        for chunk in older_chunks.into_iter().rev() {
            result.extend(chunk);
        }
        result.extend(newer_events);
        result
    }

    /// Unified Window Fetcher: Hợp nhất truy vấn cho cả cuộn xuống (search_with_count) và cuộn lên (search_before).
    /// - `before_id = None`: Lấy `limit` bản ghi mới nhất ở đáy stream (cuộn xuống, quét đầy đủ).
    /// - `before_id = Some(id)`: Lấy `limit` bản ghi nằm ngay trước `id` (cuộn lên, dừng sớm tại `limit`).
    fn fetch_window(
        &self,
        expr: Option<&Expr>,
        before_id: Option<u64>,
        limit: usize,
        now: f64,
    ) -> (usize, Vec<LogEvent>) {
        if before_id.is_none() {
            let noop_cancel = AtomicU64::new(0);
            if let Some((res, task)) = self.search_phase1(expr, limit, now, &noop_cancel, 0) {
                let exact_total = match task {
                    Some(t) => t.execute(&noop_cancel, 0).unwrap_or(res.total_matched),
                    None => res.total_matched,
                };
                return (exact_total, res.logs);
            }
            return (0, Vec::new());
        }

        let (segments, in_mem) = self.snapshot_tiers();
        if (segments.is_empty() && in_mem.is_empty()) || limit == 0 {
            return (0, Vec::new());
        }

        let (ram_total, ram_events) =
            Self::eval_in_memory_window(&in_mem, expr, before_id, limit, now);
        if ram_events.len() >= limit || segments.is_empty() {
            return (ram_total, ram_events);
        }

        let Some(expr_ref) = expr else {
            let logs = self.prepend_cold_unfiltered(&segments, before_id, limit, ram_events);
            return (logs.len(), logs);
        };

        let plan = collect_projection_plan(expr_ref);
        let mut collected = ram_events.len();
        let mut older_chunks: Vec<Vec<LogEvent>> = Vec::new();
        for seg in segments.iter().rev() {
            if collected >= limit {
                break;
            }
            if before_id.is_some_and(|id| seg.min_id >= id) {
                continue;
            }
            if !can_segment_match(expr_ref, seg, now) {
                continue;
            }
            let needed = limit - collected;
            let (_, seg_events) = scan_cold_segment(
                seg,
                expr_ref,
                now,
                before_id,
                needed,
                &plan,
                &self.spill.cache,
            );
            if !seg_events.is_empty() {
                collected += seg_events.len();
                older_chunks.push(seg_events);
            }
        }

        let mut logs = Vec::with_capacity(collected);
        for chunk in older_chunks.into_iter().rev() {
            logs.extend(chunk);
        }
        logs.extend(ram_events);
        (logs.len(), logs)
    }

    pub fn search_with_count(
        &self,
        expr: Option<&Expr>,
        limit: usize,
        now: f64,
    ) -> (usize, Vec<LogEvent>) {
        self.fetch_window(expr, None, limit, now)
    }

    pub fn search_before(
        &self,
        expr: Option<&Expr>,
        before_id: u64,
        limit: usize,
        now: f64,
    ) -> Vec<LogEvent> {
        self.fetch_window(expr, Some(before_id), limit, now).1
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

        for (batch, row_range) in BatchSliceIterator::new(&sealed, start_storage_idx, take_rows) {
            // Cắt zero-copy đúng vùng dòng mới để không eval lại các dòng cũ trong batch TARGET_BATCH_SIZE
            let sliced;
            let target_batch = if row_range.start == 0 && row_range.len() == batch.num_rows() {
                batch
            } else {
                sliced = batch.slice(row_range.start, row_range.len());
                &sliced
            };
            let mask = QueryCompiler::eval_batch(expr_ref, target_batch, now);
            if mask.true_count() == 0 {
                continue;
            }
            let cols = BatchColumns::extract(target_batch);

            for r in 0..target_batch.num_rows() {
                if mask.value(r) {
                    matched_events.push(cols.materialize_row(r));
                }
            }
        }

        let matched_len = matched_events.len();
        (matched_len, matched_events)
    }

    pub fn get_unfiltered_events(
        &self,
        target_id: Option<u64>,
        limit: usize,
    ) -> (Option<usize>, Vec<LogEvent>) {
        let (segments, in_mem) = self.snapshot_tiers();
        let cold_rows: usize = segments.iter().map(|s| s.row_count).sum();
        let in_mem_rows: usize = in_mem.iter().map(|b| b.num_rows()).sum();
        let total = cold_rows + in_mem_rows;
        if total == 0 || limit == 0 {
            return (None, Vec::new());
        }

        if let Some(target_log_id) = target_id {
            let global_pos = if let Some(pos) = Self::find_log_index_by_id(&in_mem, target_log_id) {
                Some(cold_rows + pos)
            } else {
                let mut offset = 0usize;
                let mut found = None;
                for seg in &segments {
                    if target_log_id >= seg.min_id && target_log_id <= seg.max_id {
                        if let Some(batches) = self.spill.cache.get_or_load(seg) {
                            if let Some(local_pos) =
                                Self::find_log_index_by_id(batches.as_ref(), target_log_id)
                            {
                                found = Some(offset + local_pos);
                                break;
                            }
                        }
                    }
                    offset += seg.row_count;
                }
                found
            };

            if let Some(pos) = global_pos {
                let half = limit / 2;
                let start_idx = pos.saturating_sub(half);
                let end_idx = (start_idx + limit).min(total);
                let actual_start = end_idx.saturating_sub(limit);
                let take_count = end_idx - actual_start;

                let events = self.extract_global_slice(
                    &segments,
                    &in_mem,
                    cold_rows,
                    actual_start,
                    take_count,
                );
                let target_idx = events.iter().position(|e| e.id == target_log_id);
                return (target_idx, events);
            }
        }

        // Target not specified or not found: return the last limit rows
        let skip_count = total.saturating_sub(limit);
        let take_count = total - skip_count;
        let events =
            self.extract_global_slice(&segments, &in_mem, cold_rows, skip_count, take_count);
        let target_idx = target_id.and_then(|id| events.iter().position(|e| e.id == id));
        (target_idx, events)
    }

    fn extract_global_slice(
        &self,
        segments: &[ColdSegmentMeta],
        in_mem: &[RecordBatch],
        cold_rows: usize,
        start_idx: usize,
        take_count: usize,
    ) -> Vec<LogEvent> {
        if take_count == 0 {
            return Vec::new();
        }
        let end_idx = start_idx + take_count;
        let mut events = Vec::with_capacity(take_count);

        if start_idx < cold_rows {
            let mut offset = 0usize;
            for seg in segments {
                let seg_end = offset + seg.row_count;
                if seg_end <= start_idx {
                    offset = seg_end;
                    continue;
                }
                if offset >= end_idx {
                    break;
                }
                let local_start = start_idx.saturating_sub(offset);
                let local_end = (end_idx - offset).min(seg.row_count);
                let local_take = local_end - local_start;
                if let Some(batches) = self.spill.cache.get_or_load(seg) {
                    events.extend(Self::extract_row_slice(
                        batches.as_ref(),
                        local_start,
                        local_take,
                    ));
                }
                offset = seg_end;
            }
        }

        if end_idx > cold_rows {
            let in_mem_start = start_idx.saturating_sub(cold_rows);
            let in_mem_end = end_idx - cold_rows;
            let in_mem_take = in_mem_end.saturating_sub(in_mem_start);
            events.extend(Self::extract_row_slice(in_mem, in_mem_start, in_mem_take));
        }

        events
    }

    pub fn materialize_row(batch: &RecordBatch, row_idx: usize) -> LogEvent {
        BatchColumns::extract(batch).materialize_row(row_idx)
    }

    // ------------------------------------------------------------------------
    // Internal Helpers
    // ------------------------------------------------------------------------

    /// Trích xuất và chuyển đổi (materialize) một lát cắt dòng liên tục `[start_idx..start_idx + take_count]`
    /// trên danh sách RecordBatches thông qua BatchSliceIterator.
    fn extract_row_slice(
        batches: &[RecordBatch],
        start_idx: usize,
        take_count: usize,
    ) -> Vec<LogEvent> {
        let mut events = Vec::with_capacity(take_count);
        for (batch, row_range) in BatchSliceIterator::new(batches, start_idx, take_count) {
            let cols = BatchColumns::extract(batch);
            for r in row_range {
                events.push(cols.materialize_row(r));
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

    /// Tìm vị trí dòng toàn cục (global row index) của log có ID = target_id,
    /// hoặc vị trí đầu tiên có ID >= target_id nếu target_id không tồn tại chính xác.
    fn find_log_index_or_lower_bound(batches: &[RecordBatch], target_id: u64) -> Option<usize> {
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
                if target_id < first_id {
                    return Some(global_offset);
                }
                if target_id >= first_id && target_id <= last_id {
                    for r in 0..b_rows {
                        if col.value(r) >= target_id {
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
