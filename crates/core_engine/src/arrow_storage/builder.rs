use ahash::AHashMap;
use arrow::array::{
    Array, ArrayBuilder, ArrayRef, Float64Builder, Int64Builder, RecordBatch, StringBuilder,
    UInt64Builder, UInt8Builder,
};
use arrow::datatypes::{DataType, Field, Schema};
use std::sync::Arc;
use uwu_core_schema::{value_to_cow, LogEvent};

pub enum DynamicColumnBuilder {
    Int64(Int64Builder),
    Float64(Float64Builder),
    String(StringBuilder),
}

impl DynamicColumnBuilder {
    pub fn new_for_value(value: &serde_json::Value, catchup_nulls: usize, capacity: usize) -> Self {
        let actual_cap = capacity.max(catchup_nulls + 1);
        if let Some(i) = value.as_i64() {
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
            DynamicColumnBuilder::String(b)
        }
    }

    pub fn append_null(&mut self) {
        match self {
            DynamicColumnBuilder::Int64(b) => b.append_null(),
            DynamicColumnBuilder::Float64(b) => b.append_null(),
            DynamicColumnBuilder::String(b) => b.append_null(),
        }
    }

    pub fn append_value(&mut self, value: &serde_json::Value) {
        match self {
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
                    let len = old_arr.len();
                    let mut new_b = StringBuilder::with_capacity(len + 16, len * 16 + 64);
                    for i in 0..len {
                        if old_arr.is_null(i) {
                            new_b.append_null();
                        } else {
                            new_b.append_value(old_arr.value(i).to_string());
                        }
                    }
                    let s = value_to_cow(value);
                    new_b.append_value(s.as_ref());
                    *self = DynamicColumnBuilder::String(new_b);
                }
            }
            DynamicColumnBuilder::Float64(b) => {
                if let Some(f) = value.as_f64() {
                    b.append_value(f);
                } else {
                    // Promote to String
                    let old_arr = b.finish();
                    let len = old_arr.len();
                    let mut new_b = StringBuilder::with_capacity(len + 16, len * 16 + 64);
                    for i in 0..len {
                        if old_arr.is_null(i) {
                            new_b.append_null();
                        } else {
                            new_b.append_value(old_arr.value(i).to_string());
                        }
                    }
                    let s = value_to_cow(value);
                    new_b.append_value(s.as_ref());
                    *self = DynamicColumnBuilder::String(new_b);
                }
            }
            DynamicColumnBuilder::String(b) => {
                let s = value_to_cow(value);
                b.append_value(s.as_ref());
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            DynamicColumnBuilder::Int64(b) => b.len(),
            DynamicColumnBuilder::Float64(b) => b.len(),
            DynamicColumnBuilder::String(b) => b.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn finish(self) -> (DataType, ArrayRef) {
        match self {
            DynamicColumnBuilder::Int64(mut b) => {
                (DataType::Int64, Arc::new(b.finish()) as ArrayRef)
            }
            DynamicColumnBuilder::Float64(mut b) => {
                (DataType::Float64, Arc::new(b.finish()) as ArrayRef)
            }
            DynamicColumnBuilder::String(mut b) => {
                (DataType::Utf8, Arc::new(b.finish()) as ArrayRef)
            }
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

        // 2. Dynamic columns
        for (k, v) in &event.fields {
            if let Some(dyn_b) = self.dynamic_builders.get_mut(k) {
                dyn_b.append_value(v);
            } else {
                let dyn_cap = (row_idx + 16).min(self.capacity);
                let dyn_b = DynamicColumnBuilder::new_for_value(v, row_idx, dyn_cap);
                self.dynamic_builders.insert(k.clone(), dyn_b);
            }
        }

        // Zero-alloc null filling: any builder whose len is still row_idx was absent in this row
        for dyn_b in self.dynamic_builders.values_mut() {
            if dyn_b.len() == row_idx {
                dyn_b.append_null();
            }
        }

        self.row_count += 1;
    }

    pub fn seal(&mut self) -> Option<RecordBatch> {
        if self.row_count == 0 {
            return None;
        }

        let mut fields = Vec::with_capacity(5 + self.dynamic_builders.len());
        let mut columns: Vec<ArrayRef> = Vec::with_capacity(5 + self.dynamic_builders.len());

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

        for (k, dyn_b) in dyn_items {
            let (dtype, array) = dyn_b.finish();
            fields.push(Field::new(k, dtype, true));
            columns.push(array);
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

        batch
    }
}
