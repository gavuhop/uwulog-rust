use super::builder::{null_mark_col_name, strip_user_prefix, user_col_name, JSON_VALUE_META_KEY};
use super::compiler::{ColumnView, QueryCompiler};
use super::storage::BatchColumns;
use arrow::array::{new_null_array, Array, Float64Array, Int64Array, UInt64Array};
use arrow::compute::concat_batches;
use arrow::datatypes::{Field, Schema};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use uwu_core_filter::parser::{Expr, NumOp};
use uwu_core_schema::{LogEvent, StandardField};
use uwu_core_util::contains_ignore_case;

/// Kích thước mục tiêu (số dòng) cho mỗi Cold Segment ghi xuống ổ cứng.
pub const DEFAULT_COLD_SEGMENT_ROWS: usize = 16_384;
const COLD_CACHE_MAX_SEGMENTS: usize = 6;
const MAX_DISTINCT_LEVELS: usize = 32;

/// Metadata tóm tắt (Zone Map) giữ trong RAM cho mỗi Cold Segment trên ổ cứng.
/// Giúp loại bỏ toàn bộ segment không liên quan trước khi mở file Arrow IPC.
#[derive(Debug, Clone)]
pub struct ColdSegmentMeta {
    pub segment_id: u64,
    pub path: PathBuf,
    pub row_count: usize,
    pub min_id: u64,
    pub max_id: u64,
    pub min_ts: Option<f64>,
    pub max_ts: Option<f64>,
    pub schema_fields: Vec<String>,
    pub distinct_levels: Option<HashSet<String>>,
    pub numeric_bounds: HashMap<String, (f64, f64)>,
}

impl ColdSegmentMeta {
    /// Xây dựng metadata từ danh sách batches vừa hợp nhất để ghi xuống segment.
    pub fn from_batches(segment_id: u64, path: PathBuf, batches: &[RecordBatch]) -> Self {
        let mut row_count = 0usize;
        let mut min_id = u64::MAX;
        let mut max_id = 0u64;
        let mut min_ts: Option<f64> = None;
        let mut max_ts: Option<f64> = None;
        let mut schema_fields_set = HashSet::new();
        let mut levels_set = HashSet::new();
        let mut levels_overflowed = false;
        let mut has_level_column = false;
        let mut numeric_bounds: HashMap<String, (f64, f64)> = HashMap::new();

        for batch in batches {
            let n = batch.num_rows();
            if n == 0 {
                continue;
            }
            row_count += n;
            let schema = batch.schema();
            for f in schema.fields() {
                schema_fields_set.insert(f.name().clone());
            }

            let batch_cols = BatchColumns::extract(batch);
            if let Some(id_col) = batch_cols.id_col {
                if n > 0 {
                    min_id = min_id.min(id_col.value(0));
                    max_id = max_id.max(id_col.value(n - 1));
                }
            }

            if let Some(ts_col) = batch_cols.ts_secs_col {
                for i in 0..n {
                    if ts_col.is_valid(i) {
                        let v = ts_col.value(i);
                        min_ts = Some(min_ts.map_or(v, |m| m.min(v)));
                        max_ts = Some(max_ts.map_or(v, |m| m.max(v)));
                    }
                }
            }

            // Thu thập distinct_levels (nếu ít giá trị) phục vụ prune `level:...`
            if !levels_overflowed {
                if let Some(ColumnView::Utf8(str_col)) =
                    QueryCompiler::resolve_column_view(batch, "level")
                {
                    has_level_column = true;
                    for i in 0..n {
                        if str_col.is_valid(i) {
                            let val = str_col.value(i);
                            if !val.is_empty()
                                && !levels_set
                                    .iter()
                                    .any(|l: &String| l.eq_ignore_ascii_case(val))
                            {
                                levels_set.insert(val.to_lowercase());
                                if levels_set.len() > MAX_DISTINCT_LEVELS {
                                    levels_overflowed = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // Thu thập min/max cho các cột số của người dùng (`f:<key>`)
            for (idx, field) in schema.fields().iter().enumerate() {
                let Some(user_key) = strip_user_prefix(field.name()) else {
                    continue;
                };
                let col = batch.column(idx);
                if let Some((lo, hi)) = compute_numeric_bounds(col.as_ref()) {
                    numeric_bounds
                        .entry(user_key.to_string())
                        .and_modify(|(cur_lo, cur_hi)| {
                            *cur_lo = cur_lo.min(lo);
                            *cur_hi = cur_hi.max(hi);
                        })
                        .or_insert((lo, hi));
                }
            }
        }

        if min_id == u64::MAX {
            min_id = 0;
        }
        let mut schema_fields: Vec<String> = schema_fields_set.into_iter().collect();
        schema_fields.sort();

        let distinct_levels = if levels_overflowed {
            None
        } else if has_level_column {
            Some(levels_set)
        } else {
            // Segment không hề có cột level => distinct_levels rỗng để prune `level:error` thành false
            Some(HashSet::new())
        };

        Self {
            segment_id,
            path,
            row_count,
            min_id,
            max_id,
            min_ts,
            max_ts,
            schema_fields,
            distinct_levels,
            numeric_bounds,
        }
    }

    pub fn has_field(&self, field: &str) -> bool {
        if self
            .schema_fields
            .iter()
            .any(|f| strip_user_prefix(f) == Some(field))
        {
            return true;
        }
        if let Some(std_f) = StandardField::from_alias(field) {
            return match std_f {
                StandardField::Timestamp => self
                    .schema_fields
                    .iter()
                    .any(|f| f == "__timestamp" || f == "__timestamp_secs"),
                StandardField::Id => false,
                StandardField::Message => self.schema_fields.iter().any(|f| f == "__message"),
                StandardField::Level => self.schema_fields.iter().any(|f| {
                    strip_user_prefix(f).and_then(StandardField::from_alias)
                        == Some(StandardField::Level)
                }),
            };
        }
        false
    }
}

fn compute_numeric_bounds(col: &dyn Array) -> Option<(f64, f64)> {
    let n = col.len();
    if n == 0 {
        return None;
    }
    if let Some(arr) = col.as_any().downcast_ref::<Int64Array>() {
        let mut bounds: Option<(f64, f64)> = None;
        for i in 0..n {
            if arr.is_valid(i) {
                let v = arr.value(i) as f64;
                bounds = Some(bounds.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))));
            }
        }
        return bounds;
    }
    if let Some(arr) = col.as_any().downcast_ref::<UInt64Array>() {
        let mut bounds: Option<(f64, f64)> = None;
        for i in 0..n {
            if arr.is_valid(i) {
                let v = arr.value(i) as f64;
                bounds = Some(bounds.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))));
            }
        }
        return bounds;
    }
    if let Some(arr) = col.as_any().downcast_ref::<Float64Array>() {
        let mut bounds: Option<(f64, f64)> = None;
        for i in 0..n {
            if arr.is_valid(i) {
                let v = arr.value(i);
                bounds = Some(bounds.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))));
            }
        }
        return bounds;
    }
    None
}

