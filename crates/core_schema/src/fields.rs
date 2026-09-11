use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tập hợp các trường thuộc tính phụ (Fields) của LogEvent, được tối ưu hóa mảng phẳng (Flat Vector).
/// Giảm thiểu phân mảnh heap và loại bỏ overhead bảng băm (Bucket Table) của HashMap.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LogFields(pub Vec<(String, serde_json::Value)>);

pub struct Iter<'a>(std::slice::Iter<'a, (String, serde_json::Value)>);

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a String, &'a serde_json::Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {}

pub struct IterMut<'a>(std::slice::IterMut<'a, (String, serde_json::Value)>);

impl<'a> Iterator for IterMut<'a> {
    type Item = (&'a String, &'a mut serde_json::Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (&*k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a> ExactSizeIterator for IterMut<'a> {}

pub struct Keys<'a>(std::slice::Iter<'a, (String, serde_json::Value)>);

impl<'a> Iterator for Keys<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a> ExactSizeIterator for Keys<'a> {}

pub struct Values<'a>(std::slice::Iter<'a, (String, serde_json::Value)>);

impl<'a> Iterator for Values<'a> {
    type Item = &'a serde_json::Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a> ExactSizeIterator for Values<'a> {}

impl LogFields {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut serde_json::Value> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.iter().any(|(k, _)| k == key)
    }

    pub fn insert(
        &mut self,
        key: impl Into<String>,
        val: serde_json::Value,
    ) -> Option<serde_json::Value> {
        let key_str = key.into();
        if let Some(existing) = self.0.iter_mut().find(|(k, _)| k == &key_str) {
            Some(std::mem::replace(&mut existing.1, val))
        } else {
            self.0.push((key_str, val));
            None
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        if let Some(pos) = self.0.iter().position(|(k, _)| k == key) {
            Some(self.0.swap_remove(pos).1)
        } else {
            None
        }
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&String, &serde_json::Value) -> bool,
    {
        self.0.retain(|(k, v)| f(k, v));
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter(self.0.iter())
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut(self.0.iter_mut())
    }

    pub fn keys(&self) -> Keys<'_> {
        Keys(self.0.iter())
    }

    pub fn values(&self) -> Values<'_> {
        Values(self.0.iter())
    }

    pub fn as_slice(&self) -> &[(String, serde_json::Value)] {
        &self.0
    }
}

impl<'a> IntoIterator for &'a LogFields {
    type Item = (&'a String, &'a serde_json::Value);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut LogFields {
    type Item = (&'a String, &'a mut serde_json::Value);
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl IntoIterator for LogFields {
    type Item = (String, serde_json::Value);
    type IntoIter = std::vec::IntoIter<(String, serde_json::Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<(String, serde_json::Value)> for LogFields {
    fn from_iter<T: IntoIterator<Item = (String, serde_json::Value)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl From<HashMap<String, serde_json::Value>> for LogFields {
    fn from(map: HashMap<String, serde_json::Value>) -> Self {
        Self(map.into_iter().collect())
    }
}

impl From<Vec<(String, serde_json::Value)>> for LogFields {
    fn from(vec: Vec<(String, serde_json::Value)>) -> Self {
        Self(vec)
    }
}

impl From<LogFields> for HashMap<String, serde_json::Value> {
    fn from(fields: LogFields) -> Self {
        fields.0.into_iter().collect()
    }
}

impl Serialize for LogFields {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for LogFields {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FieldsVisitor;
        impl<'de> serde::de::Visitor<'de> for FieldsVisitor {
            type Value = LogFields;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON map of fields")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut vec = Vec::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((key, value)) = access.next_entry()? {
                    vec.push((key, value));
                }
                Ok(LogFields(vec))
            }
        }

        deserializer.deserialize_map(FieldsVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_fields_operations() {
        let mut fields = LogFields::with_capacity(4);
        assert!(fields.is_empty());
        assert_eq!(fields.len(), 0);

        // Insert
        assert_eq!(fields.insert("tag", serde_json::json!("auth")), None);
        assert_eq!(fields.insert("latency", serde_json::json!(15)), None);
        assert_eq!(fields.len(), 2);
        assert!(!fields.is_empty());

        // Update existing
        assert_eq!(
            fields.insert("latency", serde_json::json!(25)),
            Some(serde_json::json!(15))
        );
        assert_eq!(fields.len(), 2);

        // Get & contains_key
        assert!(fields.contains_key("tag"));
        assert!(!fields.contains_key("nonexistent"));
        assert_eq!(fields.get("tag"), Some(&serde_json::json!("auth")));
        assert_eq!(fields.get("latency"), Some(&serde_json::json!(25)));

        // Get mut
        if let Some(v) = fields.get_mut("latency") {
            *v = serde_json::json!(30);
        }
        assert_eq!(fields.get("latency"), Some(&serde_json::json!(30)));

        // Keys & Values iterators
        let keys: Vec<&String> = fields.keys().collect();
        assert_eq!(keys, vec!["tag", "latency"]);
        let values: Vec<&serde_json::Value> = fields.values().collect();
        assert_eq!(values.len(), 2);

        // Retain
        fields.insert("temp", serde_json::json!(true));
        fields.retain(|k, _| k != "temp");
        assert_eq!(fields.len(), 2);

        // Remove
        assert_eq!(fields.remove("tag"), Some(serde_json::json!("auth")));
        assert_eq!(fields.len(), 1);
        assert!(!fields.contains_key("tag"));

        // Clear
        fields.clear();
        assert!(fields.is_empty());
    }

    #[test]
    fn test_log_fields_serde_json() {
        let mut fields = LogFields::new();
        fields.insert("service", serde_json::json!("payment"));
        fields.insert("code", serde_json::json!(200));

        let serialized = serde_json::to_string(&fields).unwrap();
        assert!(serialized.contains("\"service\":\"payment\""));
        assert!(serialized.contains("\"code\":200"));

        let deserialized: LogFields = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.get("service"),
            Some(&serde_json::json!("payment"))
        );
        assert_eq!(deserialized.get("code"), Some(&serde_json::json!(200)));
    }

    #[test]
    fn test_log_fields_conversions() {
        let mut map = HashMap::new();
        map.insert("k1".to_string(), serde_json::json!("v1"));
        map.insert("k2".to_string(), serde_json::json!(123));

        let fields: LogFields = map.clone().into();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields.get("k1"), Some(&serde_json::json!("v1")));

        let back_to_map: HashMap<String, serde_json::Value> = fields.into();
        assert_eq!(back_to_map, map);

        let vec = vec![
            ("a".to_string(), serde_json::json!(1)),
            ("b".to_string(), serde_json::json!(2)),
        ];
        let from_vec: LogFields = vec.into();
        assert_eq!(from_vec.len(), 2);
        assert_eq!(from_vec.get("a"), Some(&serde_json::json!(1)));
    }
}
