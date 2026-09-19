use super::types::RemoteServerKind;
use crate::components::ui::IconName;
use crate::overlay::RemoteModalPlacement;
use crate::theme::ActiveTheme;
use eframe::egui::{self, Align2, Color32, CornerRadius, Id, Order, Stroke};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use uwu_core_workspace::{
    clean_path, extract_project_name, RemoteConnectionOptions, SourceType, SshConnectionOptions,
    Workspace, WorkspaceLocation,
};
use uwu_driver_transport::{SshTransport, WslTransport};

/// Nối đường dẫn Unix (ví dụ: "/home/user" + "project" -> "/home/user/project")
pub fn join_unix_path(base: &str, child: &str) -> String {
    let child = child.trim_start_matches('/');
    if base.ends_with('/') {
        format!("{}{}", base, child)
    } else {
        format!("{}/{}", base, child)
    }
}

/// Nối đường dẫn thư mục Unix với dấu gạch chéo ở cuối (ví dụ: "/home/user" + "project" -> "/home/user/project/")
pub fn join_unix_dir(base: &str, child: &str) -> String {
    let joined = join_unix_path(base, child);
    if joined.ends_with('/') {
        joined
    } else {
        format!("{}/", joined)
    }
}

/// Helper làm sạch path và xác định project name (ưu tiên tên thư mục, fallback là tên server)
fn resolve_remote_dir_and_name(fallback_name: &str, target_dir: &str) -> (String, String) {
    let cleaned = clean_path(target_dir);
    let clean_dir = if cleaned.is_empty() {
        "/".to_string()
    } else {
        cleaned
    };
    let project_name = if clean_dir == "/" || clean_dir == "~" {
        fallback_name.to_string()
    } else {
        extract_project_name(&clean_dir)
    };
    (clean_dir, project_name)
}

/// Khởi tạo workspace remote cho bất kỳ loại server nào (WSL, SSH, DevContainer)
pub fn create_server_workspace(server: &RemoteServerKind, target_dir: &str) -> Workspace {
    let (clean_dir, project_name) = resolve_remote_dir_and_name(server.display_name(), target_dir);

    let is_log_file = clean_dir.ends_with(".log")
        || clean_dir.ends_with(".txt")
        || clean_dir.ends_with(".json")
        || clean_dir.ends_with(".jsonl")
        || clean_dir.ends_with(".out")
        || clean_dir.ends_with("/syslog")
        || clean_dir.ends_with("/messages")
        || clean_dir.ends_with("/dmesg");

    let source_type = if is_log_file {
        SourceType::File
    } else {
        SourceType::Process
    };

    let (work_dir, file_path) = if is_log_file {
        let parent = std::path::Path::new(&clean_dir)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("/");
        let parent_dir = if parent.is_empty() { "/" } else { parent };
        (parent_dir.to_string(), clean_dir.clone())
    } else {
        (clean_dir.clone(), String::new())
    };

    let location = match server {
        RemoteServerKind::Wsl(distro) => RemoteConnectionOptions::parse(distro, &work_dir),
        RemoteServerKind::Ssh {
            host,
            nickname,
            username,
            port,
            args,
        } => {
            let mut opts = SshConnectionOptions::new(host, &work_dir);
            if let Some(nick) = nickname {
                opts = opts.with_nickname(nick);
            }
            if let Some(u) = username {
                opts = opts.with_username(u);
            }
            if let Some(p) = port {
                opts = opts.with_port(*p);
            }
            if let Some(a) = args {
                opts = opts.with_args(a.clone());
            }
            opts.into()
        }
        RemoteServerKind::DevContainer(name) => RemoteConnectionOptions::parse(name, &work_dir),
    };

    let mut ws = Workspace::new(
        project_name,
        WorkspaceLocation::remote(location),
        source_type,
    );
    ws.file_path = file_path;
    ws
}

type DirCacheMap = HashMap<(String, String), Vec<String>>;

