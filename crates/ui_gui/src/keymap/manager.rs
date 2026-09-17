//! KeymapManager & Hierarchical Fall-through Resolver (Tầng 3).

use super::{action::KeyAction, context::KeyContext, keystroke::Keystroke};
use eframe::egui;

/// Một liên kết phím tắt (Key Binding) gồm: Tổ hợp phím, Hành động và Ngữ cảnh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub keystroke: Keystroke,
    pub action: KeyAction,
    pub context: KeyContext,
}

/// Bộ quản lý phím tắt tập trung (Keymap Manager) cho ứng dụng.
pub struct KeymapManager {
    bindings: Vec<KeyBinding>,
}

impl Default for KeymapManager {
    fn default() -> Self {
        Self::new()
    }
}

impl KeymapManager {
    /// Khởi tạo `KeymapManager` với các phím tắt mặc định chuẩn của `uwulog`.
    pub fn new() -> Self {
        let mut this = Self {
            bindings: Vec::with_capacity(32),
        };
        this.register_defaults();
        this
    }

    /// Đăng ký toàn bộ phím tắt mặc định chuẩn của ứng dụng.
    pub fn register_defaults(&mut self) {
        // --- 1. Global Context ---
        self.bind("escape", KeyAction::Dismiss, KeyContext::Global);
        self.bind(
            "ctrl-alt-o",
            KeyAction::ToggleProjectPicker,
            KeyContext::Global,
        );
        self.bind("alt-p", KeyAction::ToggleProjectPicker, KeyContext::Global);
        self.bind(
            "ctrl-pageup",
            KeyAction::PreviousSession,
            KeyContext::Global,
        );
        self.bind("ctrl-pagedown", KeyAction::NextSession, KeyContext::Global);
        self.bind("ctrl-plus", KeyAction::ZoomIn, KeyContext::Global);
        self.bind("ctrl-=", KeyAction::ZoomIn, KeyContext::Global);
        self.bind("ctrl--", KeyAction::ZoomOut, KeyContext::Global);
        self.bind("ctrl-0", KeyAction::ResetZoom, KeyContext::Global);
        self.bind("ctrl-l", KeyAction::ToggleLatch, KeyContext::Global);

        // --- 2. Autocomplete Popup Context ---
        self.bind("down", KeyAction::SelectNext, KeyContext::Autocomplete);
        self.bind("up", KeyAction::SelectPrev, KeyContext::Autocomplete);
        self.bind(
            "enter",
            KeyAction::ConfirmSelection,
            KeyContext::Autocomplete,
        );
        self.bind("tab", KeyAction::ConfirmSelection, KeyContext::Autocomplete);

        // --- 3. Search Bar Input Context ---
        self.bind("enter", KeyAction::CommitSearch, KeyContext::SearchInput);

        // --- 4. Remote Servers Modal Context ---
        // RemoteServersModal tự quản lý Back Stack nội bộ cho phím Escape (FolderPicker/WSL/Options -> ServerList -> Đóng modal)
        self.unbind("escape", KeyContext::RemoteServers);
    }

    /// Thêm một liên kết phím tắt mới. Nếu đã tồn tại, binding thêm sau sẽ có quyền ưu tiên cao hơn.
    pub fn bind(&mut self, keystroke_str: &str, action: KeyAction, context: KeyContext) {
        if let Ok(keystroke) = Keystroke::parse(keystroke_str) {
            self.bindings.push(KeyBinding {
                keystroke,
                action,
                context,
            });
        }
    }

    /// Vô hiệu hóa (Unbind) một tổ hợp phím trong một ngữ cảnh cụ thể.
    pub fn unbind(&mut self, keystroke_str: &str, context: KeyContext) {
        self.bind(keystroke_str, KeyAction::Unbind, context);
    }

    /// Xóa toàn bộ danh sách phím tắt hiện tại.
    pub fn clear(&mut self) {
        self.bindings.clear();
    }

    /// Trả về tất cả các bindings hiện có.
    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// Khớp phím bấm từ `egui::Context` theo thứ tự phân cấp ngữ cảnh (Hierarchical Fall-through):
    /// 1. Tìm trong `active_context` từ dưới lên (LIFO - phím nạp sau đè phím trước).
    /// 2. Nếu không có hoặc ngữ cảnh là `Global`, kiểm tra tiếp trong `KeyContext::Global`.
    /// 3. Nếu gặp `KeyAction::Unbind`, dừng lại và không kích hoạt hành động nào.
    pub fn process_input(
        &self,
        ctx: &egui::Context,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        ctx.input(|input| self.resolve_action(input, active_context))
    }