fn numeric_bounds_for_field(meta: &ColdSegmentMeta, field: &str) -> Option<(f64, f64)> {
    match StandardField::from_alias(field) {
        Some(StandardField::Timestamp) => meta.min_ts.zip(meta.max_ts),
        _ => meta.numeric_bounds.get(field).copied(),
    }
}

#[inline]
fn range_matches_cmp(lo: f64, hi: f64, op: &NumOp, target: f64) -> bool {
    match op {
        NumOp::Gt => hi > target,
        NumOp::Gte => hi >= target,
        NumOp::Lt => lo < target,
        NumOp::Lte => lo <= target,
    }
}

#[inline]
fn range_overlaps(lo: f64, hi: f64, q_lo: f64, q_hi: f64) -> bool {
    hi >= q_lo && lo <= q_hi
}

/// Kiểm tra nhanh từ Metadata (Zone Map) xem Cold Segment có khả năng chứa log khớp `expr` hay không.
/// Trả về `false` nghĩa là chắc chắn 0 dòng khớp -> có thể bỏ qua toàn bộ file trên đĩa.
pub fn can_segment_match(expr: &Expr, meta: &ColdSegmentMeta, now: f64) -> bool {
    let _ = now;
    if meta.row_count == 0 {
        return false;
    }
    match expr {
        Expr::And(a, b) => can_segment_match(a, meta, now) && can_segment_match(b, meta, now),
        Expr::Or(a, b) => can_segment_match(a, meta, now) || can_segment_match(b, meta, now),
        // An toàn bảo thủ với NOT để không bỏ sót dòng khi field null hoặc phủ định
        Expr::Not(_) | Expr::Text(_) => true,
        Expr::FieldCmp { field, op, value } => {
            if !meta.has_field(field) {
                return false;
            }
            if let Some((lo, hi)) = numeric_bounds_for_field(meta, field) {
                return range_matches_cmp(lo, hi, op, *value);
            }
            true
        }
        Expr::FieldRange { field, lo, hi } => {
            if !meta.has_field(field) {
                return false;
            }
            if let Some((s_lo, s_hi)) = numeric_bounds_for_field(meta, field) {
                return range_overlaps(s_lo, s_hi, *lo, *hi);
            }
            true
        }
        Expr::FieldExact { field, value } => {
            if !meta.has_field(field) {
                return false;
            }
            if StandardField::from_alias(field) == Some(StandardField::Level) {
                if let Some(levels) = &meta.distinct_levels {
                    return levels.iter().any(|l| l.eq_ignore_ascii_case(value));
                }
            }
            if let Ok(num) = value.trim().parse::<f64>() {
                if let Some((lo, hi)) = numeric_bounds_for_field(meta, field) {
                    return range_overlaps(lo, hi, num, num);
                }
            }
            true
        }
        Expr::FieldContainsAny { field, values } => {
            if !meta.has_field(field) {
                return false;
            }
            if StandardField::from_alias(field) == Some(StandardField::Level) {
                if let Some(levels) = &meta.distinct_levels {
                    return values
                        .iter()
                        .any(|v| levels.iter().any(|l| contains_ignore_case(l, v)));
                }
            }
            true
        }
        Expr::FieldRegex { field, .. } => meta.has_field(field),
    }
}