/// Bộ nhớ đệm danh sách thư mục từ xa để điều hướng tức thì (< 1ms)
static DIR_CACHE: LazyLock<Mutex<DirCacheMap>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Lấy danh sách thư mục từ xa cho bất kỳ loại server nào (có cache)
pub fn get_cached_or_read_directories(server: &RemoteServerKind, dir: &str) -> Vec<String> {
    let key = (server.display_name().to_string(), dir.to_string());
    if let Ok(cache) = DIR_CACHE.lock() {
        if let Some(entries) = cache.get(&key) {
            return entries.clone();
        }
    }

    let entries = match server {
        RemoteServerKind::Wsl(distro) => {
            WslTransport::list_remote_directories(distro, dir).unwrap_or_default()
        }
        RemoteServerKind::Ssh {
            host,
            username,
            port,
            args,
            ..
        } => SshTransport::list_remote_directories(
            host,
            dir,
            username.as_deref(),
            *port,
            args.as_deref(),
        )
        .unwrap_or_default(),
        RemoteServerKind::DevContainer(_) => Vec::new(),
    };

    if let Ok(mut cache) = DIR_CACHE.lock() {
        cache.insert(key, entries.clone());
    }
    entries
}

/// Xóa sạch bộ nhớ đệm thư mục từ xa (dùng khi refresh hoặc trong unit tests)
pub fn clear_dir_cache() {
    if let Ok(mut cache) = DIR_CACHE.lock() {
        cache.clear();
    }
}

/// Phân rã query nhập đường dẫn thành (parent_dir, suffix) theo chuẩn Unix (giống Zed)
pub fn get_dir_and_suffix(query: &str) -> (String, String) {
    let query = query.trim();
    if query.is_empty() {
        return ("/".to_string(), String::new());
    }
    if let Some(index) = query.rfind('/') {
        let dir = &query[..=index];
        let suffix = &query[index + 1..];
        (dir.to_string(), suffix.to_string())
    } else {
        ("/".to_string(), query.to_string())
    }
}

/// Helper vẽ 1 hàng action dạng danh sách chuẩn Zed (chuẩn hóa builder pattern)
pub struct ListItemRow<'a> {
    pub icon: IconName,
    pub label: &'a str,
    pub end_slot: Option<&'a str>,
    pub is_muted: bool,
    pub is_destructive: bool,
    pub is_success: bool,
    pub is_selected: bool,
    pub tooltip: Option<&'a str>,
}

impl<'a> ListItemRow<'a> {
    pub fn new(icon: IconName, label: &'a str) -> Self {
        Self {
            icon,
            label,
            end_slot: None,
            is_muted: false,
            is_destructive: false,
            is_success: false,
            is_selected: false,
            tooltip: None,
        }
    }

    pub fn end_slot(mut self, end_slot: Option<&'a str>) -> Self {
        self.end_slot = end_slot;
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.is_muted = muted;
        self
    }

    pub fn destructive(mut self, destructive: bool) -> Self {
        self.is_destructive = destructive;
        self
    }

    pub fn success(mut self, success: bool) -> Self {
        self.is_success = success;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    pub fn tooltip(mut self, tooltip: Option<&'a str>) -> Self {
        self.tooltip = tooltip;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = ui.app_theme();
        let row_size = egui::vec2(ui.available_width(), 28.0);
        let (rect, mut resp) = ui.allocate_exact_size(row_size, egui::Sense::click());
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

        let is_active = resp.hovered() || self.is_selected;
        let bg_color = if is_active {
            theme.log.row_hover
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(4), bg_color);
        }

        let content_rect = rect.shrink2(egui::vec2(6.0, 0.0));
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.style_mut().interaction.selectable_labels = false;

                let item_color = if self.is_destructive {
                    theme.status.error
                } else if self.is_success {
                    theme.status.success
                } else if self.is_muted {
                    theme.text.muted
                } else {
                    theme.text.primary
                };
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                self.icon.paint(ui.painter(), icon_rect, item_color);

                ui.add_space(6.0);

                ui.add(
                    egui::Label::new(egui::RichText::new(self.label).size(12.0).color(item_color))
                        .selectable(false)
                        .truncate(),
                );

                if let Some(end_text) = self.end_slot {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(end_text)
                                    .size(11.0)
                                    .color(theme.text.muted),
                            )
                            .selectable(false),
                        );
                    });
                }
            },
        );

        if let Some(tip) = self.tooltip {
            resp = resp.on_hover_text(tip);
        }

        resp
    }
}

