use super::presets;
use super::theme::Theme;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Thứ tự hiển thị tiêu chuẩn của các Theme tích hợp sẵn
pub const STANDARD_ORDER: &[&str] = &[
    "nord-dimmed",
    "one-dark",
    "catppuccin-mocha",
    "tokyo-night",
    "light",
];

/// Quản lý danh mục toàn bộ các Theme trong ứng dụng
pub struct ThemeRegistry {
    themes: HashMap<String, Arc<Theme>>,
    builtin_ids: HashSet<String>,
    active_id: String,
}

impl ThemeRegistry {
    /// Khởi tạo Registry với các preset mặc định
    pub fn new() -> Self {
        let mut registry = Self {
            themes: HashMap::new(),
            builtin_ids: HashSet::new(),
            active_id: "nord-dimmed".to_string(),
        };

        registry.register_builtin(presets::nord_dimmed());
        registry.register_builtin(presets::one_dark());
        registry.register_builtin(presets::catppuccin_mocha());
        registry.register_builtin(presets::tokyo_night());
        registry.register_builtin(presets::light());

        registry
    }

    /// Đăng ký một preset có sẵn của hệ thống
    fn register_builtin(&mut self, theme: Theme) {
        self.builtin_ids.insert(theme.id.clone());
        self.themes.insert(theme.id.clone(), Arc::new(theme));
    }

    /// Đăng ký một Theme mới vào Registry
    pub fn register(&mut self, theme: Theme) {
        self.themes.insert(theme.id.clone(), Arc::new(theme));
    }

    /// Kiểm tra theme có phải built-in preset hay không
    pub fn is_builtin(&self, id: &str) -> bool {
        self.builtin_ids.contains(id)
    }

    /// Tìm kiếm theme theo ID
    pub fn get(&self, id: &str) -> Option<Arc<Theme>> {
        self.themes.get(id).cloned()
    }

    /// Lấy tham chiếu tới Theme đang được kích hoạt
    pub fn active(&self) -> Arc<Theme> {
        self.themes
            .get(&self.active_id)
            .cloned()
            .unwrap_or_else(|| {
                self.themes
                    .get("nord-dimmed")
                    .cloned()
                    .expect("Default theme 'nord-dimmed' must exist")
            })
    }

    /// Chuyển đổi Theme đang được kích hoạt theo ID (trả về true nếu thành công)
    pub fn set_active(&mut self, id: &str) -> bool {
        if self.themes.contains_key(id) {
            self.active_id = id.to_string();
            true
        } else {
            false
        }
    }

    /// ID của theme đang kích hoạt
    pub fn active_id(&self) -> &str {
        &self.active_id
    }

    /// Danh sách toàn bộ các theme đã đăng ký.
    /// Preset có sẵn được xếp ở đầu theo thứ tự tiêu chuẩn, kế tiếp là custom themes xếp theo tên.
    pub fn list(&self) -> Vec<Arc<Theme>> {
        let mut list: Vec<_> = self.themes.values().cloned().collect();
        list.sort_by(|a, b| {
            let pos_a = STANDARD_ORDER.iter().position(|&id| id == a.id);
            let pos_b = STANDARD_ORDER.iter().position(|&id| id == b.id);
            match (pos_a, pos_b) {
                (Some(idx_a), Some(idx_b)) => idx_a.cmp(&idx_b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            }
        });
        list
    }

    /// Danh sách các preset có sẵn của hệ thống
    pub fn list_builtin(&self) -> Vec<Arc<Theme>> {
        let mut list: Vec<_> = self
            .themes
            .values()
            .filter(|t| self.builtin_ids.contains(&t.id))
            .cloned()
            .collect();
        list.sort_by_key(|t| {
            STANDARD_ORDER
                .iter()
                .position(|&id| id == t.id)
                .unwrap_or(usize::MAX)
        });
        list
    }

    /// Danh sách các theme tùy biến nạp từ bên ngoài
    pub fn list_custom(&self) -> Vec<Arc<Theme>> {
        let mut list: Vec<_> = self
            .themes
            .values()
            .filter(|t| !self.builtin_ids.contains(&t.id))
            .cloned()
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_initialization() {
        let mut registry = ThemeRegistry::new();
        assert_eq!(registry.active_id(), "nord-dimmed");
        assert_eq!(registry.active().id, "nord-dimmed");

        assert!(registry.set_active("one-dark"));
        assert_eq!(registry.active().id, "one-dark");

        assert!(!registry.set_active("non-existent-theme"));
        assert_eq!(registry.active().id, "one-dark");

        let themes = registry.list();
        assert!(themes.len() >= 5);
        assert_eq!(themes[0].id, "nord-dimmed");

        let builtin = registry.list_builtin();
        assert_eq!(builtin.len(), 5);
        assert!(registry.is_builtin("nord-dimmed"));
        assert!(!registry.is_builtin("custom-random"));

        let custom = registry.list_custom();
        assert_eq!(custom.len(), 0);
    }
}
