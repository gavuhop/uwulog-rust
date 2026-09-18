//! KeymapManager & Hierarchical Fall-through Resolver (Tầng 2 & 3).

use crate::action::KeyAction;
use crate::context::KeyContext;
use crate::keystroke::Keystroke;

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
        self.bind("escape", KeyAction::Back, KeyContext::RemoteServers);
        self.bind("down", KeyAction::SelectNext, KeyContext::RemoteServers);
        self.bind("up", KeyAction::SelectPrev, KeyContext::RemoteServers);
        self.bind(
            "enter",
            KeyAction::ConfirmSelection,
            KeyContext::RemoteServers,
        );
        self.bind("tab", KeyAction::TabComplete, KeyContext::RemoteServers);

        // --- 5. Modal Dialogs Context ---
        self.bind("enter", KeyAction::ConfirmSelection, KeyContext::Modal);
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

    // -----------------------------------------------------------------------
    // egui methods
    // -----------------------------------------------------------------------
    #[cfg(feature = "egui")]
    pub fn resolve_binding<'a>(
        &'a self,
        input: &egui::InputState,
        active_context: KeyContext,
    ) -> Option<(&'a KeyAction, &'a Keystroke)> {
        // Bước 1: Ưu tiên context cụ thể hiện tại (nếu khác Global)
        if active_context != KeyContext::Global {
            for binding in self.bindings.iter().rev() {
                if binding.context == active_context && binding.keystroke.matches(input) {
                    if binding.action == KeyAction::Unbind {
                        return None;
                    }
                    return Some((&binding.action, &binding.keystroke));
                }
            }
        }

        // Bước 2: Fall-through về ngữ cảnh Global
        for binding in self.bindings.iter().rev() {
            if binding.context == KeyContext::Global && binding.keystroke.matches(input) {
                if binding.action == KeyAction::Unbind {
                    return None;
                }
                return Some((&binding.action, &binding.keystroke));
            }
        }

        None
    }

    #[cfg(feature = "egui")]
    pub fn resolve_action(
        &self,
        input: &egui::InputState,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        self.resolve_binding(input, active_context)
            .map(|(act, _)| act.clone())
    }

    #[cfg(feature = "egui")]
    pub fn process_input(
        &self,
        ctx: &egui::Context,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        ctx.input(|input| self.resolve_action(input, active_context))
    }

    #[cfg(feature = "egui")]
    pub fn consume_input(
        &self,
        ui: &mut egui::Ui,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        let matched = ui.input(|input| {
            self.resolve_binding(input, active_context)
                .map(|(act, ks)| (act.clone(), *ks))
        });

        if let Some((action, keystroke)) = matched {
            if let Some(egui_key) = keystroke.key.to_egui() {
                ui.input_mut(|i| i.consume_key(keystroke.to_egui_modifiers(), egui_key));
            }
            Some(action)
        } else {
            None
        }
    }

    #[cfg(feature = "egui")]
    pub fn consume_input_ctx(
        &self,
        ctx: &egui::Context,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        let matched = ctx.input(|input| {
            self.resolve_binding(input, active_context)
                .map(|(act, ks)| (act.clone(), *ks))
        });

        if let Some((action, keystroke)) = matched {
            if let Some(egui_key) = keystroke.key.to_egui() {
                ctx.input_mut(|i| i.consume_key(keystroke.to_egui_modifiers(), egui_key));
            }
            Some(action)
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // crossterm methods
    // -----------------------------------------------------------------------
    #[cfg(feature = "crossterm")]
    pub fn resolve_crossterm_binding<'a>(
        &'a self,
        event: &crossterm::event::KeyEvent,
        active_context: KeyContext,
    ) -> Option<(&'a KeyAction, &'a Keystroke)> {
        // Bước 1: Ưu tiên context cụ thể hiện tại (nếu khác Global)
        if active_context != KeyContext::Global {
            for binding in self.bindings.iter().rev() {
                if binding.context == active_context && binding.keystroke.matches_crossterm(event) {
                    if binding.action == KeyAction::Unbind {
                        return None;
                    }
                    return Some((&binding.action, &binding.keystroke));
                }
            }
        }

        // Bước 2: Fall-through về ngữ cảnh Global
        for binding in self.bindings.iter().rev() {
            if binding.context == KeyContext::Global && binding.keystroke.matches_crossterm(event) {
                if binding.action == KeyAction::Unbind {
                    return None;
                }
                return Some((&binding.action, &binding.keystroke));
            }
        }

        None
    }

    #[cfg(feature = "crossterm")]
    pub fn process_crossterm_event(
        &self,
        event: &crossterm::event::KeyEvent,
        active_context: KeyContext,
    ) -> Option<KeyAction> {
        self.resolve_crossterm_binding(event, active_context)
            .map(|(act, _)| act.clone())
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

    #[cfg(feature = "egui")]
    #[test]
    fn test_context_fall_through_egui() {
        let manager = KeymapManager::new();

        // 1. Enter/Tab trong context Autocomplete -> ConfirmSelection (Tab được nạp sau nên ưu tiên tìm thấy trước)
        let enter_autocomplete =
            manager.get_label_for_action(&KeyAction::ConfirmSelection, KeyContext::Autocomplete);
        assert_eq!(enter_autocomplete.as_deref(), Some("Tab"));

        // 2. Escape trong context Modal -> fall-through về Global -> Dismiss
        let esc_modal = manager.get_label_for_action(&KeyAction::Dismiss, KeyContext::Modal);
        assert_eq!(esc_modal.as_deref(), Some("Esc"));

        // 3. Escape trong context RemoteServers -> Back (ghi đè Dismiss của Global)
        let esc_remote = manager.get_label_for_action(&KeyAction::Back, KeyContext::RemoteServers);
        assert_eq!(esc_remote.as_deref(), Some("Esc"));
    }
}