/// Kế hoạch Column Projection: chỉ giải mã những cột cần thiết khi lọc trên đĩa.
#[derive(Debug, Clone)]
pub enum ProjectionPlan {
    AllColumns,
    Fields(Vec<String>),
}

pub fn collect_projection_plan(expr: &Expr) -> ProjectionPlan {
    let mut fields = HashSet::new();
    if collect_expr_fields(expr, &mut fields) {
        let mut sorted: Vec<String> = fields.into_iter().collect();
        sorted.sort();
        ProjectionPlan::Fields(sorted)
    } else {
        ProjectionPlan::AllColumns
    }
}

fn collect_expr_fields(expr: &Expr, fields: &mut HashSet<String>) -> bool {
    match expr {
        Expr::And(a, b) | Expr::Or(a, b) => {
            collect_expr_fields(a, fields) && collect_expr_fields(b, fields)
        }
        Expr::Not(inner) => collect_expr_fields(inner, fields),
        Expr::Text(_) => false,
        Expr::FieldContainsAny { field, .. }
        | Expr::FieldExact { field, .. }
        | Expr::FieldRegex { field, .. }
        | Expr::FieldCmp { field, .. }
        | Expr::FieldRange { field, .. } => {
            fields.insert(field.clone());
            true
        }
    }
}

fn push_col_index(schema_fields: &[String], indices: &mut Vec<usize>, name: &str) {
    if let Some(pos) = schema_fields.iter().position(|f| f == name) {
        if !indices.contains(&pos) {
            indices.push(pos);
        }
    }
}

pub fn resolve_projection_indices(
    schema_fields: &[String],
    plan: &ProjectionPlan,
    include_id: bool,
) -> Option<Vec<usize>> {
    let ProjectionPlan::Fields(target_fields) = plan else {
        return None;
    };

    let mut indices = Vec::new();
    if include_id {
        push_col_index(schema_fields, &mut indices, "__id");
    }

    for field in target_fields {
        push_col_index(schema_fields, &mut indices, &user_col_name(field));
        push_col_index(schema_fields, &mut indices, &null_mark_col_name(field));
        if let Some(std_f) = StandardField::from_alias(field) {
            match std_f {
                StandardField::Timestamp => {
                    push_col_index(schema_fields, &mut indices, "__timestamp");
                    push_col_index(schema_fields, &mut indices, "__timestamp_secs");
                }
                StandardField::Id => {}
                StandardField::Message => push_col_index(schema_fields, &mut indices, "__message"),
                StandardField::Level => {
                    for (pos, f) in schema_fields.iter().enumerate() {
                        if strip_user_prefix(f).and_then(StandardField::from_alias)
                            == Some(StandardField::Level)
                            && !indices.contains(&pos)
                        {
                            indices.push(pos);
                        }
                    }
                }
            }
        }
    }

    indices.sort_unstable();
    if indices.is_empty() {
        // Không có cột nào trong filter tồn tại ở segment này: chỉ đọc cột 0 (__id) để lấy đúng số dòng
        if !schema_fields.is_empty() {
            Some(vec![0])
        } else {
            None
        }
    } else if indices.len() >= schema_fields.len() {
        None
    } else {
        Some(indices)
    }
}

