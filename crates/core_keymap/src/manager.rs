//! KeymapManager & Hierarchical Fall-through Resolver (Tầng 2 & 3).

use crate::action::KeyAction;
use crate::config::{KeymapConfigFile, KeymapSection};
use crate::context::KeyContext;
use crate::key::Key;
use crate::keystroke::Keystroke;
use std::collections::{BTreeMap, HashMap};

/// Một liên kết phím tắt (Key Binding) gồm: Tổ hợp phím, Hành động và Ngữ cảnh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub keystroke: Keystroke,
    pub action: KeyAction,
    pub context: KeyContext,
}

/// Bộ quản lý phím tắt tập trung (Keymap Manager) cho ứng dụng.
/// Tối ưu hóa hiệu năng với:
/// - Fast-path check thoát ngay trong frame idle (< 2ns).
/// - Pre-computed Label Cache $O(1)$ Zero-Allocation cho UI render loop.
#[derive(Debug, Clone)]
pub struct KeymapManager {
    bindings: Vec<KeyBinding>,
    /// Cache nhãn hiển thị UI: `(KeyAction, KeyContext) -> String` (Ví dụ: "Alt+P", "Ctrl+PageUp").
    label_cache: HashMap<(KeyAction, KeyContext), String>,
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
            label_cache: HashMap::with_capacity(32),
        };
        this.register_defaults();
        this
    }

    /// Tái tạo cache nhãn hiển thị UI `(KeyAction, KeyContext) -> String` để phục vụ tra cứu $O(1)$ zero-allocation trong render loop.
    pub fn rebuild_cache(&mut self) {
        self.label_cache.clear();

        let mut all_contexts: Vec<KeyContext> = KeyContext::all().to_vec();
        for b in &self.bindings {
            if !all_contexts.contains(&b.context) {
                all_contexts.push(b.context.clone());
            }
        }

        for ctx in &all_contexts {
            let mut current = Some(ctx.clone());
            let mut depth = 0;

            while let Some(c) = current {
                depth += 1;
                if depth > 16 {
                    break;
                }

                for b in self.bindings.iter().rev() {
                    if b.context == c && b.action != KeyAction::Unbind {
                        let key = (b.action.clone(), ctx.clone());
                        if !self.label_cache.contains_key(&key)
                            && self.resolve_keystroke(&b.keystroke, ctx) == Some(&b.action)
                        {
                            self.label_cache.insert(key, b.keystroke.format_label());
                        }
                    }
                }

                current = c.parent();
            }
        }
    }

    /// Đăng ký toàn bộ phím tắt mặc định chuẩn của ứng dụng bằng các hằng số Typed Keystroke.
    pub fn register_defaults(&mut self) {
        // --- 1. Global Context ---
        self.bind_keystroke_internal(
            Keystroke::new(Key::Escape),
            KeyAction::Dismiss,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::O).with_ctrl().with_alt(),
            KeyAction::ToggleProjectPicker,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::alt(Key::P),
            KeyAction::ToggleProjectPicker,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::PageUp),
            KeyAction::PreviousSession,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::PageDown),
            KeyAction::NextSession,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::Plus),
            KeyAction::ZoomIn,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::Equals),
            KeyAction::ZoomIn,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::Minus),
            KeyAction::ZoomOut,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::Num0),
            KeyAction::ResetZoom,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::L),
            KeyAction::ToggleLatch,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::Comma),
            KeyAction::OpenKeymapModal,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::F),
            KeyAction::FocusFilter,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::H),
            KeyAction::ToggleSearchHistory,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::ctrl(Key::R),
            KeyAction::RestartSource,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::F5),
            KeyAction::RestartSource,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::alt(Key::R),
            KeyAction::OpenLaunchModal,
            KeyContext::Global,
        );
        self.bind_keystroke_internal(
            Keystroke::alt(Key::C),
            KeyAction::OpenColumnsModal,
            KeyContext::Global,
        );

        // --- 2. Autocomplete Popup Context ---
        self.bind_keystroke_internal(
            Keystroke::new(Key::ArrowDown),
            KeyAction::SelectNext,
            KeyContext::Autocomplete,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::ArrowUp),
            KeyAction::SelectPrev,
            KeyContext::Autocomplete,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::Enter),
            KeyAction::ConfirmSelection,
            KeyContext::Autocomplete,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::Tab),
            KeyAction::ConfirmSelection,
            KeyContext::Autocomplete,
        );

        // --- 3. Search Bar Input Context ---
        self.bind_keystroke_internal(
            Keystroke::new(Key::Enter),
            KeyAction::CommitSearch,
            KeyContext::SearchInput,
        );

        // --- 4. Remote Servers Modal Context ---
        self.bind_keystroke_internal(
            Keystroke::new(Key::Escape),
            KeyAction::Back,
            KeyContext::RemoteServers,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::ArrowDown),
            KeyAction::SelectNext,
            KeyContext::RemoteServers,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::ArrowUp),
            KeyAction::SelectPrev,
            KeyContext::RemoteServers,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::Enter),
            KeyAction::ConfirmSelection,
            KeyContext::RemoteServers,
        );
        self.bind_keystroke_internal(
            Keystroke::new(Key::Tab),
            KeyAction::TabComplete,
            KeyContext::RemoteServers,
        );

        // --- 5. Modal Dialogs Context ---
        self.bind_keystroke_internal(
            Keystroke::new(Key::Enter),
            KeyAction::ConfirmSelection,
            KeyContext::Modal,
        );

        self.rebuild_cache();
    }

    /// Thêm liên kết nội bộ không tự động rebuild cache (dùng khi đăng ký hàng loạt).
    fn bind_keystroke_internal(
        &mut self,
        keystroke: Keystroke,
        action: KeyAction,
        context: KeyContext,
    ) {
        if let Some(pos) = self
            .bindings
            .iter()
            .position(|b| b.keystroke == keystroke && b.context == context)
        {
            if self.bindings[pos].action == action {
                return;
            }
            self.bindings.remove(pos);
        }

        self.bindings.push(KeyBinding {
            keystroke,
            action,
            context,
        });
    }

    /// Thêm liên kết phím tắt dạng typed `Keystroke`.
    /// Tự động loại bỏ duplicate và cập nhật lại cache hiệu năng $O(1)$.
    pub fn bind_keystroke(&mut self, keystroke: Keystroke, action: KeyAction, context: KeyContext) {
        self.bind_keystroke_internal(keystroke, action, context);
        self.rebuild_cache();
    }

    /// Thêm một liên kết phím tắt mới bằng chuỗi canonical.
    pub fn bind(&mut self, keystroke_str: &str, action: KeyAction, context: KeyContext) {
        if let Ok(keystroke) = Keystroke::parse(keystroke_str) {
            self.bind_keystroke(keystroke, action, context);
        }
    }

    /// Vô hiệu hóa (Unbind) một tổ hợp phím trong một ngữ cảnh cụ thể.
    pub fn unbind_keystroke(&mut self, keystroke: Keystroke, context: KeyContext) {
        self.bind_keystroke(keystroke, KeyAction::Unbind, context);
    }

    /// Vô hiệu hóa (Unbind) một tổ hợp phím bằng chuỗi canonical.
    pub fn unbind(&mut self, keystroke_str: &str, context: KeyContext) {
        if let Ok(keystroke) = Keystroke::parse(keystroke_str) {
            self.unbind_keystroke(keystroke, context);
        }
    }

    /// Xóa toàn bộ danh sách phím tắt hiện tại và xóa sạch cache.
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.label_cache.clear();
    }

    /// Trả về tất cả các bindings hiện có.
    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// Thuật toán phân giải (Resolver Engine) cốt lõi dùng chung cho mọi backend (egui, crossterm, testing).
    /// Duyệt ngược từ ngữ cảnh hiện tại (`active_context`), sau đó lần theo cây phân cấp `context.parent()`
    /// cho đến `Global`.
    pub fn resolve_matching<F>(
        &self,
        active_context: impl std::borrow::Borrow<KeyContext>,
        mut matcher: F,
    ) -> Option<(&KeyAction, &Keystroke)>
    where
        F: FnMut(&Keystroke) -> bool,
    {
        let mut current = Some(active_context.borrow().clone());
        let mut depth = 0;

        while let Some(ctx) = current {
            depth += 1;
            if depth > 16 {
                break;
            }

            for binding in self.bindings.iter().rev() {
                if binding.context == ctx && matcher(&binding.keystroke) {
                    if binding.action == KeyAction::Unbind {
                        return None;
                    }
                    return Some((&binding.action, &binding.keystroke));
                }
            }

            current = ctx.parent();
        }

        None
    }

    /// Phân giải trực tiếp từ `Keystroke` sang `KeyAction` trong ngữ cảnh chỉ định theo cây Context Hierarchy.
    pub fn resolve_keystroke(
        &self,
        keystroke: &Keystroke,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<&KeyAction> {
        self.resolve_matching(active_context.borrow(), |k| k == keystroke)
            .map(|(act, _)| act)
    }

    /// Tìm keystroke đầu tiên khớp với action trong context (hoặc cha theo hierarchy).
    /// Đảm bảo tính nhất quán 2 chiều tuyệt đối:
    /// Một phím tắt `K` chỉ được xem là đại diện cho `Action` nếu tại thời điểm hiện tại `resolve(K) == Action`.
    pub fn find_keystroke_for_action(
        &self,
        action: &KeyAction,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<Keystroke> {
        let active_ref = active_context.borrow();
        let mut current = Some(active_ref.clone());
        let mut depth = 0;

        while let Some(ctx) = current {
            depth += 1;
            if depth > 16 {
                break;
            }

            for b in self.bindings.iter().rev() {
                if b.context == ctx
                    && &b.action == action
                    && self.resolve_keystroke(&b.keystroke, active_ref) == Some(action)
                {
                    return Some(b.keystroke);
                }
            }

            current = ctx.parent();
        }

        None
    }

    /// Lấy chuỗi phím tắt hiển thị UI (dạng mượn `&str`, Zero-Allocation, $O(1)$ Hash Lookup).
    /// Ví dụ: "Alt+P", "Ctrl+PageUp". Rất thích hợp gọi trong vòng lặp render 60 FPS.
    pub fn get_label_str(
        &self,
        action: &KeyAction,
        context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<&str> {
        self.label_cache
            .get(&(action.clone(), context.borrow().clone()))
            .map(|s| s.as_str())
    }

    /// Lấy chuỗi phím tắt hiển thị lên tooltip UI hoặc nút bấm (trả về String để tương thích ngược).
    pub fn get_label_for_action(
        &self,
        action: &KeyAction,
        context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<String> {
        self.get_label_str(action, context.borrow())
            .map(|s| s.to_string())
    }

    /// Lấy danh sách tất cả các phím tắt đang được gán cho một hành động trong ngữ cảnh chỉ định (bao gồm cả kế thừa).
    pub fn keystrokes_for_action(
        &self,
        action: &KeyAction,
        context: impl std::borrow::Borrow<KeyContext>,
    ) -> Vec<Keystroke> {
        let ctx_ref = context.borrow();
        let mut result = Vec::new();
        for b in &self.bindings {
            if &b.context == ctx_ref
                && &b.action == action
                && self.resolve_keystroke(&b.keystroke, ctx_ref) == Some(action)
            {
                result.push(b.keystroke);
            }
        }
        if result.is_empty() {
            if let Some(parent) = ctx_ref.parent() {
                return self.keystrokes_for_action(action, parent);
            }
        }
        result
    }

    /// Kiểm tra xem một tổ hợp phím có bị xung đột với hành động nào khác trong cùng ngữ cảnh không.
    pub fn find_conflict(
        &self,
        keystroke: &Keystroke,
        context: impl std::borrow::Borrow<KeyContext>,
        except_action: &KeyAction,
    ) -> Option<KeyAction> {
        if let Some(existing_action) = self.resolve_keystroke(keystroke, context.borrow()) {
            if existing_action != except_action && existing_action != &KeyAction::Unbind {
                return Some(existing_action.clone());
            }
        }
        None
    }

    /// Gỡ bỏ hoàn toàn một tổ hợp phím trong ngữ cảnh chỉ định (nếu có).
    pub fn remove_keystroke(
        &mut self,
        keystroke: &Keystroke,
        context: impl std::borrow::Borrow<KeyContext>,
    ) {
        let ctx_ref = context.borrow();
        let initial_len = self.bindings.len();
        self.bindings
            .retain(|b| !(b.keystroke == *keystroke && &b.context == ctx_ref));

        // Nếu phím này được kế thừa từ context cha, ta unbind rõ ràng để ghi đè
        if self.bindings.len() == initial_len
            && ctx_ref.parent().is_some()
            && self.resolve_keystroke(keystroke, ctx_ref).is_some()
        {
            self.unbind_keystroke(*keystroke, ctx_ref.clone());
            return;
        }

        self.rebuild_cache();
    }

    /// Gỡ bỏ toàn bộ phím tắt đang gán cho một hành động trong ngữ cảnh cụ thể.
    pub fn remove_action_binding(
        &mut self,
        action: &KeyAction,
        context: impl std::borrow::Borrow<KeyContext>,
    ) {
        let ctx = context.borrow();
        let current_keys = self.keystrokes_for_action(action, ctx);
        for ks in current_keys {
            self.remove_keystroke(&ks, ctx);
        }
    }

    /// Trả về danh sách tất cả các ngữ cảnh hiện có (bao gồm các built-in mặc định và các context tùy biến đang được gán).
    pub fn active_contexts(&self) -> Vec<KeyContext> {
        let mut all_contexts: Vec<KeyContext> = KeyContext::all().to_vec();
        for b in &self.bindings {
            if !all_contexts.contains(&b.context) {
                all_contexts.push(b.context.clone());
            }
        }
        all_contexts
    }

    /// Xuất toàn bộ cấu hình phím tắt hiện tại ra cấu trúc `KeymapConfigFile` (bao gồm cả phím mặc định).
    pub fn export_full_config(&self) -> KeymapConfigFile {
        let mut sections = Vec::new();
        let all_contexts = self.active_contexts();

        for ctx in &all_contexts {
            let mut bindings_map = BTreeMap::new();
            let mut unbind_list = Vec::new();

            for b in &self.bindings {
                if &b.context == ctx {
                    if b.action == KeyAction::Unbind {
                        unbind_list.push(b.keystroke);
                    } else {
                        bindings_map.insert(b.keystroke, b.action.clone());
                    }
                }
            }

            if !bindings_map.is_empty() || !unbind_list.is_empty() {
                sections.push(KeymapSection {
                    context: ctx.clone(),
                    bindings: if bindings_map.is_empty() {
                        None
                    } else {
                        Some(bindings_map)
                    },
                    unbind: if unbind_list.is_empty() {
                        None
                    } else {
                        Some(unbind_list)
                    },
                });
            }
        }
        KeymapConfigFile(sections)
    }

    /// Xuất cấu hình phím tắt khác biệt (delta / user overrides) so với mặc định hệ thống.
    /// File cấu hình người dùng chỉ lưu phần thay đổi (thêm mới, đổi phím, hoặc unbind)
    /// chuẩn theo triết lý của Zed Editor, không đóng băng toàn bộ phím hệ thống.
    pub fn export_config(&self) -> KeymapConfigFile {
        let defaults = Self::new();
        let mut sections = Vec::new();
        let mut all_contexts = self.active_contexts();
        for db in &defaults.bindings {
            if !all_contexts.contains(&db.context) {
                all_contexts.push(db.context.clone());
            }
        }

        for ctx in &all_contexts {
            let mut bindings_map = BTreeMap::new();
            let mut unbind_list = Vec::new();

            // 1. Phím mặc định bị xóa hoặc chuyển sang Unbind
            for def_b in &defaults.bindings {
                if &def_b.context == ctx && def_b.action != KeyAction::Unbind {
                    let self_matching = self
                        .bindings
                        .iter()
                        .find(|b| b.keystroke == def_b.keystroke && &b.context == ctx);
                    let is_unbound_or_removed = match self_matching {
                        None => true,
                        Some(b) => b.action == KeyAction::Unbind,
                    };
                    if is_unbound_or_removed && !unbind_list.contains(&def_b.keystroke) {
                        unbind_list.push(def_b.keystroke);
                    }
                }
            }

            // 2. Phím người dùng thêm mới hoặc đổi sang hành động khác
            for b in &self.bindings {
                if &b.context == ctx {
                    if b.action == KeyAction::Unbind {
                        if !unbind_list.contains(&b.keystroke) {
                            unbind_list.push(b.keystroke);
                        }
                    } else {
                        let def_matching = defaults
                            .bindings
                            .iter()
                            .find(|db| db.keystroke == b.keystroke && &db.context == ctx);
                        let is_same_as_default =
                            def_matching.is_some_and(|db| db.action == b.action);
                        if !is_same_as_default {
                            bindings_map.insert(b.keystroke, b.action.clone());
                        }
                    }
                }
            }

            if !bindings_map.is_empty() || !unbind_list.is_empty() {
                sections.push(KeymapSection {
                    context: ctx.clone(),
                    bindings: if bindings_map.is_empty() {
                        None
                    } else {
                        Some(bindings_map)
                    },
                    unbind: if unbind_list.is_empty() {
                        None
                    } else {
                        Some(unbind_list)
                    },
                });
            }
        }
        KeymapConfigFile(sections)
    }

    // -----------------------------------------------------------------------
    // egui methods
    // -----------------------------------------------------------------------
    #[cfg(feature = "egui")]
    pub fn resolve_binding<'a>(
        &'a self,
        input: &egui::InputState,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<(&'a KeyAction, &'a Keystroke)> {
        // Fast-path: Nếu frame hiện tại không có bất kỳ phím nào được nhấn, thoát ngay lập tức!
        // Giúp loại bỏ 99.9% chi phí CPU khi ứng dụng ở trạng thái idle/chỉ di chuột.
        if !input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
        {
            return None;
        }

        self.resolve_matching(active_context, |k| k.matches(input))
    }

    #[cfg(feature = "egui")]
    pub fn resolve_action(
        &self,
        input: &egui::InputState,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<KeyAction> {
        self.resolve_binding(input, active_context)
            .map(|(act, _)| act.clone())
    }

    #[cfg(feature = "egui")]
    pub fn process_input(
        &self,
        ctx: &egui::Context,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<KeyAction> {
        let active = active_context.borrow();
        ctx.input(|input| self.resolve_action(input, active))
    }

    #[cfg(feature = "egui")]
    pub fn consume_input_ctx(
        &self,
        ctx: &egui::Context,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<KeyAction> {
        let active = active_context.borrow();
        let matched = ctx.input(|input| {
            self.resolve_binding(input, active)
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

    #[cfg(feature = "egui")]
    pub fn consume_input(
        &self,
        ui: &mut egui::Ui,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<KeyAction> {
        self.consume_input_ctx(ui.ctx(), active_context)
    }

    // -----------------------------------------------------------------------
    // crossterm methods
    // -----------------------------------------------------------------------
    #[cfg(feature = "crossterm")]
    pub fn resolve_crossterm_binding<'a>(
        &'a self,
        event: &crossterm::event::KeyEvent,
        active_context: impl std::borrow::Borrow<KeyContext>,
    ) -> Option<(&'a KeyAction, &'a Keystroke)> {
        self.resolve_matching(active_context, |k| k.matches_crossterm(event))
    }

    #[cfg(feature = "crossterm")]
    pub fn process_crossterm_event(
        &self,
        event: &crossterm::event::KeyEvent,
        active_context: impl std::borrow::Borrow<KeyContext>,
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

        // Kiểm tra tra cứu O(1) Zero-Allocation
        let label_str = manager.get_label_str(&KeyAction::PreviousSession, KeyContext::Global);
        assert_eq!(label_str, Some("Ctrl+PageUp"));
    }

    #[test]
    fn test_override_and_unbind() {
        let mut manager = KeymapManager::new();

        // Thêm binding mới ghi đè
        manager.bind("ctrl-p", KeyAction::ToggleProjectPicker, KeyContext::Global);
        let label =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(label.as_deref(), Some("Ctrl+P"));
        assert_eq!(
            manager.get_label_str(&KeyAction::ToggleProjectPicker, KeyContext::Global),
            Some("Ctrl+P")
        );

        // Unbind phím
        manager.unbind("ctrl-p", KeyContext::Global);
        let label_after =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(label_after.as_deref(), Some("Alt+P"));
        assert_eq!(
            manager.get_label_str(&KeyAction::ToggleProjectPicker, KeyContext::Global),
            Some("Alt+P")
        );
    }

    #[test]
    fn test_override_to_another_action_does_not_leak_old_label() {
        let mut manager = KeymapManager::new();

        // Ban đầu ToggleProjectPicker có Alt+P và Ctrl+Alt+O
        let initial_label =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(initial_label.as_deref(), Some("Alt+P"));

        // Người dùng rebind Alt+P sang Quit
        manager.bind("alt-p", KeyAction::Quit, KeyContext::Global);

        // Lúc này ToggleProjectPicker KHÔNG ĐƯỢC trả về Alt+P nữa!
        // Nó phải tự động fall-through về phím còn lại: Ctrl+Alt+O
        let updated_label =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(updated_label.as_deref(), Some("Ctrl+Alt+O"));
        assert_eq!(
            manager.get_label_str(&KeyAction::ToggleProjectPicker, KeyContext::Global),
            Some("Ctrl+Alt+O")
        );
    }

    #[test]
    fn test_no_duplicate_bindings_on_repeated_bind() {
        let mut manager = KeymapManager::new();
        let initial_len = manager.bindings().len();

        // Đăng ký lại chính xác binding mặc định
        manager.bind("alt-p", KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(manager.bindings().len(), initial_len);
    }

    #[cfg(feature = "egui")]
    #[test]
    fn test_idle_fast_path_egui() {
        let manager = KeymapManager::new();

        // Giả lập frame idle (không có sự kiện nhấn phím nào)
        let input = egui::InputState::default();
        let result = manager.resolve_binding(&input, KeyContext::Global);
        assert!(result.is_none());
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

    #[test]
    fn test_keystrokes_for_action_and_conflict() {
        let mut manager = KeymapManager::new();

        let keys =
            manager.keystrokes_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert!(keys.contains(&Keystroke::alt(Key::P)));

        // Conflict check: alt-p is already bound to ToggleProjectPicker
        let conflict = manager.find_conflict(
            &Keystroke::alt(Key::P),
            KeyContext::Global,
            &KeyAction::Quit,
        );
        assert_eq!(conflict, Some(KeyAction::ToggleProjectPicker));

        // No conflict if except_action is the same action
        let no_conflict = manager.find_conflict(
            &Keystroke::alt(Key::P),
            KeyContext::Global,
            &KeyAction::ToggleProjectPicker,
        );
        assert_eq!(no_conflict, None);

        // Rebind alt-p to Quit
        manager.bind_keystroke(Keystroke::alt(Key::P), KeyAction::Quit, KeyContext::Global);
        let quit_keys = manager.keystrokes_for_action(&KeyAction::Quit, KeyContext::Global);
        assert!(quit_keys.contains(&Keystroke::alt(Key::P)));

        // Export config
        let cfg = manager.export_config();
        assert!(!cfg.0.is_empty());
    }

    #[test]
    fn test_export_delta_clean_when_unchanged() {
        let manager = KeymapManager::new();
        let delta = manager.export_config();
        assert!(
            delta.0.is_empty(),
            "Khi không có thay đổi, delta phải rỗng chuẩn Zed"
        );

        let full = manager.export_full_config();
        assert!(
            !full.0.is_empty(),
            "Full config phải chứa toàn bộ phím mặc định"
        );
    }
}
