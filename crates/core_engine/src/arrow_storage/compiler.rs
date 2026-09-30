use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array,
};
use arrow::buffer::BooleanBuffer;
use arrow::compute::kernels::cmp::eq;
use arrow::datatypes::DataType;
use std::io::Write;
use uwu_core_filter::parser::{Expr, NumOp};
use uwu_core_filter::Regex;
use uwu_core_schema::StandardField;
use uwu_core_util::{contains_ignore_case, contains_ignore_case_ascii_bytes, parse_numeric_value};

// ============================================================================
// ColumnView: Typed Column Dispatcher (Zero-Cost Enum Wrapper)
// ============================================================================

pub enum ColumnView<'a> {
    Utf8(&'a StringArray),
    Int64(&'a Int64Array),
    UInt64(&'a UInt64Array),
    Float64(&'a Float64Array),
    Boolean(&'a BooleanArray),
    Unsupported,
}

impl<'a> ColumnView<'a> {
    #[inline]
    pub fn from_array(col: &'a (dyn Array + 'static)) -> Self {
        match col.data_type() {
            DataType::Utf8 => {
                if let Some(a) = col.as_any().downcast_ref::<StringArray>() {
                    Self::Utf8(a)
                } else {
                    Self::Unsupported
                }
            }
            DataType::Int64 => {
                if let Some(a) = col.as_any().downcast_ref::<Int64Array>() {
                    Self::Int64(a)
                } else {
                    Self::Unsupported
                }
            }
            DataType::UInt64 => {
                if let Some(a) = col.as_any().downcast_ref::<UInt64Array>() {
                    Self::UInt64(a)
                } else {
                    Self::Unsupported
                }
            }
            DataType::Float64 => {
                if let Some(a) = col.as_any().downcast_ref::<Float64Array>() {
                    Self::Float64(a)
                } else {
                    Self::Unsupported
                }
            }
            DataType::Boolean => {
                if let Some(a) = col.as_any().downcast_ref::<BooleanArray>() {
                    Self::Boolean(a)
                } else {
                    Self::Unsupported
                }
            }
            _ => Self::Unsupported,
        }
    }
}

// ============================================================================
// Masked Evaluation Template (Higher-Order Function Pattern)
// ============================================================================

#[inline(always)]
fn eval_masked(
    num_rows: usize,
    mask: Option<&BooleanBuffer>,
    mut predicate: impl FnMut(usize) -> bool,
) -> BooleanArray {
    let buf = BooleanBuffer::collect_bool(num_rows, |i| {
        if let Some(m) = mask {
            if !m.value(i) {
                return false;
            }
        }
        predicate(i)
    });
    BooleanArray::new(buf, None)
}

// ============================================================================
// Zero-Allocation Fast Stack Formatters & Op Helpers
// ============================================================================

#[inline(always)]
fn eval_cmp_op<T: PartialOrd>(v: T, target: T, op: &NumOp) -> bool {
    match op {
        NumOp::Gt => v > target,
        NumOp::Lt => v < target,
        NumOp::Gte => v >= target,
        NumOp::Lte => v <= target,
    }
}

#[inline]
fn format_i64(val: i64, buf: &mut [u8; 24]) -> &str {
    if val == 0 {
        buf[0] = b'0';
        return "0";
    }
    let mut is_neg = false;
    let mut u = if val < 0 {
        is_neg = true;
        (val as i128).unsigned_abs() as u64
    } else {
        val as u64
    };
    let mut i = buf.len();
    while u > 0 {
        i -= 1;
        buf[i] = b'0' + (u % 10) as u8;
        u /= 10;
    }
    if is_neg {
        i -= 1;
        buf[i] = b'-';
    }
    // Safety: only ASCII digits and '-' were written into buf
    unsafe { std::str::from_utf8_unchecked(&buf[i..]) }
}

#[inline]
fn format_u64(mut val: u64, buf: &mut [u8; 24]) -> &str {
    if val == 0 {
        buf[0] = b'0';
        return "0";
    }
    let mut i = buf.len();
    while val > 0 {
        i -= 1;
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    unsafe { std::str::from_utf8_unchecked(&buf[i..]) }
}

#[inline]
fn format_f64(val: f64, buf: &mut [u8; 32]) -> &str {
    let mut cursor = std::io::Cursor::new(&mut buf[..]);
    let _ = write!(cursor, "{}", val);
    let len = cursor.position() as usize;
    unsafe { std::str::from_utf8_unchecked(&buf[..len]) }
}

// ============================================================================
// QueryCompiler: Vectorized AST Query Execution on Arrow Batches
// ============================================================================

pub struct QueryCompiler;

impl QueryCompiler {
    // ------------------------------------------------------------------------
    // Public Mask & Boolean Helpers
    // ------------------------------------------------------------------------

    #[inline]
    pub fn all_false(num_rows: usize) -> BooleanArray {
        BooleanArray::new(BooleanBuffer::new_unset(num_rows), None)
    }

    #[inline]
    pub fn all_true(num_rows: usize) -> BooleanArray {
        BooleanArray::new(BooleanBuffer::new_set(num_rows), None)
    }

    #[inline]
    pub fn sanitize_boolean(arr: &BooleanArray) -> BooleanArray {
        Self::apply_mask(arr.clone(), None)
    }

    #[inline]
    pub fn apply_mask(arr: BooleanArray, mask: Option<&BooleanBuffer>) -> BooleanArray {
        let clean_buf = match arr.nulls() {
            Some(nulls) => arr.values() & nulls.inner(),
            None => arr.values().clone(),
        };
        match mask {
            Some(m) => BooleanArray::new(&clean_buf & m, None),
            None => BooleanArray::new(clean_buf, None),
        }
    }

    pub fn resolve_column<'a>(batch: &'a RecordBatch, field_name: &str) -> Option<&'a ArrayRef> {
        // 1. Direct match by exact name
        if let Some(col) = batch.column_by_name(field_name) {
            return Some(col);
        }

        // 2. Check if field_name is an alias of a StandardField
        if let Some(std_field) = StandardField::from_alias(field_name) {
            match std_field {
                StandardField::Timestamp => {
                    return batch
                        .column_by_name("__timestamp")
                        .or_else(|| batch.column_by_name("__timestamp_secs"));
                }
                StandardField::Id => {
                    return batch.column_by_name("__id");
                }
                StandardField::Message => {
                    return batch.column_by_name("__message");
                }
                StandardField::Level => {
                    if let Some(col) = batch.column_by_name("level") {
                        return Some(col);
                    }
                    for f in batch.schema().fields() {
                        if StandardField::from_alias(f.name()) == Some(StandardField::Level) {
                            return batch.column_by_name(f.name());
                        }
                    }
                }
            }
        }

        None
    }

    #[inline]
    pub fn resolve_column_view<'a>(
        batch: &'a RecordBatch,
        field_name: &str,
    ) -> Option<ColumnView<'a>> {
        Self::resolve_column(batch, field_name).map(|col| ColumnView::from_array(col.as_ref()))
    }

    #[inline]
    pub fn resolve_numeric_column<'a>(
        batch: &'a RecordBatch,
        field_name: &str,
    ) -> Option<ColumnView<'a>> {
        if StandardField::from_alias(field_name) == Some(StandardField::Timestamp) {
            return batch
                .column_by_name("__timestamp_secs")
                .map(|c| ColumnView::from_array(c.as_ref()));
        }
        if StandardField::from_alias(field_name) == Some(StandardField::Id) {
            return batch
                .column_by_name("__id")
                .map(|c| ColumnView::from_array(c.as_ref()));
        }
        Self::resolve_column_view(batch, field_name)
    }

    // ------------------------------------------------------------------------
    // Main Evaluation Entry Points
    // ------------------------------------------------------------------------

    #[inline]
    pub fn eval_batch(expr: &Expr, batch: &RecordBatch, now: f64) -> BooleanArray {
        Self::eval_batch_with_mask(expr, batch, now, None)
    }

    pub fn eval_batch_with_mask(
        expr: &Expr,
        batch: &RecordBatch,
        now: f64,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let num_rows = batch.num_rows();
        if num_rows == 0 {
            return Self::all_false(0);
        }

        // Early pruning: If active mask has 0 set bits, 0 rows can match
        if let Some(m) = mask {
            if m.count_set_bits() == 0 {
                return Self::all_false(num_rows);
            }
        }

        match expr {
            Expr::And(a, b) => Self::eval_and(a, b, batch, now, mask),
            Expr::Or(a, b) => Self::eval_or(a, b, batch, now, mask),
            Expr::Not(inner) => Self::eval_not(inner, batch, now, mask),
            Expr::FieldCmp { field, op, value } => {
                Self::eval_field_cmp(field, op, *value, batch, now, mask)
            }
            Expr::FieldRange { field, lo, hi } => {
                Self::eval_field_range(field, *lo, *hi, batch, now, mask)
            }
            Expr::FieldExact { field, value } => Self::eval_field_exact(field, value, batch, mask),
            Expr::FieldContainsAny { field, values } => {
                Self::eval_field_contains_any(field, values, batch, mask)
            }
            Expr::FieldRegex { field, re, .. } => Self::eval_field_regex(field, re, batch, mask),
            Expr::Text(alts) => Self::eval_text(alts, batch, mask),
        }
    }

    // ------------------------------------------------------------------------
    // Boolean AST Operators: Short-Circuit Mask Propagation
    // ------------------------------------------------------------------------

    fn eval_and(
        a: &Expr,
        b: &Expr,
        batch: &RecordBatch,
        now: f64,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        // Short-circuit: Evaluate a first with current mask
        let mask_a = Self::eval_batch_with_mask(a, batch, now, mask);
        if mask_a.values().count_set_bits() == 0 {
            return mask_a;
        }
        // Pass mask_a as selection mask into b (only active matching rows evaluated in b)
        Self::eval_batch_with_mask(b, batch, now, Some(mask_a.values()))
    }

    fn eval_or(
        a: &Expr,
        b: &Expr,
        batch: &RecordBatch,
        now: f64,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let mask_a = Self::eval_batch_with_mask(a, batch, now, mask);
        let a_true_count = mask_a.values().count_set_bits();
        let active_rows = mask.map(|m| m.count_set_bits()).unwrap_or(batch.num_rows());
        if a_true_count >= active_rows {
            return mask_a;
        }

        // Remaining rows to evaluate in b: active in mask but not yet true in a
        let not_a = !mask_a.values();
        let rem_buf = match mask {
            Some(m) => &not_a & m,
            None => not_a,
        };
        if rem_buf.count_set_bits() == 0 {
            return mask_a;
        }

        let mask_b = Self::eval_batch_with_mask(b, batch, now, Some(&rem_buf));
        BooleanArray::new(mask_a.values() | mask_b.values(), None)
    }

    fn eval_not(
        inner: &Expr,
        batch: &RecordBatch,
        now: f64,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let inner_res = Self::eval_batch_with_mask(inner, batch, now, mask);
        let not_buf = !inner_res.values();
        match mask {
            Some(m) => BooleanArray::new(&not_buf & m, None),
            None => BooleanArray::new(not_buf, None),
        }
    }

    // ------------------------------------------------------------------------
    // Field Numeric Comparison & Range Operators
    // ------------------------------------------------------------------------

    fn eval_field_cmp(
        field: &str,
        op: &NumOp,
        value: f64,
        batch: &RecordBatch,
        now: f64,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let num_rows = batch.num_rows();
        let view = match Self::resolve_numeric_column(batch, field) {
            Some(v) => v,
            None => return Self::all_false(num_rows),
        };

        match view {
            ColumnView::Float64(float_col) => {
                let vals = float_col.values();
                eval_masked(num_rows, mask, |i| {
                    float_col.is_valid(i) && eval_cmp_op(vals[i], value, op)
                })
            }
            ColumnView::Int64(int_col) => {
                let vals = int_col.values();
                let target = value as i64;
                eval_masked(num_rows, mask, |i| {
                    int_col.is_valid(i) && eval_cmp_op(vals[i], target, op)
                })
            }
            ColumnView::UInt64(u_col) => {
                let vals = u_col.values();
                let target = value as u64;
                eval_masked(num_rows, mask, |i| {
                    u_col.is_valid(i) && eval_cmp_op(vals[i], target, op)
                })
            }
            ColumnView::Utf8(str_col) => eval_masked(num_rows, mask, |i| {
                if str_col.is_valid(i) {
                    if let Some(v) = parse_numeric_value(str_col.value(i), now) {
                        return eval_cmp_op(v, value, op);
                    }
                }
                false
            }),
            _ => Self::all_false(num_rows),
        }
    }

    fn eval_field_range(
        field: &str,
        lo: f64,
        hi: f64,
        batch: &RecordBatch,
        now: f64,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let num_rows = batch.num_rows();
        let min = lo.min(hi);
        let max = lo.max(hi);

        let view = match Self::resolve_numeric_column(batch, field) {
            Some(v) => v,
            None => return Self::all_false(num_rows),
        };

        match view {
            ColumnView::Float64(float_col) => {
                let vals = float_col.values();
                eval_masked(num_rows, mask, |i| {
                    float_col.is_valid(i) && vals[i] >= min && vals[i] <= max
                })
            }
            ColumnView::Int64(int_col) => {
                let vals = int_col.values();
                let min_i = min as i64;
                let max_i = max as i64;
                eval_masked(num_rows, mask, |i| {
                    int_col.is_valid(i) && vals[i] >= min_i && vals[i] <= max_i
                })
            }
            ColumnView::UInt64(u_col) => {
                let vals = u_col.values();
                let min_u = min as u64;
                let max_u = max as u64;
                eval_masked(num_rows, mask, |i| {
                    u_col.is_valid(i) && vals[i] >= min_u && vals[i] <= max_u
                })
            }
            ColumnView::Utf8(str_col) => eval_masked(num_rows, mask, |i| {
                if str_col.is_valid(i) {
                    if let Some(v) = parse_numeric_value(str_col.value(i), now) {
                        return v >= min && v <= max;
                    }
                }
                false
            }),
            _ => Self::all_false(num_rows),
        }
    }

    // ------------------------------------------------------------------------
    // Field Equality, Contains, and Regex Matching
    // ------------------------------------------------------------------------

    fn eval_field_exact(
        field: &str,
        value: &str,
        batch: &RecordBatch,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let num_rows = batch.num_rows();
        let view = match Self::resolve_column_view(batch, field) {
            Some(v) => v,
            None => return Self::all_false(num_rows),
        };

        match view {
            ColumnView::Utf8(str_col) => {
                let data = str_col.value_data();
                if data.is_empty() {
                    return Self::all_false(num_rows);
                }
                // Chunk pruning on raw contiguous buffer
                let can_match = if value.is_ascii() {
                    contains_ignore_case_ascii_bytes(data, value.as_bytes())
                } else {
                    contains_ignore_case(std::str::from_utf8(data).unwrap_or(""), value)
                };
                if !can_match {
                    return Self::all_false(num_rows);
                }

                let offsets = str_col.value_offsets();
                let val_len = value.len();
                let val_bytes = value.as_bytes();
                let is_ascii = value.is_ascii();

                eval_masked(num_rows, mask, |i| {
                    if str_col.is_valid(i) {
                        let start = offsets[i] as usize;
                        let end = offsets[i + 1] as usize;
                        if end - start == val_len {
                            let slice = &data[start..end];
                            if is_ascii {
                                slice.eq_ignore_ascii_case(val_bytes)
                            } else {
                                std::str::from_utf8(slice)
                                    .map(|s| s.eq_ignore_ascii_case(value))
                                    .unwrap_or(false)
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
            }
            ColumnView::Int64(int_col) => {
                if let Ok(target) = value.trim().parse::<i64>() {
                    let vals = int_col.values();
                    eval_masked(num_rows, mask, |i| int_col.is_valid(i) && vals[i] == target)
                } else {
                    Self::all_false(num_rows)
                }
            }
            ColumnView::UInt64(u_col) => {
                if let Ok(target) = value.trim().parse::<u64>() {
                    let vals = u_col.values();
                    eval_masked(num_rows, mask, |i| u_col.is_valid(i) && vals[i] == target)
                } else {
                    Self::all_false(num_rows)
                }
            }
            ColumnView::Float64(float_col) => {
                if let Ok(target) = value.trim().parse::<f64>() {
                    let vals = float_col.values();
                    eval_masked(num_rows, mask, |i| {
                        if float_col.is_valid(i) {
                            let v = vals[i];
                            v == target || (v - target).abs() < f64::EPSILON
                        } else {
                            false
                        }
                    })
                } else {
                    Self::all_false(num_rows)
                }
            }
            ColumnView::Boolean(b_col) => {
                let val_lower = value.trim().to_ascii_lowercase();
                if val_lower == "true" {
                    eval_masked(num_rows, mask, |i| b_col.is_valid(i) && b_col.value(i))
                } else if val_lower == "false" {
                    eval_masked(num_rows, mask, |i| b_col.is_valid(i) && !b_col.value(i))
                } else {
                    Self::all_false(num_rows)
                }
            }
            _ => Self::all_false(num_rows),
        }
    }

    fn eval_field_contains_any(
        field: &str,
        values: &[String],
        batch: &RecordBatch,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let num_rows = batch.num_rows();
        let view = match Self::resolve_column_view(batch, field) {
            Some(v) => v,
            None => return Self::all_false(num_rows),
        };

        match view {
            ColumnView::Utf8(str_col) => {
                let data = str_col.value_data();
                if data.is_empty() {
                    return Self::all_false(num_rows);
                }
                // Chunk pruning on string column
                let can_match = values.iter().any(|v| {
                    if v.is_ascii() {
                        contains_ignore_case_ascii_bytes(data, v.as_bytes())
                    } else {
                        contains_ignore_case(std::str::from_utf8(data).unwrap_or(""), v)
                    }
                });
                if !can_match {
                    return Self::all_false(num_rows);
                }

                let offsets = str_col.value_offsets();
                eval_masked(num_rows, mask, |i| {
                    if str_col.is_valid(i) {
                        let start = offsets[i] as usize;
                        let end = offsets[i + 1] as usize;
                        let slice = &data[start..end];
                        values.iter().any(|v| {
                            if v.is_ascii() {
                                contains_ignore_case_ascii_bytes(slice, v.as_bytes())
                            } else {
                                contains_ignore_case(std::str::from_utf8(slice).unwrap_or(""), v)
                            }
                        })
                    } else {
                        false
                    }
                })
            }
            ColumnView::Int64(int_col) => {
                let has_possible_match = values.iter().any(|v| {
                    let t = v.trim();
                    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '-')
                });
                if !has_possible_match {
                    return Self::all_false(num_rows);
                }

                let parsed_targets: Vec<i64> = values
                    .iter()
                    .filter_map(|v| v.trim().parse::<i64>().ok())
                    .collect();

                let vals = int_col.values();
                eval_masked(num_rows, mask, |i| {
                    if !int_col.is_valid(i) {
                        return false;
                    }
                    let v = vals[i];
                    if parsed_targets.contains(&v) {
                        return true;
                    }
                    let mut stack_buf = [0u8; 24];
                    let str_val = format_i64(v, &mut stack_buf);
                    values.iter().any(|needle| str_val.contains(needle.trim()))
                })
            }
            ColumnView::UInt64(u_col) => {
                let has_possible_match = values.iter().any(|v| {
                    let t = v.trim();
                    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
                });
                if !has_possible_match {
                    return Self::all_false(num_rows);
                }

                let parsed_targets: Vec<u64> = values
                    .iter()
                    .filter_map(|v| v.trim().parse::<u64>().ok())
                    .collect();

                let vals = u_col.values();
                eval_masked(num_rows, mask, |i| {
                    if !u_col.is_valid(i) {
                        return false;
                    }
                    let v = vals[i];
                    if parsed_targets.contains(&v) {
                        return true;
                    }
                    let mut stack_buf = [0u8; 24];
                    let str_val = format_u64(v, &mut stack_buf);
                    values.iter().any(|needle| str_val.contains(needle.trim()))
                })
            }
            ColumnView::Float64(float_col) => {
                let parsed_targets: Vec<f64> = values
                    .iter()
                    .filter_map(|v| v.trim().parse::<f64>().ok())
                    .collect();

                let vals = float_col.values();
                eval_masked(num_rows, mask, |i| {
                    if !float_col.is_valid(i) {
                        return false;
                    }
                    let v = vals[i];
                    if parsed_targets
                        .iter()
                        .any(|&target| v == target || (v - target).abs() < f64::EPSILON)
                    {
                        return true;
                    }
                    let mut stack_buf = [0u8; 32];
                    let str_val = format_f64(v, &mut stack_buf);
                    values.iter().any(|needle| str_val.contains(needle.trim()))
                })
            }
            ColumnView::Boolean(b_col) => {
                let match_true = values
                    .iter()
                    .any(|v| "true".contains(&v.trim().to_ascii_lowercase()));
                let match_false = values
                    .iter()
                    .any(|v| "false".contains(&v.trim().to_ascii_lowercase()));
                eval_masked(num_rows, mask, |i| {
                    if !b_col.is_valid(i) {
                        return false;
                    }
                    if b_col.value(i) {
                        match_true
                    } else {
                        match_false
                    }
                })
            }
            _ => Self::all_false(num_rows),
        }
    }

    fn eval_field_regex(
        field: &str,
        re: &Regex,
        batch: &RecordBatch,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let num_rows = batch.num_rows();
        let view = match Self::resolve_column_view(batch, field) {
            Some(v) => v,
            None => return Self::all_false(num_rows),
        };

        match view {
            ColumnView::Utf8(str_col) => eval_masked(num_rows, mask, |i| {
                str_col.is_valid(i) && re.is_match(str_col.value(i))
            }),
            ColumnView::Int64(int_col) => {
                let vals = int_col.values();
                eval_masked(num_rows, mask, |i| {
                    if int_col.is_valid(i) {
                        let mut stack_buf = [0u8; 24];
                        let str_val = format_i64(vals[i], &mut stack_buf);
                        re.is_match(str_val)
                    } else {
                        false
                    }
                })
            }
            ColumnView::UInt64(u_col) => {
                let vals = u_col.values();
                eval_masked(num_rows, mask, |i| {
                    if u_col.is_valid(i) {
                        let mut stack_buf = [0u8; 24];
                        let str_val = format_u64(vals[i], &mut stack_buf);
                        re.is_match(str_val)
                    } else {
                        false
                    }
                })
            }
            ColumnView::Float64(float_col) => {
                let vals = float_col.values();
                eval_masked(num_rows, mask, |i| {
                    if float_col.is_valid(i) {
                        let mut stack_buf = [0u8; 32];
                        let str_val = format_f64(vals[i], &mut stack_buf);
                        re.is_match(str_val)
                    } else {
                        false
                    }
                })
            }
            ColumnView::Boolean(b_col) => eval_masked(num_rows, mask, |i| {
                if b_col.is_valid(i) {
                    let str_val = if b_col.value(i) { "true" } else { "false" };
                    re.is_match(str_val)
                } else {
                    false
                }
            }),
            _ => Self::all_false(num_rows),
        }
    }

    // ------------------------------------------------------------------------
    // Free-Text Alternative Search (Type-Aware & SIMD Chunk Pruning)
    // ------------------------------------------------------------------------

    fn eval_text(
        alts: &[String],
        batch: &RecordBatch,
        mask: Option<&BooleanBuffer>,
    ) -> BooleanArray {
        let num_rows = batch.num_rows();
        if alts.is_empty() || alts.iter().any(|a| a.is_empty()) {
            return match mask {
                Some(m) => BooleanArray::new(m.clone(), None),
                None => Self::all_true(num_rows),
            };
        }

        // 1. Quét các cột số nếu từ khóa là số (Type-Aware Free-Text Search)
        let num_match_buf = Self::match_numeric_columns(batch, alts, mask);

        let msg_col = batch
            .column_by_name("__message")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());

        // 2. Fast SIMD chunk pruning on __message
        let msg_can_match = if let Some(msg) = msg_col {
            let data = msg.value_data();
            if data.is_empty() {
                false
            } else {
                alts.iter().any(|alt| {
                    if alt.is_ascii() {
                        contains_ignore_case_ascii_bytes(data, alt.as_bytes())
                    } else {
                        contains_ignore_case(std::str::from_utf8(data).unwrap_or(""), alt)
                    }
                })
            }
        } else {
            false
        };

        // 3. Scan other string columns only if candidate exists
        let other_str_cols: Vec<&StringArray> = batch
            .schema()
            .fields()
            .iter()
            .enumerate()
            .filter_map(|(idx, f)| {
                if f.name() == "__message" {
                    None
                } else if let Some(str_col) =
                    batch.column(idx).as_any().downcast_ref::<StringArray>()
                {
                    let data = str_col.value_data();
                    if data.is_empty() {
                        return None;
                    }
                    let can_match = alts.iter().any(|alt| {
                        if alt.is_ascii() {
                            contains_ignore_case_ascii_bytes(data, alt.as_bytes())
                        } else {
                            contains_ignore_case(std::str::from_utf8(data).unwrap_or(""), alt)
                        }
                    });
                    if can_match {
                        Some(str_col)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // Chunk Pruning: Nếu không cột chuỗi nào chứa từ khóa
        if !msg_can_match && other_str_cols.is_empty() {
            if let Some(num_buf) = num_match_buf {
                return BooleanArray::new(num_buf, None);
            }
            return Self::all_false(num_rows);
        }

        // 4. Row-level scanning: Single needle vs Multi needle
        let str_buf = if alts.len() == 1 {
            let needle = &alts[0];
            let needle_bytes = needle.as_bytes();
            let is_ascii = needle.is_ascii();

            eval_masked(num_rows, mask, |i| {
                // Check __message first (short-circuit row immediately!)
                if msg_can_match {
                    if let Some(msg) = msg_col {
                        if msg.is_valid(i) {
                            let matched = if is_ascii {
                                contains_ignore_case_ascii_bytes(
                                    msg.value(i).as_bytes(),
                                    needle_bytes,
                                )
                            } else {
                                contains_ignore_case(msg.value(i), needle)
                            };
                            if matched {
                                return true;
                            }
                        }
                    }
                }

                // Check other string columns only if __message did not match
                for col in &other_str_cols {
                    if col.is_valid(i) {
                        let matched = if is_ascii {
                            contains_ignore_case_ascii_bytes(col.value(i).as_bytes(), needle_bytes)
                        } else {
                            contains_ignore_case(col.value(i), needle)
                        };
                        if matched {
                            return true;
                        }
                    }
                }

                false
            })
            .values()
            .clone()
        } else {
            eval_masked(num_rows, mask, |i| {
                // Check __message first
                if msg_can_match {
                    if let Some(msg) = msg_col {
                        if msg.is_valid(i) {
                            let val = msg.value(i);
                            for alt in alts {
                                if contains_ignore_case(val, alt) {
                                    return true;
                                }
                            }
                        }
                    }
                }

                // Check other string columns only if needed
                for col in &other_str_cols {
                    if col.is_valid(i) {
                        let val = col.value(i);
                        for alt in alts {
                            if contains_ignore_case(val, alt) {
                                return true;
                            }
                        }
                    }
                }

                false
            })
            .values()
            .clone()
        };

        let final_buf = match num_match_buf {
            Some(num_buf) => &str_buf | &num_buf,
            None => str_buf,
        };
        BooleanArray::new(final_buf, None)
    }

    /// Khi query tự do (free-text) chứa số (ví dụ: "500", "404"), hàm này quét song song các cột số (Int64, UInt64, Float64)
    /// bằng SIMD equality của Arrow. Nếu từ khóa là chữ thuần ("database"), hàm return None ngay lập tức (0ns overhead).
    fn match_numeric_columns(
        batch: &RecordBatch,
        alts: &[String],
        mask: Option<&BooleanBuffer>,
    ) -> Option<BooleanBuffer> {
        let mut combined_buf: Option<BooleanBuffer> = None;

        for alt in alts {
            let trimmed = alt.trim();
            let parsed_i64 = trimmed.parse::<i64>().ok();
            let parsed_f64 = trimmed.parse::<f64>().ok();

            // Nếu không phải là số (là chữ cái như "database", "timeout"), bỏ qua ngay lập tức
            if parsed_i64.is_none() && parsed_f64.is_none() {
                continue;
            }

            for col in batch.columns() {
                match ColumnView::from_array(col.as_ref()) {
                    ColumnView::Int64(int_col) => {
                        if let Some(val) = parsed_i64 {
                            let scalar = Int64Array::new_scalar(val);
                            if let Ok(res) = eq(int_col, &scalar) {
                                let clean = match res.nulls() {
                                    Some(nulls) => res.values() & nulls.inner(),
                                    None => res.values().clone(),
                                };
                                combined_buf = Some(match combined_buf {
                                    Some(prev) => &prev | &clean,
                                    None => clean,
                                });
                            }
                        }
                    }
                    ColumnView::UInt64(u_col) => {
                        if let Some(val) = parsed_i64 {
                            if val >= 0 {
                                let scalar = UInt64Array::new_scalar(val as u64);
                                if let Ok(res) = eq(u_col, &scalar) {
                                    let clean = match res.nulls() {
                                        Some(nulls) => res.values() & nulls.inner(),
                                        None => res.values().clone(),
                                    };
                                    combined_buf = Some(match combined_buf {
                                        Some(prev) => &prev | &clean,
                                        None => clean,
                                    });
                                }
                            }
                        }
                    }
                    ColumnView::Float64(f_col) => {
                        if let Some(val) = parsed_f64 {
                            let scalar = Float64Array::new_scalar(val);
                            if let Ok(res) = eq(f_col, &scalar) {
                                let clean = match res.nulls() {
                                    Some(nulls) => res.values() & nulls.inner(),
                                    None => res.values().clone(),
                                };
                                combined_buf = Some(match combined_buf {
                                    Some(prev) => &prev | &clean,
                                    None => clean,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        combined_buf.map(|buf| match mask {
            Some(m) => &buf & m,
            None => buf,
        })
    }
}
