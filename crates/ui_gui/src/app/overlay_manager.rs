use crate::overlay::{OverlayLayer, OverlayStack, RemoteModalPlacement};
use crate::session::GuiSession;
use crate::state::ActiveTab;
use uwu_core_workspace::SourceConfig;

/// Zed-style Sub-Manager chịu trách nhiệm quản lý Navigation Stack của Modals, Popovers và Floating Palettes.
pub struct OverlayManager {
    pub stack: OverlayStack,
    pub launch_modal_draft: Option<SourceConfig>,
    pub project_search_query: String,
    pub remote_placement: RemoteModalPlacement,
}

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            stack: OverlayStack::new(),
            launch_modal_draft: None,
            project_search_query: String::new(),
            remote_placement: RemoteModalPlacement::TopCenter,
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

    /// Đóng một layer cụ thể khỏi ngăn xếp
    #[inline]
    pub fn close(&mut self, layer: OverlayLayer) {
        self.stack.close(layer);
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
                OverlayLayer::LaunchModal => {
                    self.launch_modal_draft = None;
                }
                OverlayLayer::ProjectPicker => {
                    self.project_search_query.clear();
                }
                OverlayLayer::ColumnsModal => {
                    active_session.view.columns.close_modal();
                }
                OverlayLayer::RemoteServersModal => {}
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
        } else if active_session.view.inspector.selected_log.is_some() {
            active_session.view.inspector.selected_log = None;
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
