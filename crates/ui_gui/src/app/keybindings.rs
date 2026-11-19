use super::{AppAction, OverlayLayer, UwuGuiApp};
use crate::keymap::KeyActionExt;
use eframe::egui;

impl UwuGuiApp {
    /// Lấy ngữ cảnh phím tắt hiện tại dựa trên Overlay Stack và trạng thái session (chuẩn Zed Dispatch Tree)
    pub fn current_key_context(&self) -> crate::keymap::KeyContext {
        match self.overlays.stack.top() {
            Some(OverlayLayer::RemoteServersModal) => crate::keymap::KeyContext::RemoteServers,
            Some(OverlayLayer::LaunchModal)
            | Some(OverlayLayer::AboutModal)
            | Some(OverlayLayer::KeymapModal)
            | Some(OverlayLayer::ColumnsModal)
            | Some(OverlayLayer::ProjectPicker)
            | Some(OverlayLayer::MainMenu)
            | Some(OverlayLayer::ThemeSubmenu)
            | Some(OverlayLayer::SourceSwitcher) => crate::keymap::KeyContext::Modal,
            None => {
                if let Some(active) = self.active_session_opt() {
                    if active.view.search.autocomplete.is_open {
                        crate::keymap::KeyContext::Autocomplete
                    } else {
                        crate::keymap::KeyContext::Global
                    }
                } else {
                    crate::keymap::KeyContext::Global
                }
            }
        }
    }

    /// Tự động suy luận KeyContext hiện tại dựa trên Overlay Stack, Autocomplete Popup và trạng thái Focus
    pub fn resolve_active_key_context(&self, ctx: &egui::Context) -> crate::keymap::KeyContext {
        let base = self.current_key_context();
        if base == crate::keymap::KeyContext::Global {
            let search_focused = self.active_session_opt().is_some_and(|active| {
                let id = crate::views::header::search_bar::search_input_id(active.session.id);
                ctx.memory(|m| m.has_focus(id))
            });
            if search_focused {
                crate::keymap::KeyContext::SearchInput
            } else {
                crate::keymap::KeyContext::Table
            }
        } else {
            base
        }
    }

    /// Bộ điều phối phím tắt tập trung tại đầu mỗi Frame (Centralized Key Event Pipeline)
    pub fn handle_keybindings(
        &mut self,
        ctx: &egui::Context,
        dispatch: &mut impl FnMut(AppAction),
    ) {
        let active_context = self.resolve_active_key_context(ctx);

        // Với RemoteServersModal: các subviews tự tiêu thụ Action (SelectNext, SelectPrev, ConfirmSelection, Back, TabComplete)
        // trong lúc render UI qua keymap.consume_input.
        if active_context == crate::keymap::KeyContext::RemoteServers {
            return;
        }

        // Nếu KeymapModal đang ở chế độ recording phím tắt: không tiêu thụ phím tắt ở đây
        if self.is_overlay_open(OverlayLayer::KeymapModal)
            && self
                .overlays
                .keymap_modal_state
                .as_ref()
                .is_some_and(|s| s.recording.is_some() || s.search_recording)
        {
            return;
        }

        if let Some(key_action) = self.keymap.consume_input_ctx(ctx, &active_context) {
            match key_action {
                crate::keymap::KeyAction::ZoomIn => {
                    let current = ctx.zoom_factor();
                    ctx.set_zoom_factor((current + 0.1).min(2.5));
                }
                crate::keymap::KeyAction::ZoomOut => {
                    let current = ctx.zoom_factor();
                    ctx.set_zoom_factor((current - 0.1).max(0.6));
                }
                crate::keymap::KeyAction::ResetZoom => {
                    ctx.set_zoom_factor(1.0);
                }
                crate::keymap::KeyAction::SelectNext => {
                    if active_context == crate::keymap::KeyContext::Autocomplete {
                        dispatch(AppAction::AutocompleteNext);
                    } else {
                        dispatch(AppAction::NavigateDown);
                    }
                }
                crate::keymap::KeyAction::SelectPrev => {
                    if active_context == crate::keymap::KeyContext::Autocomplete {
                        dispatch(AppAction::AutocompletePrev);
                    } else {
                        dispatch(AppAction::NavigateUp);
                    }
                }
                crate::keymap::KeyAction::ConfirmSelection => {
                    if active_context == crate::keymap::KeyContext::Modal {
                        match self.overlays.stack.top() {
                            Some(OverlayLayer::LaunchModal) => {
                                dispatch(AppAction::ApplyLaunchModal);
                            }
                            Some(OverlayLayer::ColumnsModal) => {
                                dispatch(AppAction::ApplyColumnsModal);
                            }
                            Some(OverlayLayer::AboutModal) => {
                                dispatch(AppAction::CloseAboutModal);
                            }
                            Some(OverlayLayer::KeymapModal) => {
                                if let Some(state) = &self.overlays.keymap_modal_state {
                                    if state.recording.is_none() {
                                        dispatch(AppAction::ApplyKeymapModal(Box::new(
                                            state.draft.clone(),
                                        )));
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else if active_context == crate::keymap::KeyContext::Autocomplete {
                        dispatch(AppAction::AutocompleteConfirm);
                    }
                }
                _ => {
                    if let Some(app_action) = key_action.to_app_action() {
                        dispatch(app_action);
                    }
                }
            }
        }
    }

    /// Lấy nhãn phím tắt định dạng thân thiện cho UI dạng mượn &str (Zero-Allocation, O(1))
    #[inline]
    pub fn keystroke_str_for(&self, action: &crate::keymap::KeyAction) -> &str {
        self.keymap
            .get_label_str(action, crate::keymap::KeyContext::Global)
            .unwrap_or_default()
    }

    /// Lấy nhãn phím tắt định dạng thân thiện cho UI (như "Alt+P", "Ctrl+PageUp")
    #[inline]
    pub fn keystroke_text_for(&self, action: &crate::keymap::KeyAction) -> String {
        self.keystroke_str_for(action).to_string()
    }
}
