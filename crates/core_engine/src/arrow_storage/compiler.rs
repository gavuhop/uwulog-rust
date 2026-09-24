use arrow::array::{
    Array, ArrayRef, BooleanArray, BooleanBuilder, Float64Array, Int64Array, RecordBatch,
    StringArray, UInt64Array,
};
use arrow::compute::kernels::boolean::{and, not, or};
use arrow::compute::kernels::cmp::{gt, gt_eq, lt, lt_eq};
use arrow::datatypes::DataType;
use uwu_core_filter::parser::{Expr, NumOp};
use uwu_core_schema::StandardField;
use uwu_core_util::{contains_ignore_case, parse_numeric_value};

#[inline]
fn contains_ignore_case_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

pub struct QueryCompiler;

impl QueryCompiler {
    pub fn all_false(num_rows: usize) -> BooleanArray {
        let mut b = BooleanBuilder::with_capacity(num_rows);
        b.append_n(num_rows, false);
        b.finish()
    }

    pub fn all_true(num_rows: usize) -> BooleanArray {
        let mut b = BooleanBuilder::with_capacity(num_rows);
        b.append_n(num_rows, true);
        b.finish()
    }

    pub fn sanitize_boolean(arr: &BooleanArray) -> BooleanArray {
        if arr.null_count() == 0 {
            return arr.clone();
        }
        let len = arr.len();
        let mut builder = BooleanBuilder::with_capacity(len);
        for i in 0..len {
            builder.append_value(arr.is_valid(i) && arr.value(i));
        }
        builder.finish()
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

    pub fn eval_batch(expr: &Expr, batch: &RecordBatch, now: f64) -> BooleanArray {
        let num_rows = batch.num_rows();
        if num_rows == 0 {
            return BooleanArray::from(Vec::<bool>::new());
        }

        match expr {
            Expr::And(a, b) => {
                let mask_a = Self::eval_batch(a, batch, now);
                if mask_a.true_count() == 0 {
                    return mask_a;
                }
                let mask_b = Self::eval_batch(b, batch, now);
                if let Ok(res) = and(&mask_a, &mask_b) {
                    Self::sanitize_boolean(&res)
                } else {
                    Self::all_false(num_rows)
                }
            }

            Expr::Or(a, b) => {
                let mask_a = Self::eval_batch(a, batch, now);
                if mask_a.true_count() == num_rows {
                    return mask_a;
                }
                let mask_b = Self::eval_batch(b, batch, now);
                if let Ok(res) = or(&mask_a, &mask_b) {
                    Self::sanitize_boolean(&res)
                } else {
                    Self::all_false(num_rows)
                }
            }

            Expr::Not(inner) => {
                let mask = Self::eval_batch(inner, batch, now);
                if let Ok(res) = not(&mask) {
                    Self::sanitize_boolean(&res)
                } else {
                    Self::all_false(num_rows)
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
                                return Self::sanitize_boolean(&b_arr);
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
                                return Self::sanitize_boolean(&b_arr);
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
                                return Self::sanitize_boolean(&b_arr);
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
                                return Self::sanitize_boolean(&b_arr);
                            }
                        }
                    }
                    DataType::Utf8 => {
                        if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                            let mut b = BooleanBuilder::with_capacity(num_rows);
                            for i in 0..num_rows {
                                if str_col.is_valid(i) {
                                    if let Some(v) = parse_numeric_value(str_col.value(i), now) {
                                        let matched = match op {
                                            NumOp::Gt => v > *value,
                                            NumOp::Lt => v < *value,
                                            NumOp::Gte => v >= *value,
                                            NumOp::Lte => v <= *value,
                                        };
                                        b.append_value(matched);
                                        continue;
                                    }
                                }
                                b.append_value(false);
                            }
                            return b.finish();
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
                                if let Ok(res) = and(&c1, &c2) {
                                    return Self::sanitize_boolean(&res);
                                }
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
                                if let Ok(res) = and(&c1, &c2) {
                                    return Self::sanitize_boolean(&res);
                                }
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
                                if let Ok(res) = and(&c1, &c2) {
                                    return Self::sanitize_boolean(&res);
                                }
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
                                if let Ok(res) = and(&c1, &c2) {
                                    return Self::sanitize_boolean(&res);
                                }
                            }
                        }
                    }
                    DataType::Utf8 => {
                        if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                            let mut b = BooleanBuilder::with_capacity(num_rows);
                            for i in 0..num_rows {
                                if str_col.is_valid(i) {
                                    if let Some(v) = parse_numeric_value(str_col.value(i), now) {
                                        b.append_value(v >= min && v <= max);
                                        continue;
                                    }
                                }
                                b.append_value(false);
                            }
                            return b.finish();
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
                    let mut b = BooleanBuilder::with_capacity(num_rows);
                    for i in 0..num_rows {
                        if str_col.is_valid(i) && str_col.value(i).eq_ignore_ascii_case(value) {
                            b.append_value(true);
                        } else {
                            b.append_value(false);
                        }
                    }
                    return b.finish();
                }

