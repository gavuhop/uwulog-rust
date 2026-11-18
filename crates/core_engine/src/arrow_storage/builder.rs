use ahash::AHashMap;
use arrow::array::{
    new_null_array, Array, ArrayBuilder, ArrayRef, BooleanBuilder, Float64Builder, Int64Builder,
    RecordBatch, StringBuilder, UInt64Builder, UInt8Builder,
};
use arrow::datatypes::{DataType, Field, Schema};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uwu_core_schema::{value_to_cow, LogEvent};

pub(crate) const USER_COL_PREFIX: &str = "f:";
pub(crate) const NULL_MARK_PREFIX: &str = "n:";
pub(crate) const JSON_VALUE_META_KEY: &str = "uwu.json";

#[inline]
pub(crate) fn strip_user_prefix(name: &str) -> Option<&str> {
    name.strip_prefix(USER_COL_PREFIX)
}

#[inline]
pub(crate) fn strip_null_mark_prefix(name: &str) -> Option<&str> {
    name.strip_prefix(NULL_MARK_PREFIX)
}

#[inline]
pub(crate) fn user_col_name(key: &str) -> String {
    format!("{USER_COL_PREFIX}{key}")
}

#[inline]
pub(crate) fn null_mark_col_name(key: &str) -> String {
    format!("{NULL_MARK_PREFIX}{key}")
}

pub enum DynamicColumnBuilder {
    Int64(Int64Builder),
    Float64(Float64Builder),
    Boolean(BooleanBuilder),
    String {
        builder: StringBuilder,
        is_json_only: bool,
    },
}

fn is_structured_json(value: &serde_json::Value) -> bool {
    value.is_array() || value.is_object()
}

fn promote_to_string_builder(
    len: usize,
    mut format_row: impl FnMut(usize, &mut StringBuilder),
    next_value: &serde_json::Value,
) -> DynamicColumnBuilder {
    let mut new_b = StringBuilder::with_capacity(len + 16, len * 16 + 64);
    for i in 0..len {
        format_row(i, &mut new_b);
    }
    let s = value_to_cow(next_value);
    new_b.append_value(s.as_ref());
    DynamicColumnBuilder::String {
        builder: new_b,
        is_json_only: false,
    }
}

impl DynamicColumnBuilder {
    pub fn new_for_value(value: &serde_json::Value, catchup_nulls: usize, capacity: usize) -> Self {
        let actual_cap = capacity.max(catchup_nulls + 1);
        if let Some(b_val) = value.as_bool() {
            let mut b = BooleanBuilder::with_capacity(actual_cap);
            b.append_nulls(catchup_nulls);
            b.append_value(b_val);
            DynamicColumnBuilder::Boolean(b)
        } else if let Some(i) = value.as_i64() {
            let mut b = Int64Builder::with_capacity(actual_cap);
            b.append_nulls(catchup_nulls);
            b.append_value(i);
            DynamicColumnBuilder::Int64(b)
        } else if let Some(f) = value.as_f64() {
            let mut b = Float64Builder::with_capacity(actual_cap);
            b.append_nulls(catchup_nulls);
            b.append_value(f);
            DynamicColumnBuilder::Float64(b)
        } else {
            let mut b = StringBuilder::with_capacity(actual_cap, actual_cap * 16);
            for _ in 0..catchup_nulls {
                b.append_null();
            }
            let s = value_to_cow(value);
            b.append_value(s.as_ref());
            DynamicColumnBuilder::String {
                builder: b,
                is_json_only: is_structured_json(value),
            }
        }
    }

    pub fn append_null(&mut self) {
        match self {
            DynamicColumnBuilder::Int64(b) => b.append_null(),
            DynamicColumnBuilder::Float64(b) => b.append_null(),
            DynamicColumnBuilder::Boolean(b) => b.append_null(),
            DynamicColumnBuilder::String { builder, .. } => builder.append_null(),
        }
    }