/// Hợp nhất và nối các RecordBatches cùng/tương thích schema thành từng nhóm không xung đột kiểu.
pub fn unify_batches_into_compatible_groups(batches: &[RecordBatch]) -> Vec<Vec<RecordBatch>> {
    let mut groups: Vec<Vec<RecordBatch>> = Vec::new();
    for batch in batches {
        if batch.num_rows() == 0 {
            continue;
        }
        let can_append = groups
            .last()
            .and_then(|g| g.first())
            .is_some_and(|first| schemas_compatible(&first.schema(), &batch.schema()));
        if can_append {
            if let Some(last_group) = groups.last_mut() {
                last_group.push(batch.clone());
            }
        } else {
            groups.push(vec![batch.clone()]);
        }
    }

    groups
        .into_iter()
        .filter_map(|group| align_group_batches(&group))
        .collect()
}

pub(crate) fn schemas_compatible(a: &Schema, b: &Schema) -> bool {
    for field_a in a.fields() {
        if let Ok(field_b) = b.field_with_name(field_a.name()) {
            if field_a.data_type() != field_b.data_type() {
                return false;
            }
        }
    }
    true
}

pub(crate) fn build_unified_schema<'a>(
    schemas: impl IntoIterator<Item = &'a Schema>,
) -> Arc<Schema> {
    let mut map: HashMap<String, (Field, bool)> = HashMap::new();
    for s in schemas {
        for f in s.fields() {
            let is_json = f
                .metadata()
                .get(JSON_VALUE_META_KEY)
                .is_some_and(|v| v == "1");
            map.entry(f.name().clone())
                .and_modify(|(existing, all_json)| {
                    *all_json &= is_json;
                    if f.is_nullable() && !existing.is_nullable() {
                        *existing = Field::new(existing.name(), existing.data_type().clone(), true)
                            .with_metadata(existing.metadata().clone());
                    }
                })
                .or_insert_with(|| (f.as_ref().clone(), is_json));
        }
    }
    let mut names: Vec<String> = map.keys().cloned().collect();
    // Sắp xếp xác định: cột hệ thống '__' trước, sau đó cột user 'f:', cuối cùng 'n:'
    names.sort_by(|x, y| {
        let rank = |name: &str| -> u8 {
            if name.starts_with("__") {
                0
            } else if strip_user_prefix(name).is_some() {
                1
            } else {
                2
            }
        };
        rank(x).cmp(&rank(y)).then_with(|| x.cmp(y))
    });
    let fields: Vec<Field> = names
        .into_iter()
        .filter_map(|n| {
            map.remove(&n).map(|(f, all_json)| {
                if !all_json && f.metadata().contains_key(JSON_VALUE_META_KEY) {
                    let mut meta = f.metadata().clone();
                    meta.remove(JSON_VALUE_META_KEY);
                    f.with_metadata(meta)
                } else {
                    f
                }
            })
        })
        .collect();
    Arc::new(Schema::new(fields))
}

pub(crate) fn align_batch_to_schema(
    batch: &RecordBatch,
    unified_schema: &Arc<Schema>,
) -> Option<RecordBatch> {
    if batch.schema().as_ref() == unified_schema.as_ref() {
        return Some(batch.clone());
    }
    let src_schema = batch.schema();
    let idx_map: HashMap<&str, usize> = src_schema
        .fields()
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name().as_str(), i))
        .collect();
    let mut cols = Vec::with_capacity(unified_schema.fields().len());
    for field in unified_schema.fields() {
        if let Some(&idx) = idx_map.get(field.name().as_str()) {
            cols.push(Arc::clone(batch.column(idx)));
        } else {
            cols.push(new_null_array(field.data_type(), batch.num_rows()));
        }
    }
    RecordBatch::try_new(Arc::clone(unified_schema), cols).ok()
}

fn align_group_batches(group: &[RecordBatch]) -> Option<Vec<RecordBatch>> {
    if group.is_empty() {
        return None;
    }
    let unified_schema = build_unified_schema(group.iter().map(|b| b.schema_ref().as_ref()));
    let aligned: Vec<RecordBatch> = group
        .iter()
        .filter_map(|b| align_batch_to_schema(b, &unified_schema))
        .collect();
    if aligned.is_empty() {
        return None;
    }
    // Gộp chung thành 1 batch lớn (nếu <= 65536 dòng) hoặc giữ danh sách aligned cùng schema
    if let Ok(merged) = concat_batches(&unified_schema, aligned.iter()) {
        Some(vec![merged])
    } else {
        Some(aligned)
    }
}

/// Ghi một nhóm `RecordBatch` cùng schema ra file Arrow IPC.
pub fn write_ipc_segment(path: &Path, batches: &[RecordBatch]) -> anyhow::Result<()> {
    if batches.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let schema = batches[0].schema();
    let mut writer = FileWriter::try_new(file, &schema)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;
    Ok(())
}

