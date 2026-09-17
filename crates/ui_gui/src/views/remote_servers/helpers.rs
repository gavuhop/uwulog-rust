use crate::theme::ActiveTheme;
use eframe::egui::{self, Align2, Color32, CornerRadius, Id, Order, Stroke, Vec2};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use uwu_driver_transport::WslTransport;

type DirCacheMap = HashMap<(String, String), Vec<String>>;

/// Bộ nhớ đệm danh sách thư mục từ xa để điều hướng tức thì (< 1ms)
static DIR_CACHE: LazyLock<Mutex<DirCacheMap>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn get_cached_or_read_directories(distro: &str, dir: &str) -> Vec<String> {
    let key = (distro.to_string(), dir.to_string());
    if let Ok(cache) = DIR_CACHE.lock() {
        if let Some(entries) = cache.get(&key) {
            return entries.clone();
        }
    }

    let entries = WslTransport::list_remote_directories(distro, dir).unwrap_or_default();
    if let Ok(mut cache) = DIR_CACHE.lock() {
        cache.insert(key, entries.clone());
    }
    entries
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
    pub icon: &'a str,
    pub label: &'a str,
    pub end_slot: Option<&'a str>,
    pub is_muted: bool,
    pub is_destructive: bool,
    pub is_success: bool,
    pub is_selected: bool,
    pub tooltip: Option<&'a str>,
}

impl<'a> ListItemRow<'a> {
    pub fn new(icon: &'a str, label: &'a str) -> Self {
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
                ui.add(
                    egui::Label::new(egui::RichText::new(self.icon).size(12.0).color(item_color))
                        .selectable(false),
                );

                ui.add_space(4.0);

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

/// Container dạng Zed Command Palette nổi giữa màn hình (học theo kiến trúc Area của PopoverContainer và suggestion)
/// Không bị giới hạn fixed_size của Window trong modal.rs, giúp nội dung co giãn tự nhiên
pub struct RemotePickerContainer<'a> {
    id: Id,
    width: f32,
    max_height: Option<f32>,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> RemotePickerContainer<'a> {
    pub fn new(id_str: &'a str) -> Self {
        Self {
            id: Id::new(id_str),
            width: 520.0,
            max_height: Some(480.0),
            _phantom: std::marker::PhantomData,
        }
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

        // 1. Lớp phủ nền tối mờ toàn màn hình (Backdrop scrim)
        let screen_rect = ctx.viewport_rect();
        egui::Area::new(self.id.with("_backdrop"))
            .order(Order::Middle)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(screen_rect.size(), egui::Sense::click());
                ui.painter()
                    .rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(150));
                if response.clicked() {
                    close_requested = true;
                }
            });

        // 2. Khung modal nổi ở giữa màn hình
        let theme = ctx.app_theme();
        egui::Area::new(self.id)
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
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
                        color: Color32::from_black_alpha(180),
                    });

                let total_margin = frame.total_margin();
                frame.show(ui, |ui| {
                    let inner_width = (self.width - total_margin.sum().x).max(0.0);
                    ui.set_width(inner_width);
                    if let Some(h) = self.max_height {
                        ui.set_max_height(h);
                    }
                    add_contents(ui);
                });
            });

        close_requested
    }
}
