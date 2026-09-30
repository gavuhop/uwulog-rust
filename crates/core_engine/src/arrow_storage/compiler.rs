use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array,
};
use arrow::buffer::BooleanBuffer;
use arrow::compute::kernels::cmp::{eq, gt, gt_eq, lt, lt_eq};
use arrow::datatypes::DataType;
use uwu_core_filter::parser::{Expr, NumOp};
use uwu_core_schema::StandardField;
use uwu_core_util::{contains_ignore_case, contains_ignore_case_ascii_bytes, parse_numeric_value};

pub struct QueryCompiler;

impl QueryCompiler {
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
                        .column_by_name("__timestamp_secs")
                        .or_else(|| batch.column_by_name("__timestamp"));
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

        // Early pruning: If mask has 0 set bits, 0 rows can match
        if let Some(m) = mask {
            if m.count_set_bits() == 0 {
                return Self::all_false(num_rows);
            }
        }

        match expr {
            Expr::And(a, b) => {
                // Short-circuit: Evaluate a first with current mask
                let mask_a = Self::eval_batch_with_mask(a, batch, now, mask);
                if mask_a.values().count_set_bits() == 0 {
                    return mask_a;
                }
                // Pass mask_a as selection mask into b (only active matching rows evaluated in b)
                Self::eval_batch_with_mask(b, batch, now, Some(mask_a.values()))
            }

            Expr::Or(a, b) => {
                let mask_a = Self::eval_batch_with_mask(a, batch, now, mask);
                let a_true_count = mask_a.values().count_set_bits();
                let active_rows = mask.map(|m| m.count_set_bits()).unwrap_or(num_rows);
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

            Expr::Not(inner) => {
                let inner_res = Self::eval_batch_with_mask(inner, batch, now, None);
                let not_buf = !inner_res.values();
                match mask {
                    Some(m) => BooleanArray::new(&not_buf & m, None),
                    None => BooleanArray::new(not_buf, None),
                }
            }

            Expr::FieldCmp { field, op, value } => {
                let is_ts = StandardField::from_alias(field) == Some(StandardField::Timestamp);
                let is_id = StandardField::from_alias(field) == Some(StandardField::Id);

                if is_ts {
                    if let Some(col) = batch.column_by_name("__timestamp_secs") {
                        if let Some(float_col) = col.as_any().downcast_ref::<Float64Array>() {
                            let scalar = Float64Array::new_scalar(*value);
                            let res = match op {
                                NumOp::Gt => gt(float_col, &scalar),
                                NumOp::Lt => lt(float_col, &scalar),
                                NumOp::Gte => gt_eq(float_col, &scalar),
                                NumOp::Lte => lt_eq(float_col, &scalar),
                            };
                            if let Ok(b_arr) = res {
                                return Self::apply_mask(b_arr, mask);
                            }
                        }
                    }
                    return Self::all_false(num_rows);
                }

                if is_id {
                    if let Some(col) = batch.column_by_name("__id") {
                        if let Some(u_col) = col.as_any().downcast_ref::<UInt64Array>() {
                            let scalar = UInt64Array::new_scalar(*value as u64);
                            let res = match op {
                                NumOp::Gt => gt(u_col, &scalar),
                                NumOp::Lt => lt(u_col, &scalar),
                                NumOp::Gte => gt_eq(u_col, &scalar),
                                NumOp::Lte => lt_eq(u_col, &scalar),
                            };
                            if let Ok(b_arr) = res {
                                return Self::apply_mask(b_arr, mask);
                            }
                        }
                    }
                    return Self::all_false(num_rows);
                }

                let col = match Self::resolve_column(batch, field) {
                    Some(c) => c,
                    None => return Self::all_false(num_rows),
                };

                match col.data_type() {
                    DataType::Float64 => {
                        if let Some(float_col) = col.as_any().downcast_ref::<Float64Array>() {
                            let scalar = Float64Array::new_scalar(*value);
                            let res = match op {
                                NumOp::Gt => gt(float_col, &scalar),
                                NumOp::Lt => lt(float_col, &scalar),
                                NumOp::Gte => gt_eq(float_col, &scalar),
                                NumOp::Lte => lt_eq(float_col, &scalar),
                            };
                            if let Ok(b_arr) = res {
                                return Self::apply_mask(b_arr, mask);
                            }
                        }
                    }
                    DataType::Int64 => {
                        if let Some(int_col) = col.as_any().downcast_ref::<Int64Array>() {
                            let scalar = Int64Array::new_scalar(*value as i64);
                            let res = match op {
                                NumOp::Gt => gt(int_col, &scalar),
                                NumOp::Lt => lt(int_col, &scalar),
                                NumOp::Gte => gt_eq(int_col, &scalar),
                                NumOp::Lte => lt_eq(int_col, &scalar),
                            };
                            if let Ok(b_arr) = res {
                                return Self::apply_mask(b_arr, mask);
                            }
                        }
                    }
                    DataType::Utf8 => {
                        if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                            let buf = BooleanBuffer::collect_bool(num_rows, |i| {
                                if let Some(m) = mask {
                                    if !m.value(i) {
                                        return false;
                                    }
                                }
                                if str_col.is_valid(i) {
                                    if let Some(v) = parse_numeric_value(str_col.value(i), now) {
                                        return match op {
                                            NumOp::Gt => v > *value,
                                            NumOp::Lt => v < *value,
                                            NumOp::Gte => v >= *value,
                                            NumOp::Lte => v <= *value,
                                        };
                                    }
                                }
                                false
                            });
                            return BooleanArray::new(buf, None);
                        }
                    }
                    _ => {}
                }

                Self::all_false(num_rows)
            }

            Expr::FieldRange { field, lo, hi } => {
                let min = lo.min(*hi);
                let max = lo.max(*hi);

                let is_ts = StandardField::from_alias(field) == Some(StandardField::Timestamp);
                let is_id = StandardField::from_alias(field) == Some(StandardField::Id);

                if is_ts {
                    if let Some(col) = batch.column_by_name("__timestamp_secs") {
                        if let Some(float_col) = col.as_any().downcast_ref::<Float64Array>() {
                            let min_scalar = Float64Array::new_scalar(min);
                            let max_scalar = Float64Array::new_scalar(max);
                            if let (Ok(c1), Ok(c2)) =
                                (gt_eq(float_col, &min_scalar), lt_eq(float_col, &max_scalar))
                            {
                                let res_buf = c1.values() & c2.values();
                                return Self::apply_mask(BooleanArray::new(res_buf, None), mask);
                            }
                        }
                    }
                    return Self::all_false(num_rows);
                }

                if is_id {
                    if let Some(col) = batch.column_by_name("__id") {
                        if let Some(u_col) = col.as_any().downcast_ref::<UInt64Array>() {
                            let min_scalar = UInt64Array::new_scalar(min as u64);
                            let max_scalar = UInt64Array::new_scalar(max as u64);
                            if let (Ok(c1), Ok(c2)) =
                                (gt_eq(u_col, &min_scalar), lt_eq(u_col, &max_scalar))
                            {
                                let res_buf = c1.values() & c2.values();
                                return Self::apply_mask(BooleanArray::new(res_buf, None), mask);
                            }
                        }
                    }
                    return Self::all_false(num_rows);
                }

                let col = match Self::resolve_column(batch, field) {
                    Some(c) => c,
                    None => return Self::all_false(num_rows),
                };

                match col.data_type() {
                    DataType::Float64 => {
                        if let Some(float_col) = col.as_any().downcast_ref::<Float64Array>() {
                            let min_scalar = Float64Array::new_scalar(min);
                            let max_scalar = Float64Array::new_scalar(max);
                            if let (Ok(c1), Ok(c2)) =
                                (gt_eq(float_col, &min_scalar), lt_eq(float_col, &max_scalar))
                            {
                                let res_buf = c1.values() & c2.values();
                                return Self::apply_mask(BooleanArray::new(res_buf, None), mask);
                            }
                        }
                    }
                    DataType::Int64 => {
                        if let Some(int_col) = col.as_any().downcast_ref::<Int64Array>() {
                            let min_scalar = Int64Array::new_scalar(min as i64);
                            let max_scalar = Int64Array::new_scalar(max as i64);
                            if let (Ok(c1), Ok(c2)) =
                                (gt_eq(int_col, &min_scalar), lt_eq(int_col, &max_scalar))
                            {
                                let res_buf = c1.values() & c2.values();
                                return Self::apply_mask(BooleanArray::new(res_buf, None), mask);
                            }
                        }
                    }
                    DataType::Utf8 => {
                        if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                            let buf = BooleanBuffer::collect_bool(num_rows, |i| {
                                if let Some(m) = mask {
                                    if !m.value(i) {
                                        return false;
                                    }
                                }
                                if str_col.is_valid(i) {
                                    if let Some(v) = parse_numeric_value(str_col.value(i), now) {
                                        return v >= min && v <= max;
                                    }
                                }
                                false
                            });
                            return BooleanArray::new(buf, None);
                        }
                    }
                    _ => {}
                }

                Self::all_false(num_rows)
            }