/// Đọc Cold Segment từ đĩa với tùy chọn Column Projection.
pub fn read_ipc_segment(
    path: &Path,
    projection: Option<Vec<usize>>,
) -> anyhow::Result<(Vec<String>, Vec<RecordBatch>)> {
    let file = File::open(path)?;
    let reader = FileReader::try_new(BufReader::new(file), projection)?;
    let schema_fields = reader
        .schema()
        .fields()
        .iter()
        .map(|f| f.name().clone())
        .collect();
    let mut batches = Vec::new();
    for res in reader {
        batches.push(res?);
    }
    Ok((schema_fields, batches))
}

/// LRU Cache kích thước nhỏ giữ các Cold Segment vừa đọc full để cuộn ngược/Raw Context mượt.
pub struct ColdBatchCache {
    entries: Mutex<Vec<(u64, Arc<Vec<RecordBatch>>)>>,
}

impl Default for ColdBatchCache {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Vec::with_capacity(COLD_CACHE_MAX_SEGMENTS)),
        }
    }
}

impl ColdBatchCache {
    pub fn get_or_load(&self, meta: &ColdSegmentMeta) -> Option<Arc<Vec<RecordBatch>>> {
        {
            let mut cache = self.entries.lock();
            if let Some(pos) = cache.iter().position(|(id, _)| *id == meta.segment_id) {
                let entry = cache.remove(pos);
                let arc = Arc::clone(&entry.1);
                cache.push(entry);
                return Some(arc);
            }
        }

        let (_, batches) = read_ipc_segment(&meta.path, None).ok()?;
        let arc = Arc::new(batches);
        let mut cache = self.entries.lock();
        if let Some(pos) = cache.iter().position(|(id, _)| *id == meta.segment_id) {
            cache.remove(pos);
        }
        if cache.len() >= COLD_CACHE_MAX_SEGMENTS {
            cache.remove(0);
        }
        cache.push((meta.segment_id, Arc::clone(&arc)));
        Some(arc)
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
    }
}

/// Đánh giá 1 Cold Segment với Column Projection Pushdown:
/// - Nếu chỉ cần đếm (`collect_limit == 0`), chỉ đọc đúng các cột có trong filter.
/// - Nếu cần thu thập dòng (`collect_limit > 0`), bước 1 đọc cột filter; bước 2 chỉ đọc full những batch thực sự có match.
pub fn scan_cold_segment(
    meta: &ColdSegmentMeta,
    expr: &Expr,
    now: f64,
    before_id: Option<u64>,
    collect_limit: usize,
    plan: &ProjectionPlan,
    cache: &ColdBatchCache,
) -> (usize, Vec<LogEvent>) {
    let proj_indices = resolve_projection_indices(&meta.schema_fields, plan, before_id.is_some());

    // Khi cần đọc tất cả cột (vd Text search) hoặc lấy hết -> dùng cache full để tái sử dụng nếu cuộn lại
    if proj_indices.is_none() {
        let Some(batches) = cache.get_or_load(meta) else {
            return (0, Vec::new());
        };
        return eval_segment_batches(batches.as_ref(), expr, now, before_id, collect_limit);
    }

    let Ok((_, proj_batches)) = read_ipc_segment(&meta.path, proj_indices) else {
        return (0, Vec::new());
    };

    let mut total_matched = 0usize;
    let mut matched_batch_masks: Vec<(usize, arrow::array::BooleanArray, usize)> = Vec::new();

    for (b_idx, batch) in proj_batches.iter().enumerate() {
        let effective_rows = effective_row_limit_by_id(batch, before_id);
        if effective_rows == 0 {
            continue;
        }
        let mask = QueryCompiler::eval_batch(expr, batch, now);
        let cnt = if effective_rows == batch.num_rows() {
            mask.true_count()
        } else {
            (0..effective_rows).filter(|&r| mask.value(r)).count()
        };
        if cnt > 0 {
            total_matched += cnt;
            if collect_limit > 0 {
                matched_batch_masks.push((b_idx, mask, effective_rows));
            }
        }
    }

    if total_matched == 0 || collect_limit == 0 {
        return (total_matched, Vec::new());
    }

    // Bước 2: Chỉ đọc toàn bộ cột để materialize đúng những dòng khớp (từ cuối về đầu)
    let Some(full_batches) = cache.get_or_load(meta) else {
        return (total_matched, Vec::new());
    };
    let mut events = Vec::with_capacity(collect_limit.min(total_matched));
    for (b_idx, mask, effective_rows) in matched_batch_masks.into_iter().rev() {
        let Some(full_batch) = full_batches.get(b_idx) else {
            continue;
        };
        let cols = BatchColumns::extract(full_batch);
        for r in (0..effective_rows).rev() {
            if mask.value(r) {
                events.push(cols.materialize_row(r));
                if events.len() >= collect_limit {
                    break;
                }
            }
        }
        if events.len() >= collect_limit {
            break;
        }
    }
    events.reverse();
    (total_matched, events)
}

