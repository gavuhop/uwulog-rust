use std::collections::HashSet;

/// Trạng thái cảnh báo va chạm key JSON theo từng session (deduplicated, không spam theo từng dòng).
#[derive(Default, Clone, Debug)]
pub struct FieldWarningState {
    pub last_version: u64,
    pub collisions: Vec<(String, u64)>,
    pub dismissed_keys: HashSet<String>,
}

impl FieldWarningState {
    pub fn sync(&mut self, version: u64, collisions: Vec<(String, u64)>) {
        self.last_version = version;
        if collisions.is_empty() {
            self.dismissed_keys.clear();
        }
        self.collisions = collisions;
    }

    pub fn visible_collisions(&self) -> Vec<(&str, u64)> {
        self.collisions
            .iter()
            .filter(|(k, _)| !self.dismissed_keys.contains(k))
            .map(|(k, c)| (k.as_str(), *c))
            .collect()
    }

    pub fn dismiss_current(&mut self) {
        for (k, _) in &self.collisions {
            self.dismissed_keys.insert(k.clone());
        }
    }

    pub fn reset(&mut self) {
        self.last_version = 0;
        self.collisions.clear();
        self.dismissed_keys.clear();
    }

    pub fn has_collision_key(&self, key: &str) -> bool {
        self.collisions.iter().any(|(k, _)| k == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dismiss_and_reappear_on_new_collision_key() {
        let mut state = FieldWarningState::default();
        state.sync(1, vec![("a.b".to_string(), 3)]);
        assert_eq!(state.visible_collisions(), vec![("a.b", 3)]);

        state.dismiss_current();
        assert!(state.visible_collisions().is_empty());

        // Same key incremented -> stays dismissed
        state.sync(2, vec![("a.b".to_string(), 10)]);
        assert!(state.visible_collisions().is_empty());

        // New distinct key -> re-appears for the new key
        state.sync(3, vec![("a.b".to_string(), 10), ("x.y".to_string(), 1)]);
        assert_eq!(state.visible_collisions(), vec![("x.y", 1)]);

        state.reset();
        assert!(state.collisions.is_empty());
        assert!(state.dismissed_keys.is_empty());
    }
}