/// Render thông báo trạng thái rỗng
pub fn render_empty_state(ui: &mut egui::Ui, text: &str) {
    let theme = ui.app_theme();
    ui.vertical_centered(|ui| {
        ui.add_space(16.0);
        ui.label(egui::RichText::new(text).size(12.0).color(theme.text.muted));
        ui.add_space(12.0);
    });
}

/// Render tiêu đề phân mục chuẩn Zed
pub fn render_section_title(ui: &mut egui::Ui, title: &str) {
    let theme = ui.app_theme();
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title)
            .size(11.0)
            .color(theme.text.muted),
    );
    ui.add_space(2.0);
}

/// Tính toán chiều cao ScrollArea tự co giãn theo số lượng item (giống cách làm của suggestion trong autocomplete_popup)
pub fn calculate_adaptive_scroll_height(
    item_count: usize,
    item_height: f32,
    max_height: f32,
) -> f32 {
    if item_count == 0 {
        48.0
    } else {
        ((item_count as f32 * item_height) + 6.0).clamp(40.0, max_height)
    }
}

/// Điều hướng chỉ số item được chọn qua phím mũi tên Lên/Xuống với cơ chế cuộn vòng (wrap-around)
pub fn step_selected_index(current: &mut usize, total: usize, key_down: bool, key_up: bool) {
    if total == 0 {
        *current = 0;
        return;
    }
    if key_down {
        *current = if *current + 1 >= total {
            0
        } else {
            *current + 1
        };
    }
    if key_up {
        *current = if *current == 0 {
            total - 1
        } else {
            *current - 1
        };
    }
    if *current >= total {
        *current = 0;
    }
}

/// Neo con trỏ văn bản vào cuối ô nhập liệu và yêu cầu focus (chuẩn Zed)
pub fn anchor_cursor_to_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    let mut text_state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let char_count = text.chars().count();
    text_state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(char_count),
        )));
    text_state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Container hiển thị danh sách Remote Servers/WSL theo vị trí TopCenter hoặc TopLeft