fn eval_segment_batches(
    batches: &[RecordBatch],
    expr: &Expr,
    now: f64,
    before_id: Option<u64>,
    collect_limit: usize,
) -> (usize, Vec<LogEvent>) {
    let mut total_matched = 0usize;
    let mut matched_masks: Vec<(usize, arrow::array::BooleanArray, usize)> = Vec::new();

    for (idx, batch) in batches.iter().enumerate() {
        let effective_rows = effective_row_limit_by_id(batch, before_id);
        if effective_rows == 0 {
            continue;
        }
        let mask = QueryCompiler::eval_batch(expr, batch, now);
        let cnt = if effective_rows == batch.num_rows() {
            mask.true_count()
        } else {
            (0..effective_rows).filter(|&r| mask.value(r)).count()
        };
        if cnt > 0 {
            total_matched += cnt;
            if collect_limit > 0 {
                matched_masks.push((idx, mask, effective_rows));
            }
        }
    }

    if total_matched == 0 || collect_limit == 0 {
        return (total_matched, Vec::new());
    }

    let mut events = Vec::with_capacity(collect_limit.min(total_matched));
    for (idx, mask, effective_rows) in matched_masks.into_iter().rev() {
        let cols = BatchColumns::extract(&batches[idx]);
        for r in (0..effective_rows).rev() {
            if mask.value(r) {
                events.push(cols.materialize_row(r));
                if events.len() >= collect_limit {
                    break;
                }
            }
        }
        if events.len() >= collect_limit {
            break;
        }
    }
    events.reverse();
    (total_matched, events)
}

pub(crate) fn effective_row_limit_by_id(batch: &RecordBatch, before_id: Option<u64>) -> usize {
    let n = batch.num_rows();
    let Some(target) = before_id else {
        return n;
    };
    let Some(id_col) = BatchColumns::extract(batch).id_col else {
        return n;
    };
    if n == 0 || id_col.value(0) >= target {
        return 0;
    }
    if id_col.value(n - 1) < target {
        return n;
    }
    let slice = id_col.values();
    slice.partition_point(|&id| id < target)
}

/// Trạng thái Cold Tier gồm các Cold Segments trên đĩa và `pending_batches` đang chờ ghi.
pub struct ColdTierState {
    pub segments: Vec<ColdSegmentMeta>,
    pub pending_batches: Vec<RecordBatch>,
    pub total_cold_rows: u64,
    pub total_spilled_rows: u64,
    pub generation: u64,
    pub next_segment_id: u64,
    pub spill_dir: PathBuf,
}

enum SpillCmd {
    CheckPending,
    Flush(mpsc::Sender<()>),
    Shutdown,
}

/// Quản lý luồng nền ghi các batch tràn RAM xuống thư mục phiên dưới dạng Arrow IPC.
pub struct ColdSpillManager {
    pub state: Arc<RwLock<ColdTierState>>,
    pub cache: Arc<ColdBatchCache>,
    segment_target_rows: usize,
    cmd_tx: Mutex<Option<mpsc::Sender<SpillCmd>>>,
    worker_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl ColdSpillManager {
    pub fn new(spill_dir: PathBuf, max_capacity: usize) -> Self {
        let segment_target_rows = if max_capacity < DEFAULT_COLD_SEGMENT_ROWS * 2 {
            (max_capacity / 2).clamp(1, DEFAULT_COLD_SEGMENT_ROWS)
        } else {
            DEFAULT_COLD_SEGMENT_ROWS
        };
        let state = Arc::new(RwLock::new(ColdTierState {
            segments: Vec::new(),
            pending_batches: Vec::new(),
            total_cold_rows: 0,
            total_spilled_rows: 0,
            generation: 0,
            next_segment_id: 1,
            spill_dir,
        }));
        Self {
            state,
            cache: Arc::new(ColdBatchCache::default()),
            segment_target_rows,
            cmd_tx: Mutex::new(None),
            worker_handle: Mutex::new(None),
        }
    }

    fn ensure_worker(&self) -> Option<mpsc::Sender<SpillCmd>> {
        let mut tx_guard = self.cmd_tx.lock();
        if let Some(ref tx) = *tx_guard {
            return Some(tx.clone());
        }

        let (tx, rx) = mpsc::channel::<SpillCmd>();
        let state_ref = Arc::clone(&self.state);
        let min_rows = self.segment_target_rows;

        let handle = std::thread::Builder::new()
            .name("uwu-spill-writer".to_string())
            .spawn(move || {
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        SpillCmd::CheckPending => {
                            drain_pending_to_disk(&state_ref, min_rows);
                        }
                        SpillCmd::Flush(ack) => {
                            drain_pending_to_disk(&state_ref, 1);
                            let _ = ack.send(());
                        }
                        SpillCmd::Shutdown => break,
                    }
                }
            })
            .ok()?;