            Expr::FieldExact { field, value } => {
                let col = match Self::resolve_column(batch, field) {
                    Some(c) => c,
                    None => return Self::all_false(num_rows),
                };

                if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                    let buf = BooleanBuffer::collect_bool(num_rows, |i| {
                        if let Some(m) = mask {
                            if !m.value(i) {
                                return false;
                            }
                        }
                        str_col.is_valid(i) && str_col.value(i).eq_ignore_ascii_case(value)
                    });
                    return BooleanArray::new(buf, None);
                }

                Self::all_false(num_rows)
            }

            Expr::FieldContainsAny { field, values } => {
                let col = match Self::resolve_column(batch, field) {
                    Some(c) => c,
                    None => return Self::all_false(num_rows),
                };

                if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                    let buf = BooleanBuffer::collect_bool(num_rows, |i| {
                        if let Some(m) = mask {
                            if !m.value(i) {
                                return false;
                            }
                        }
                        str_col.is_valid(i)
                            && values
                                .iter()
                                .any(|v| contains_ignore_case(str_col.value(i), v))
                    });
                    return BooleanArray::new(buf, None);
                }

                Self::all_false(num_rows)
            }

            Expr::FieldRegex { field, re, .. } => {
                let col = match Self::resolve_column(batch, field) {
                    Some(c) => c,
                    None => return Self::all_false(num_rows),
                };

                if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                    let buf = BooleanBuffer::collect_bool(num_rows, |i| {
                        if let Some(m) = mask {
                            if !m.value(i) {
                                return false;
                            }
                        }
                        str_col.is_valid(i) && re.is_match(str_col.value(i))
                    });
                    return BooleanArray::new(buf, None);
                }

                Self::all_false(num_rows)
            }

            Expr::Text(alts) => {
                if alts.is_empty() {
                    return match mask {
                        Some(m) => BooleanArray::new(m.clone(), None),
                        None => Self::all_true(num_rows),
                    };
                }

                if alts.iter().any(|a| a.is_empty()) {
                    return match mask {
                        Some(m) => BooleanArray::new(m.clone(), None),
                        None => Self::all_true(num_rows),
                    };
                }

                // A. Quét các cột số nếu từ khóa là số (Type-Aware Free-Text Search)
                let num_match_buf = Self::match_numeric_columns(batch, alts, mask);

                let msg_col = batch
                    .column_by_name("__message")
                    .and_then(|c| c.as_any().downcast_ref::<StringArray>());

                // 1. Fast SIMD chunk pruning on __message
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

                // 2. Scan other string columns only if candidate exists
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
                                    contains_ignore_case(
                                        std::str::from_utf8(data).unwrap_or(""),
                                        alt,
                                    )
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
                    // Nếu các cột số có kết quả match, trả về kết quả số
                    if let Some(num_buf) = num_match_buf {
                        return BooleanArray::new(num_buf, None);
                    }
                    return Self::all_false(num_rows);
                }

                // 3. Fast-path: Single needle (overwhelmingly most common case)
                let str_buf = if alts.len() == 1 {
                    let needle = &alts[0];
                    let needle_bytes = needle.as_bytes();
                    let is_ascii = needle.is_ascii();

                    BooleanBuffer::collect_bool(num_rows, |i| {
                        if let Some(m) = mask {
                            if !m.value(i) {
                                return false;
                            }
                        }

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
                                    contains_ignore_case_ascii_bytes(
                                        col.value(i).as_bytes(),
                                        needle_bytes,
                                    )
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
                } else {
                    // 4. Multi-needle fallback
                    BooleanBuffer::collect_bool(num_rows, |i| {
                        if let Some(m) = mask {
                            if !m.value(i) {
                                return false;
                            }
                        }

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
                };

                let final_buf = match num_match_buf {
                    Some(num_buf) => &str_buf | &num_buf,
                    None => str_buf,
                };
                BooleanArray::new(final_buf, None)
            }
        }
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
                match col.data_type() {
                    DataType::Int64 => {
                        if let Some(val) = parsed_i64 {
                            if let Some(int_col) = col.as_any().downcast_ref::<Int64Array>() {
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
                    }
                    DataType::UInt64 => {
                        if let Some(val) = parsed_i64 {
                            if val >= 0 {
                                if let Some(u_col) = col.as_any().downcast_ref::<UInt64Array>() {
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
                    }
                    DataType::Float64 => {
                        if let Some(val) = parsed_f64 {
                            if let Some(f_col) = col.as_any().downcast_ref::<Float64Array>() {
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
