use crate::components::ui::IconName;
use std::time::Instant;
use uwu_core_workspace::{SshConnection, SshConnectionOptions};

/// Trạng thái của hộp thoại chọn thư mục WSL/Remote
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPickerState {
    pub server: RemoteServerKind,
    pub path_query: String,
    pub current_dir: String,
    pub entries: Vec<String>,
    pub selected_index: usize,
    pub focus_input: bool,
}

impl FolderPickerState {
    pub fn new(
        server: RemoteServerKind,
        initial_dir: impl Into<String>,
        entries: Vec<String>,
    ) -> Self {
        let init = initial_dir.into();
        Self {
            server,
            path_query: init.clone(),
            current_dir: init,
            entries,
            selected_index: 0,
            focus_input: true,
        }
    }
}

/// Các giai đoạn kết nối SSH theo chuẩn của Zed Editor:
/// 1. Input: Nhập lệnh ssh (ví dụ: `ssh user@example -o 2222`)
/// 2. HostKeyVerification: Nhận diện host authenticity fingerprint -> gõ 'yes' hoặc 'no' (unmasked)
/// 3. PasswordPrompt: Nhập mật khẩu tài khoản từ xa -> masked, có nút Toggle Unmask/Mask (Eye/EyeOff)
/// 4. Connecting: Đang kết nối nền và tải danh sách thư mục
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SshPickerStage {
    #[default]
    Input,
    HostKeyVerification {
        prompt_message: String,
        user_input: String,
    },
    PasswordPrompt {
        prompt_message: String,
        password_input: String,
        is_masked: bool,
        error_message: Option<String>,
    },
    Connecting {
        status_message: String,
    },
}

/// Trạng thái của hộp thoại nhập SSH Host / Autocomplete (chuẩn Zed Editor)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshPickerState {
    pub input_query: String,
    pub suggested_hosts: Vec<String>,
    pub selected_index: usize,
    pub focus_input: bool,
    pub stage: SshPickerStage,
    pub parsed_options: Option<uwu_core_workspace::SshConnectionOptions>,
    pub error_message: Option<String>,
}

impl SshPickerState {
    pub fn new(suggested_hosts: Vec<String>) -> Self {
        Self {
            input_query: String::new(),
            suggested_hosts,
            selected_index: 0,
            focus_input: true,
            stage: SshPickerStage::Input,
            parsed_options: None,
            error_message: None,
        }
    }
}

/// Loại remote server được hỗ trợ trong uwulog (chuẩn Zed: WSL, SSH, DevContainer)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteServerKind {
    Wsl(String),
    Ssh(SshConnection),
    DevContainer(String),
}

impl RemoteServerKind {
    pub fn display_name(&self) -> &str {
        match self {
            RemoteServerKind::Wsl(name) => name,
            RemoteServerKind::Ssh(conn) => conn.display_name(),
            RemoteServerKind::DevContainer(name) => name,
        }
    }

    pub fn icon(&self) -> IconName {
        match self {
            RemoteServerKind::Wsl(_) => IconName::Linux,
            RemoteServerKind::Ssh(_) => IconName::Server,
            RemoteServerKind::DevContainer(_) => IconName::Box,
        }
    }

    pub fn from_ssh_connection(conn: &SshConnection) -> Self {
        Self::Ssh(conn.clone())
    }

    pub fn from_ssh_options(opts: &SshConnectionOptions) -> Self {
        Self::Ssh(SshConnection::from(opts))
    }

    pub fn ssh_simple(host: impl Into<String>) -> Self {
        Self::Ssh(SshConnection::new(host))
    }
}

impl From<SshConnection> for RemoteServerKind {
    fn from(conn: SshConnection) -> Self {
        Self::Ssh(conn)
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
            RemoteServerKind::Ssh(conn) => {
                let nickname_label = if conn.nickname.is_some() {
                    "Edit Nickname"
                } else {
                    "Add Nickname to Server"
                };
                let target = conn.target_string();
                vec![
                    Self {
                        action: ServerOptionAction::EditNickname,
                        icon: IconName::Pencil,
                        label: nickname_label.to_string(),
                        end_slot: None,
                        is_destructive: false,
                    },
                    Self {
                        action: ServerOptionAction::CopyAddress(target.clone()),
                        icon: IconName::Copy,
                        label: "Copy Server Address".to_string(),
                        end_slot: Some(target),
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
    SshPicker(SshPickerState),
    FolderPicker(FolderPickerState),
    ServerOptions(ServerOptionsState),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RemoteNavAction {
    #[default]
    None,
    Navigate(Box<RemoteSubView>),
    Back,
}

impl RemoteNavAction {
    pub fn navigate(subview: RemoteSubView) -> Self {
        Self::Navigate(Box::new(subview))
    }
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