        *self.worker_handle.lock() = Some(handle);
        *tx_guard = Some(tx.clone());
        Some(tx)
    }

    /// Đẩy các batch bị loại khỏi Hot RAM vào `pending_batches` và đánh thức writer nền khi đủ kích thước segment.
    pub fn enqueue_evicted_batches(&self, batches: Vec<RecordBatch>) {
        if batches.is_empty() {
            return;
        }
        let added_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        if added_rows == 0 {
            return;
        }

        let pending_rows = {
            let mut st = self.state.write();
            st.pending_batches.extend(batches);
            st.total_cold_rows += added_rows as u64;
            st.total_spilled_rows += added_rows as u64;
            st.pending_batches
                .iter()
                .map(|b| b.num_rows())
                .sum::<usize>()
        };

        if pending_rows >= self.segment_target_rows {
            if let Some(tx) = self.ensure_worker() {
                let _ = tx.send(SpillCmd::CheckPending);
            }
        }
    }

    /// Ép ghi toàn bộ `pending_batches` xuống đĩa đồng bộ (dùng cho test hoặc khi cần flush tức thì).
    pub fn flush_to_disk(&self) {
        let has_pending = !self.state.read().pending_batches.is_empty();
        if !has_pending {
            return;
        }
        if let Some(tx) = self.ensure_worker() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if tx.send(SpillCmd::Flush(ack_tx)).is_ok() {
                let _ = ack_rx.recv();
            }
        }
    }

    /// Xóa toàn bộ dữ liệu Cold Tier & thư mục spill trên đĩa (khi Clear/Restart logs).
    pub fn clear(&self) {
        let spill_dir = {
            let mut st = self.state.write();
            st.generation += 1;
            st.segments.clear();
            st.pending_batches.clear();
            st.total_cold_rows = 0;
            st.total_spilled_rows = 0;
            st.next_segment_id = 1;
            st.spill_dir.clone()
        };
        self.cache.clear();
        let _ = fs::remove_dir_all(&spill_dir);
    }

    pub fn total_spilled(&self) -> u64 {
        self.state.read().total_spilled_rows
    }

    pub fn cold_rows_count(&self) -> u64 {
        self.state.read().total_cold_rows
    }
}

impl Drop for ColdSpillManager {
    fn drop(&mut self) {
        if let Some(tx) = self.cmd_tx.lock().take() {
            let _ = tx.send(SpillCmd::Shutdown);
        }
        if let Some(handle) = self.worker_handle.lock().take() {
            let _ = handle.join();
        }
        self.cache.clear();
        let dir = self.state.read().spill_dir.clone();
        let _ = fs::remove_dir_all(dir);
    }
}

fn drain_pending_to_disk(state_ref: &Arc<RwLock<ColdTierState>>, min_batch_rows: usize) {
    loop {
        let snapshot = {
            let st = state_ref.read();
            let pending_rows: usize = st.pending_batches.iter().map(|b| b.num_rows()).sum();
            if pending_rows < min_batch_rows || st.pending_batches.is_empty() {
                return;
            }
            // Gom tối đa ~DEFAULT_COLD_SEGMENT_ROWS dòng trong một lượt ghi segment
            let mut take_count = 0usize;
            let mut acc_rows = 0usize;
            for b in &st.pending_batches {
                take_count += 1;
                acc_rows += b.num_rows();
                if acc_rows >= DEFAULT_COLD_SEGMENT_ROWS {
                    break;
                }
            }
            (
                st.generation,
                st.spill_dir.clone(),
                st.pending_batches[..take_count].to_vec(),
                take_count,
            )
        };

        let (gen, spill_dir, chunk, take_count) = snapshot;
        let groups = unify_batches_into_compatible_groups(&chunk);
        if groups.is_empty() {
            let mut st = state_ref.write();
            if st.generation == gen && st.pending_batches.len() >= take_count {
                st.pending_batches.drain(0..take_count);
            }
            continue;
        }

        let mut created_metas = Vec::with_capacity(groups.len());
        let base_seg_id = {
            let mut st = state_ref.write();
            if st.generation != gen {
                continue;
            }
            let base = st.next_segment_id;
            st.next_segment_id += groups.len() as u64;
            base
        };

        let mut write_failed = false;
        for (offset, group_batches) in groups.into_iter().enumerate() {
            let seg_id = base_seg_id + offset as u64;
            let path = spill_dir.join(format!("seg_{:06}.arrow", seg_id));
            if write_ipc_segment(&path, &group_batches).is_err() {
                write_failed = true;
                break;
            }
            created_metas.push(ColdSegmentMeta::from_batches(seg_id, path, &group_batches));
        }

        if write_failed {
            for m in created_metas {
                let _ = fs::remove_file(&m.path);
            }
            return;
        }

        let mut st = state_ref.write();
        if st.generation != gen {
            drop(st);
            for m in created_metas {
                let _ = fs::remove_file(&m.path);
            }
            continue;
        }
        if st.pending_batches.len() >= take_count {
            st.pending_batches.drain(0..take_count);
        }
        st.segments.extend(created_metas);
    }
}

