use crate::overlay::{OverlayLayer, OverlayStack, RemoteModalPlacement};
use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::views::header::devcontainer::{LocalDevContainerProbeCache, SwitcherContainersState};
use crate::views::remote_servers::{
    DevContainerConnectSession, RemoteModalState, SshInteractiveSession,
};
use uwu_core_workspace::SourceConfig;

/// Trạng thái điều hướng & phiên bất đồng bộ của modal Remote Servers.
#[derive(Default)]
pub struct RemoteServersOverlayState {
    pub modal: RemoteModalState,
    pub ssh_session: Option<SshInteractiveSession>,
    pub devcontainer_session: Option<DevContainerConnectSession>,
}

impl RemoteServersOverlayState {
    /// Đặt lại toàn bộ lịch sử điều hướng và hủy mọi phiên kết nối nền đang chạy.
    pub fn reset(&mut self) {
        self.modal.reset();
        if let Some(sess) = self.ssh_session.take() {
            sess.cancel();
        }
        self.devcontainer_session = None;
    }
}

/// Zed-style Sub-Manager chịu trách nhiệm quản lý Navigation Stack của Modals, Popovers và Floating Palettes.
pub struct OverlayManager {
    pub stack: OverlayStack,
    pub launch_modal_draft: Option<SourceConfig>,
    pub project_search_query: String,
    pub remote_placement: RemoteModalPlacement,
    pub keymap_modal_state: Option<KeymapModalState>,
    pub remote_servers: RemoteServersOverlayState,
    pub source_switcher_state: SwitcherContainersState,
    pub devcontainer_probe_cache: LocalDevContainerProbeCache,
}

/// Trạng thái nháp tương tác khi mở Modal quản lý Key Map
#[derive(Debug, Clone)]
pub struct KeymapModalState {
    pub draft: uwu_core_keymap::KeymapManager,
    pub search_query: String,
    pub selected_context: Option<uwu_core_keymap::KeyContext>,
    pub recording: Option<(uwu_core_keymap::KeyAction, uwu_core_keymap::KeyContext)>,
    pub pending_keystroke: Option<uwu_core_keymap::Keystroke>,
    pub conflict_warning: Option<String>,
    pub search_recording: bool,
    pub is_recording_keystroke: bool,
    pub pending_context: uwu_core_keymap::KeyContext,
    pub context_text: String,
    pub context_autocomplete_open: bool,
    pub context_selected_index: usize,
}

impl KeymapModalState {
    pub fn new(current_manager: &uwu_core_keymap::KeymapManager) -> Self {
        Self {
            draft: current_manager.clone(),
            search_query: String::new(),
            selected_context: None,
            recording: None,
            pending_keystroke: None,
            conflict_warning: None,
            search_recording: false,
            is_recording_keystroke: false,
            pending_context: uwu_core_keymap::KeyContext::Global,
            context_text: String::new(),
            context_autocomplete_open: false,
            context_selected_index: 0,
        }
    }

    /// Đóng và đặt lại toàn bộ trạng thái hộp thoại tùy biến phím tắt (Key Customizer).
    pub fn close_customizer(&mut self) {
        self.recording = None;
        self.pending_keystroke = None;
        self.is_recording_keystroke = false;
        self.pending_context = uwu_core_keymap::KeyContext::Global;
        self.context_text.clear();
        self.context_autocomplete_open = false;
        self.context_selected_index = 0;
    }
}

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            stack: OverlayStack::new(),
            launch_modal_draft: None,
            project_search_query: String::new(),
            remote_placement: RemoteModalPlacement::TopCenter,
            keymap_modal_state: None,
            remote_servers: RemoteServersOverlayState::default(),
            source_switcher_state: SwitcherContainersState::default(),
            devcontainer_probe_cache: LocalDevContainerProbeCache::default(),
        }
    }

    /// Kiểm tra xem một layer có đang mở trên stack hay không
    #[inline]
    pub fn is_open(&self, layer: OverlayLayer) -> bool {
        self.stack.is_open(layer)
    }

    /// Đẩy một layer mới vào đỉnh ngăn xếp nếu chưa có
    #[inline]
    pub fn push(&mut self, layer: OverlayLayer) {
        self.stack.push(layer);
    }

    /// Đóng một layer cụ thể khỏi ngăn xếp và dọn dẹp trạng thái phiên đi kèm
    pub fn close(&mut self, layer: OverlayLayer) {
        self.stack.close(layer);
        match layer {
            OverlayLayer::RemoteServersModal => self.remote_servers.reset(),
            OverlayLayer::SourceSwitcher => self.source_switcher_state.reset(),
            _ => {}
        }
    }

    /// Đóng toàn bộ hệ thống menu chính và submenu liên quan
    #[inline]
    pub fn close_main_menu(&mut self) {
        self.stack.close_main_menu();
    }

    /// Đóng popup chuyển dự án và xóa truy vấn tìm kiếm
    pub fn close_project_picker(&mut self) {
        self.close(OverlayLayer::ProjectPicker);
        self.project_search_query.clear();
    }

    /// Đóng lớp giao diện trên cùng theo thứ tự ngăn xếp (Navigation Stack LIFO)
    pub fn dismiss_top_layer(&mut self, active_session: &mut GuiSession) -> bool {
        if let Some(top) = self.stack.pop() {
            match top {
                OverlayLayer::MainMenu => {
                    self.close(OverlayLayer::ThemeSubmenu);
                }
                OverlayLayer::ThemeSubmenu => {}
                OverlayLayer::AboutModal => {}
                OverlayLayer::KeymapModal => {
                    self.keymap_modal_state = None;
                }
                OverlayLayer::LaunchModal => {
                    self.launch_modal_draft = None;
                }
                OverlayLayer::ProjectPicker => {
                    self.project_search_query.clear();
                }
                OverlayLayer::ColumnsModal => {
                    active_session.view.shared.columns.close_modal();
                }
                OverlayLayer::RemoteServersModal => {
                    self.remote_servers.reset();
                }
                OverlayLayer::SourceSwitcher => {
                    self.source_switcher_state.reset();
                }
            }
            true
        } else if active_session.view.search.autocomplete.is_open {
            active_session.view.search.autocomplete.is_open = false;
            true
        } else if active_session.view.search.history.is_open {
            active_session.view.search.history.close_popup();
            true
        } else if active_session.view.active_tab == ActiveTab::Unfiltered {
            active_session.close_unfiltered_stream();
            true
        } else if active_session.view.shared.inspector.selected_log.is_some() {
            active_session.set_selected_log(None);
            true
        } else {
            false
        }
    }
}

impl Default for OverlayManager {
    fn default() -> Self {
        Self::new()
    }
}
