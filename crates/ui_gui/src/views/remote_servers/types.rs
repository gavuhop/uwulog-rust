use crate::components::ui::IconName;
use std::time::Instant;

/// Trạng thái của hộp thoại chọn thư mục WSL/Remote
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPickerState {
    pub distro: String,
    pub path_query: String,
    pub current_dir: String,
    pub entries: Vec<String>,
    pub selected_index: usize,
    pub focus_input: bool,
    pub error: Option<String>,
}

/// Loại remote server được hỗ trợ trong uwulog (chuẩn Zed: WSL, SSH, DevContainer)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteServerKind {
    Wsl(String),
    Ssh {
        host: String,
        nickname: Option<String>,
    },
    DevContainer(String),
}

impl RemoteServerKind {
    pub fn display_name(&self) -> &str {
        match self {
            RemoteServerKind::Wsl(name) => name,
            RemoteServerKind::Ssh { host, nickname } => nickname.as_deref().unwrap_or(host),
            RemoteServerKind::DevContainer(name) => name,
        }
    }

    pub fn icon(&self) -> IconName {
        match self {
            RemoteServerKind::Wsl(_) => IconName::Linux,
            RemoteServerKind::Ssh { .. } => IconName::Server,
            RemoteServerKind::DevContainer(_) => IconName::Box,
        }
    }
}

/// Một mục hành động trong danh sách Server Options (chuẩn Zed: mở rộng dễ dàng cho SSH, Dev Container...)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerOptionAction {
    /// Xóa server khỏi danh sách (bật native Windows dialog)
    RemoveServer,
    /// Đổi hoặc đặt nickname cho SSH
    EditNickname,
    /// Copy địa chỉ server vào clipboard (cho SSH)
    CopyAddress(String),
    /// Quay lại màn hình trước
    GoBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerOptionItem {
    pub action: ServerOptionAction,
    pub icon: IconName,
    pub label: String,
    pub end_slot: Option<String>,
    pub is_destructive: bool,
}

impl ServerOptionItem {
    /// Sinh danh sách options theo loại server (chuẩn Zed: mở rộng cho SSH, Dev Container...)
    pub fn list_for_server(server: &RemoteServerKind) -> Vec<Self> {
        match server {
            RemoteServerKind::Wsl(_) => vec![
                Self {
                    action: ServerOptionAction::RemoveServer,
                    icon: IconName::Trash,
                    label: "Remove Distro".to_string(),
                    end_slot: None,
                    is_destructive: true,
                },
                Self {
                    action: ServerOptionAction::GoBack,
                    icon: IconName::ArrowLeft,
                    label: "Go Back".to_string(),
                    end_slot: None,
                    is_destructive: false,
                },
            ],
            RemoteServerKind::Ssh { host, nickname } => {
                let nickname_label = if nickname.is_some() {
                    "Edit Nickname"
                } else {
                    "Add Nickname to Server"
                };
                vec![
                    Self {
                        action: ServerOptionAction::EditNickname,
                        icon: IconName::Pencil,
                        label: nickname_label.to_string(),
                        end_slot: None,
                        is_destructive: false,
                    },
                    Self {
                        action: ServerOptionAction::CopyAddress(host.clone()),
                        icon: IconName::Copy,
                        label: "Copy Server Address".to_string(),
                        end_slot: Some(host.clone()),
                        is_destructive: false,
                    },
                    Self {
                        action: ServerOptionAction::RemoveServer,
                        icon: IconName::Trash,
                        label: "Remove Server".to_string(),
                        end_slot: None,
                        is_destructive: true,
                    },
                    Self {
                        action: ServerOptionAction::GoBack,
                        icon: IconName::ArrowLeft,
                        label: "Go Back".to_string(),
                        end_slot: None,
                        is_destructive: false,
                    },
                ]
            }
            RemoteServerKind::DevContainer(_) => vec![
                Self {
                    action: ServerOptionAction::RemoveServer,
                    icon: IconName::Trash,
                    label: "Remove Dev Container".to_string(),
                    end_slot: None,
                    is_destructive: true,
                },
                Self {
                    action: ServerOptionAction::GoBack,
                    icon: IconName::ArrowLeft,
                    label: "Go Back".to_string(),
                    end_slot: None,
                    is_destructive: false,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerOptionsState {
    pub server: RemoteServerKind,
    pub selected_index: usize,
    pub copied_flash_time: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RemoteSubView {
    #[default]
    List,
    WslPicker,
    FolderPicker(FolderPickerState),
    ServerOptions(ServerOptionsState),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RemoteNavAction {
    #[default]
    None,
    Navigate(RemoteSubView),
    Back,
}

#[derive(Clone, Default)]
pub struct RemoteModalState {
    pub search_query: String,
    pub selected_index: usize,
    pub subview: RemoteSubView,
    pub history: Vec<RemoteSubView>,
}

impl RemoteModalState {
    /// Chuyển tới màn hình mới và lưu màn hình hiện tại vào lịch sử (Back Stack)
    pub fn navigate(&mut self, next: RemoteSubView) {
        let current = std::mem::replace(&mut self.subview, next);
        self.history.push(current);
    }

    /// Quay lại màn hình trước đó trong lịch sử. Trả về true nếu back thành công, false nếu lịch sử đã rỗng.
    pub fn back(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.subview = prev;
            true
        } else {
            false
        }
    }

    /// Đặt lại toàn bộ trạng thái modal về ban đầu
    pub fn reset(&mut self) {
        self.search_query.clear();
        self.selected_index = 0;
        self.subview = RemoteSubView::List;
        self.history.clear();
    }
}