/// Tác vụ nền đếm nốt số dòng khớp trên các Cold Segments còn lại (Pha 2 của Two-Phase Search).
pub struct ColdCountTask {
    pub segments: Vec<ColdSegmentMeta>,
    pub expr: Expr,
    pub now: f64,
    pub already_matched: usize,
    pub plan: ProjectionPlan,
}

impl ColdCountTask {
    pub fn execute(self, cancel_token: &AtomicU64, query_id: u64) -> Option<usize> {
        let mut total = self.already_matched;
        for seg in self.segments.iter().rev() {
            if cancel_token.load(Ordering::Relaxed) != query_id {
                return None;
            }
            if !can_segment_match(&self.expr, seg, self.now) {
                continue;
            }
            let proj = resolve_projection_indices(&seg.schema_fields, &self.plan, false);
            if let Ok((_, batches)) = read_ipc_segment(&seg.path, proj) {
                for batch in &batches {
                    if cancel_token.load(Ordering::Relaxed) != query_id {
                        return None;
                    }
                    total += QueryCompiler::eval_batch(&self.expr, batch, self.now).true_count();
                }
            }
        }
        Some(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwu_core_filter::parser::{tokenize, Parser};

    fn parse(q: &str, now: f64) -> Expr {
        let tokens = tokenize(q);
        Parser::new(tokens, now).parse().unwrap()
    }

    #[test]
    fn test_zone_map_pruning_and_projection_plan() {
        let meta = ColdSegmentMeta {
            segment_id: 1,
            path: PathBuf::from("seg_000001.arrow"),
            row_count: 100,
            min_id: 1,
            max_id: 100,
            min_ts: Some(1000.0),
            max_ts: Some(2000.0),
            schema_fields: vec![
                "__id".into(),
                "__color".into(),
                "__timestamp".into(),
                "__timestamp_secs".into(),
                "__message".into(),
                "f:level".into(),
                "f:status".into(),
            ],
            distinct_levels: Some(["info".to_string(), "warn".to_string()].into()),
            numeric_bounds: [("status".to_string(), (200.0, 404.0))].into(),
        };

        // 1. Level không có trong segment -> prune thành false
        assert!(!can_segment_match(
            &parse("level:error", 3000.0),
            &meta,
            3000.0
        ));
        assert!(can_segment_match(
            &parse("level:warn", 3000.0),
            &meta,
            3000.0
        ));

        // 2. Numeric bounds -> prune status>=500
        assert!(!can_segment_match(
            &parse("status>=500", 3000.0),
            &meta,
            3000.0
        ));
        assert!(can_segment_match(
            &parse("status>=400", 3000.0),
            &meta,
            3000.0
        ));

        // 3. Time range pruning
        assert!(!can_segment_match(
            &parse("time>2500", 3000.0),
            &meta,
            3000.0
        ));
        assert!(can_segment_match(
            &parse("time:1500..2500", 3000.0),
            &meta,
            3000.0
        ));

        // 4. Projection plan bỏ qua __message khi query chỉ dùng level & status
        let plan = collect_projection_plan(&parse("level:error status>=500", 3000.0));
        let proj = resolve_projection_indices(&meta.schema_fields, &plan, false).unwrap();
        let selected: Vec<&str> = proj
            .iter()
            .map(|&i| meta.schema_fields[i].as_str())
            .collect();
        assert_eq!(selected, vec!["f:level", "f:status"]);
        assert!(!selected.contains(&"__message"));
    }
}