pub struct RemotePickerContainer<'a> {
    id: Id,
    width: f32,
    max_height: Option<f32>,
    placement: RemoteModalPlacement,
    close_on_escape: bool,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> RemotePickerContainer<'a> {
    pub fn new(id_str: &'a str) -> Self {
        Self {
            id: Id::new(id_str),
            width: 520.0,
            max_height: Some(480.0),
            placement: RemoteModalPlacement::TopCenter,
            close_on_escape: true,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn placement(mut self, placement: RemoteModalPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn close_on_escape(mut self, close_on_escape: bool) -> Self {
        self.close_on_escape = close_on_escape;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = Some(max_height);
        self
    }

    pub fn show(self, ctx: &egui::Context, add_contents: impl FnOnce(&mut egui::Ui)) -> bool {
        let mut close_requested = false;

        // Phím Escape để đóng (nếu bật cờ close_on_escape)
        if self.close_on_escape && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close_requested = true;
        }

        let modal_rect_id = self.id.with("_modal_rect");
        let last_modal_rect: Option<egui::Rect> = ctx.data(|d| d.get_temp(modal_rect_id));
        let server_btn_rect: Option<egui::Rect> =
            ctx.data(|d| d.get_temp(Id::new("server_button_rect")));

        // Click outside để đóng
        if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
            if let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let in_server_btn = server_btn_rect.is_some_and(|r| r.contains(click_pos));
                if !in_server_btn {
                    if let Some(modal_rect) = last_modal_rect {
                        if !modal_rect.contains(click_pos) {
                            close_requested = true;
                        }
                    }
                }
            }
        }

        let screen_rect = ctx.viewport_rect();
        let (anchor, offset, max_allowed_h) = match self.placement {
            RemoteModalPlacement::TopLeft => {
                let x = server_btn_rect.map_or(38.0, |r| r.min.x);
                (
                    Align2::LEFT_TOP,
                    egui::vec2(x, 38.0),
                    (screen_rect.height() - 54.0).max(180.0),
                )
            }
            RemoteModalPlacement::TopCenter => (
                Align2::CENTER_TOP,
                egui::vec2(0.0, 60.0),
                (screen_rect.height() - 80.0).max(180.0),
            ),
        };

        let theme = ctx.app_theme();
        let area_resp = egui::Area::new(self.id)
            .order(Order::Foreground)
            .anchor(anchor, offset)
            .show(ctx, |ui| {
                let frame = egui::Frame::default()
                    .fill(theme.surfaces.mantle)
                    .stroke(Stroke::new(1.0, theme.borders.border))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(16))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(50),
                    });

                let total_margin = frame.total_margin();
                frame.show(ui, |ui| {
                    let inner_width = (self.width - total_margin.sum().x).max(0.0);
                    ui.set_width(inner_width);
                    if let Some(h) = self.max_height {
                        ui.set_max_height(h.min(max_allowed_h));
                    }
                    add_contents(ui);
                });
            });

        ctx.data_mut(|d| d.insert_temp(modal_rect_id, area_resp.response.rect));

        close_requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_unix_path_and_dir() {
        assert_eq!(
            join_unix_path("/home/user", "project"),
            "/home/user/project"
        );
        assert_eq!(
            join_unix_path("/home/user/", "project"),
            "/home/user/project"
        );
        assert_eq!(
            join_unix_path("/home/user/", "/project"),
            "/home/user/project"
        );
        assert_eq!(
            join_unix_dir("/home/user", "project"),
            "/home/user/project/"
        );
        assert_eq!(
            join_unix_dir("/home/user/", "project/"),
            "/home/user/project/"
        );
    }

    #[test]
    fn test_create_server_workspace_and_view_helpers() {
        let server_wsl = RemoteServerKind::Wsl("Ubuntu".to_string());
        let ws1 = create_server_workspace(&server_wsl, "/home/user/backend/");
        assert_eq!(ws1.name, "backend");
        assert_eq!(ws1.location.working_dir(), "/home/user/backend");
        assert_eq!(ws1.server_name(), Some("Ubuntu"));

        let ws_root = create_server_workspace(&server_wsl, "/");
        assert_eq!(ws_root.name, "Ubuntu");
        assert_eq!(ws_root.location.working_dir(), "/");
        assert_eq!(ws_root.server_name(), Some("Ubuntu"));

        // Test create_server_workspace for SSH
        let server_ssh = RemoteServerKind::Ssh {
            host: "192.168.1.100".to_string(),
            nickname: Some("my-vps".to_string()),
            username: Some("root".to_string()),
            port: Some(2222),
            args: None,
        };
        let ws_ssh = create_server_workspace(&server_ssh, "/var/log/nginx");
        assert_eq!(ws_ssh.name, "nginx");
        assert_eq!(ws_ssh.location.working_dir(), "/var/log/nginx");
        assert_eq!(ws_ssh.server_name(), Some("my-vps"));
        if let WorkspaceLocation::Remote(RemoteConnectionOptions::Ssh(ref opts)) = ws_ssh.location {
            assert_eq!(opts.username.as_deref(), Some("root"));
            assert_eq!(opts.port, Some(2222));
        } else {
            panic!("Expected Ssh location");
        }

        // Test create_server_workspace at root
        let ws_ssh_root = create_server_workspace(&server_ssh, "/");
        assert_eq!(ws_ssh_root.name, "my-vps");

        // Test create_server_workspace with a log file
        let ws_ssh_file = create_server_workspace(&server_ssh, "/var/log/nginx/access.log");
        assert_eq!(ws_ssh_file.name, "access.log");
        assert_eq!(ws_ssh_file.source_type, SourceType::File);
        assert_eq!(ws_ssh_file.file_path, "/var/log/nginx/access.log");
        assert_eq!(ws_ssh_file.location.working_dir(), "/var/log/nginx");
    }
}