                Self::all_false(num_rows)
            }

            Expr::FieldContainsAny { field, values } => {
                let col = match Self::resolve_column(batch, field) {
                    Some(c) => c,
                    None => return Self::all_false(num_rows),
                };

                if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                    let mut b = BooleanBuilder::with_capacity(num_rows);
                    for i in 0..num_rows {
                        if str_col.is_valid(i)
                            && values
                                .iter()
                                .any(|v| contains_ignore_case(str_col.value(i), v))
                        {
                            b.append_value(true);
                        } else {
                            b.append_value(false);
                        }
                    }
                    return b.finish();
                }

                Self::all_false(num_rows)
            }

            Expr::FieldRegex { field, re, .. } => {
                let col = match Self::resolve_column(batch, field) {
                    Some(c) => c,
                    None => return Self::all_false(num_rows),
                };

                if let Some(str_col) = col.as_any().downcast_ref::<StringArray>() {
                    let mut b = BooleanBuilder::with_capacity(num_rows);
                    for i in 0..num_rows {
                        if str_col.is_valid(i) && re.is_match(str_col.value(i)) {
                            b.append_value(true);
                        } else {
                            b.append_value(false);
                        }
                    }
                    return b.finish();
                }

                Self::all_false(num_rows)
            }

            Expr::Text(alts) => {
                if alts.is_empty() {
                    return Self::all_true(num_rows);
                }

                let msg_col = batch
                    .column_by_name("__message")
                    .and_then(|c| c.as_any().downcast_ref::<StringArray>());

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
                            let can_match = alts
                                .iter()
                                .any(|alt| contains_ignore_case_bytes(data, alt.as_bytes()));
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

                let msg_can_match = if let Some(msg) = msg_col {
                    let data = msg.value_data();
                    alts.iter()
                        .any(|alt| contains_ignore_case_bytes(data, alt.as_bytes()))
                } else {
                    false
                };

                // Chunk pruning: If neither __message nor any other string column contains needle, 0 rows match
                if !msg_can_match && other_str_cols.is_empty() {
                    return Self::all_false(num_rows);
                }

                // Fast-path: Only __message can match and single needle
                if other_str_cols.is_empty() {
                    if let Some(msg) = msg_col {
                        if alts.len() == 1 {
                            let needle = &alts[0];
                            if needle.is_empty() {
                                return Self::all_true(num_rows);
                            }
                            let mut b = BooleanBuilder::with_capacity(num_rows);
                            for i in 0..num_rows {
                                b.append_value(
                                    msg.is_valid(i) && contains_ignore_case(msg.value(i), needle),
                                );
                            }
                            return b.finish();
                        } else {
                            let mut b = BooleanBuilder::with_capacity(num_rows);
                            for i in 0..num_rows {
                                let matched = msg.is_valid(i)
                                    && alts.iter().any(|alt| {
                                        alt.is_empty() || contains_ignore_case(msg.value(i), alt)
                                    });
                                b.append_value(matched);
                            }
                            return b.finish();
                        }
                    }
                }

                let mut b = BooleanBuilder::with_capacity(num_rows);
                for i in 0..num_rows {
                    let mut matched = false;
                    for alt in alts {
                        if alt.is_empty() {
                            matched = true;
                            break;
                        }
                        if msg_can_match {
                            if let Some(msg) = msg_col {
                                if msg.is_valid(i) && contains_ignore_case(msg.value(i), alt) {
                                    matched = true;
                                    break;
                                }
                            }
                        }
                        for col in &other_str_cols {
                            if col.is_valid(i) && contains_ignore_case(col.value(i), alt) {
                                matched = true;
                                break;
                            }
                        }
                    }
                    b.append_value(matched);
                }
                b.finish()
            }
        }
    }
}