    /// Resolve action từ `egui::InputState`
    pub fn resolve_action(
        &self,
        input: &egui::InputState,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        // Bước 1: Ưu tiên context cụ thể hiện tại (nếu khác Global)
        if active_context != KeyContext::Global {
            for binding in self.bindings.iter().rev() {
                if binding.context == active_context && binding.keystroke.matches(input) {
                    if binding.action == KeyAction::Unbind {
                        return None;
                    }
                    return Some(binding.action.clone());
                }
            }
        }

        // Bước 2: Fall-through về ngữ cảnh Global
        for binding in self.bindings.iter().rev() {
            if binding.context == KeyContext::Global && binding.keystroke.matches(input) {
                if binding.action == KeyAction::Unbind {
                    return None;
                }
                return Some(binding.action.clone());
            }
        }

        None
    }

    /// Tiêu thụ sự kiện phím (Consume Key) trên `egui::Ui` để tránh phím lan truyền xuống các control bên dưới.
    /// Thường dùng cho các modal hoặc popup autocomplete cần chặn phím mũi tên / Enter / Tab / Escape.
    pub fn consume_input(
        &self,
        ui: &mut egui::Ui,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        // Kiểm tra xem có action nào khớp không
        let action = ui.input(|input| self.resolve_action(input, active_context));

        if let Some(ref act) = action {
            // Tìm keystroke tương ứng để consume
            if let Some(keystroke) = self.find_keystroke_for_action(act, active_context) {
                let modifiers = egui::Modifiers {
                    alt: keystroke.alt,
                    ctrl: keystroke.ctrl,
                    shift: keystroke.shift,
                    mac_cmd: keystroke.mac_cmd,
                    command: keystroke.ctrl || keystroke.mac_cmd,
                };
                ui.input_mut(|i| i.consume_key(modifiers, keystroke.key));
            }
        }

        action
    }

    /// Tìm keystroke đầu tiên khớp với action trong context (hoặc Global fallback), loại trừ các phím đã bị unbind sau đó.
    fn find_keystroke_for_action(
        &self,
        action: &KeyAction,
        active_context: KeyContext,
    ) -> Option<Keystroke> {
        let is_unbound =
            |target_ks: &Keystroke, target_ctx: KeyContext, after_idx: usize| -> bool {
                self.bindings[after_idx + 1..].iter().any(|b| {
                    b.context == target_ctx
                        && &b.keystroke == target_ks
                        && b.action == KeyAction::Unbind
                })
            };

        if active_context != KeyContext::Global {
            for (idx, b) in self.bindings.iter().enumerate().rev() {
                if &b.action == action
                    && b.context == active_context
                    && !is_unbound(&b.keystroke, active_context, idx)
                {
                    return Some(b.keystroke);
                }
            }
        }

        for (idx, b) in self.bindings.iter().enumerate().rev() {
            if &b.action == action
                && b.context == KeyContext::Global
                && !is_unbound(&b.keystroke, KeyContext::Global, idx)
            {
                return Some(b.keystroke);
            }
        }

        None
    }

    /// Lấy chuỗi phím tắt hiển thị lên tooltip UI hoặc nút bấm (ví dụ "Alt+P", "Ctrl+PageUp").
    pub fn get_label_for_action(&self, action: &KeyAction, context: KeyContext) -> Option<String> {
        self.find_keystroke_for_action(action, context)
            .map(|k| k.format_label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keymap_bindings() {
        let manager = KeymapManager::new();

        let alt_p =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert!(alt_p.is_some());

        let label = manager.get_label_for_action(&KeyAction::PreviousSession, KeyContext::Global);
        assert_eq!(label.as_deref(), Some("Ctrl+PageUp"));
    }

    #[test]
    fn test_override_and_unbind() {
        let mut manager = KeymapManager::new();

        // Thêm binding mới ghi đè
        manager.bind("ctrl-p", KeyAction::ToggleProjectPicker, KeyContext::Global);
        let label =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(label.as_deref(), Some("Ctrl+P"));

        // Unbind phím
        manager.unbind("ctrl-p", KeyContext::Global);
        let label_after =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(label_after.as_deref(), Some("Alt+P"));
    }
}