    pub fn append_value(&mut self, value: &serde_json::Value) {
        match self {
            DynamicColumnBuilder::Boolean(b) => {
                if let Some(v) = value.as_bool() {
                    b.append_value(v);
                } else {
                    let old_arr = b.finish();
                    *self = promote_to_string_builder(
                        old_arr.len(),
                        |i, nb| {
                            if old_arr.is_null(i) {
                                nb.append_null();
                            } else {
                                nb.append_value(if old_arr.value(i) { "true" } else { "false" });
                            }
                        },
                        value,
                    );
                }
            }
            DynamicColumnBuilder::Int64(b) => {
                if let Some(i) = value.as_i64() {
                    b.append_value(i);
                } else if let Some(f) = value.as_f64() {
                    // Promote to Float64
                    let old_arr = b.finish();
                    let len = old_arr.len();
                    let mut new_b = Float64Builder::with_capacity(len + 16);
                    for i in 0..len {
                        if old_arr.is_null(i) {
                            new_b.append_null();
                        } else {
                            new_b.append_value(old_arr.value(i) as f64);
                        }
                    }
                    new_b.append_value(f);
                    *self = DynamicColumnBuilder::Float64(new_b);
                } else {
                    // Promote to String
                    let old_arr = b.finish();
                    *self = promote_to_string_builder(
                        old_arr.len(),
                        |i, nb| {
                            if old_arr.is_null(i) {
                                nb.append_null();
                            } else {
                                nb.append_value(old_arr.value(i).to_string());
                            }
                        },
                        value,
                    );
                }
            }
            DynamicColumnBuilder::Float64(b) => {
                if let Some(f) = value.as_f64() {
                    b.append_value(f);
                } else {
                    // Promote to String
                    let old_arr = b.finish();
                    *self = promote_to_string_builder(
                        old_arr.len(),
                        |i, nb| {
                            if old_arr.is_null(i) {
                                nb.append_null();
                            } else {
                                nb.append_value(old_arr.value(i).to_string());
                            }
                        },
                        value,
                    );
                }
            }
            DynamicColumnBuilder::String {
                builder,
                is_json_only,
            } => {
                if !is_structured_json(value) {
                    *is_json_only = false;
                }
                let s = value_to_cow(value);
                builder.append_value(s.as_ref());
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            DynamicColumnBuilder::Int64(b) => b.len(),
            DynamicColumnBuilder::Float64(b) => b.len(),
            DynamicColumnBuilder::Boolean(b) => b.len(),
            DynamicColumnBuilder::String { builder, .. } => builder.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn finish(self) -> (DataType, ArrayRef, bool) {
        match self {
            DynamicColumnBuilder::Int64(mut b) => {
                (DataType::Int64, Arc::new(b.finish()) as ArrayRef, false)
            }
            DynamicColumnBuilder::Float64(mut b) => {
                (DataType::Float64, Arc::new(b.finish()) as ArrayRef, false)
            }
            DynamicColumnBuilder::Boolean(mut b) => {
                (DataType::Boolean, Arc::new(b.finish()) as ArrayRef, false)
            }
            DynamicColumnBuilder::String {
                mut builder,
                is_json_only,
            } => (
                DataType::Utf8,
                Arc::new(builder.finish()) as ArrayRef,
                is_json_only,
            ),
        }
    }
}

const MIN_ROW_CAPACITY: usize = 32;

pub struct ActiveRecordBatchBuilder {
    capacity: usize,
    row_count: usize,
    id_builder: UInt64Builder,
    color_builder: UInt8Builder,
    timestamp_builder: StringBuilder,
    timestamp_secs_builder: Float64Builder,
    message_builder: StringBuilder,
    dynamic_builders: AHashMap<String, DynamicColumnBuilder>,
    null_markers: AHashMap<String, BooleanBuilder>,
}

impl ActiveRecordBatchBuilder {
    pub fn new(capacity: usize) -> Self {
        let initial_cap = capacity.min(MIN_ROW_CAPACITY);
        Self {
            capacity,
            row_count: 0,
            id_builder: UInt64Builder::with_capacity(initial_cap),
            color_builder: UInt8Builder::with_capacity(initial_cap),
            timestamp_builder: StringBuilder::with_capacity(initial_cap, initial_cap * 24),
            timestamp_secs_builder: Float64Builder::with_capacity(initial_cap),
            message_builder: StringBuilder::with_capacity(initial_cap, initial_cap * 64),
            dynamic_builders: AHashMap::new(),
            null_markers: AHashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.row_count
    }

    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    pub fn append_log(&mut self, event: &LogEvent) {
        let row_idx = self.row_count;

        // 1. Core fixed columns
        self.id_builder.append_value(event.id);
        self.color_builder.append_value(event.color as u8);
        self.timestamp_builder.append_value(&event.timestamp);
        if let Some(ts_sec) = event.timestamp_secs {
            self.timestamp_secs_builder.append_value(ts_sec);
        } else {
            self.timestamp_secs_builder.append_null();
        }
        self.message_builder.append_value(&event.message);

        // 2. Dynamic columns & explicit null markers
        for (k, v) in &event.fields {
            if v.is_null() {
                if let Some(null_b) = self.null_markers.get_mut(k) {
                    null_b.append_value(true);
                } else {
                    let cap = (row_idx + 16).min(self.capacity);
                    let mut null_b = BooleanBuilder::with_capacity(cap);
                    null_b.append_n(row_idx, false);
                    null_b.append_value(true);
                    self.null_markers.insert(k.clone(), null_b);
                }
                continue;
            }
            if let Some(dyn_b) = self.dynamic_builders.get_mut(k) {
                dyn_b.append_value(v);
            } else {
                let dyn_cap = (row_idx + 16).min(self.capacity);
                let dyn_b = DynamicColumnBuilder::new_for_value(v, row_idx, dyn_cap);
                self.dynamic_builders.insert(k.clone(), dyn_b);
            }
        }

        // Zero-alloc null filling: any builder whose len is still row_idx was absent (or null) in this row
        for dyn_b in self.dynamic_builders.values_mut() {
            if dyn_b.len() == row_idx {
                dyn_b.append_null();
            }
        }
        for null_b in self.null_markers.values_mut() {
            if null_b.len() == row_idx {
                null_b.append_value(false);
            }
        }

        self.row_count += 1;
    }

    pub fn seal(&mut self) -> Option<RecordBatch> {
        if self.row_count == 0 {
            return None;
        }

        let total_cols = 5 + self.dynamic_builders.len() + self.null_markers.len() * 2;
        let mut fields = Vec::with_capacity(total_cols);
        let mut columns: Vec<ArrayRef> = Vec::with_capacity(total_cols);

        fields.push(Field::new("__id", DataType::UInt64, false));
        columns.push(Arc::new(self.id_builder.finish()) as ArrayRef);

        fields.push(Field::new("__color", DataType::UInt8, false));
        columns.push(Arc::new(self.color_builder.finish()) as ArrayRef);

        fields.push(Field::new("__timestamp", DataType::Utf8, false));
        columns.push(Arc::new(self.timestamp_builder.finish()) as ArrayRef);

        fields.push(Field::new("__timestamp_secs", DataType::Float64, true));
        columns.push(Arc::new(self.timestamp_secs_builder.finish()) as ArrayRef);

        fields.push(Field::new("__message", DataType::Utf8, false));
        columns.push(Arc::new(self.message_builder.finish()) as ArrayRef);

        // Sort dynamic column names deterministically
        let mut dyn_items: Vec<(String, DynamicColumnBuilder)> =
            self.dynamic_builders.drain().collect();
        dyn_items.sort_by(|a, b| a.0.cmp(&b.0));

        let mut emitted_keys = HashSet::with_capacity(dyn_items.len());
        for (k, dyn_b) in dyn_items {
            let (dtype, array, is_json_only) = dyn_b.finish();
            let mut field = Field::new(user_col_name(&k), dtype, true);
            if is_json_only {
                field = field.with_metadata(HashMap::from([(
                    JSON_VALUE_META_KEY.to_string(),
                    "1".to_string(),
                )]));
            }
            fields.push(field);
            columns.push(array);
            emitted_keys.insert(k);
        }

        // Emit explicit null marker columns (n:<key>) and ensure f:<key> exists even for all-null columns
        let mut null_items: Vec<(String, BooleanBuilder)> = self.null_markers.drain().collect();
        null_items.sort_by(|a, b| a.0.cmp(&b.0));

        for (k, mut null_b) in null_items {
            if !emitted_keys.contains(&k) {
                fields.push(Field::new(user_col_name(&k), DataType::Utf8, true));
                columns.push(new_null_array(&DataType::Utf8, self.row_count));
            }
            fields.push(Field::new(null_mark_col_name(&k), DataType::Boolean, true));
            columns.push(Arc::new(null_b.finish()) as ArrayRef);
        }

        let schema = Arc::new(Schema::new(fields));
        let batch = RecordBatch::try_new(schema, columns).ok();

        // Reset state
        self.row_count = 0;
        let initial_cap = self.capacity.min(MIN_ROW_CAPACITY);
        self.id_builder = UInt64Builder::with_capacity(initial_cap);
        self.color_builder = UInt8Builder::with_capacity(initial_cap);
        self.timestamp_builder = StringBuilder::with_capacity(initial_cap, initial_cap * 24);
        self.timestamp_secs_builder = Float64Builder::with_capacity(initial_cap);
        self.message_builder = StringBuilder::with_capacity(initial_cap, initial_cap * 64);
        self.dynamic_builders.clear();
        self.null_markers.clear();

        batch
    }
}
