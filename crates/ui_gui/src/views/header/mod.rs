pub mod counter;
pub mod devcontainer;
pub mod search_bar;
pub mod toolbar;
pub mod window_controls;

pub use counter::render_log_counter;
pub use search_bar::render_search_bar;
pub use toolbar::render_toolbar;
pub use window_controls::{
    handle_titlebar_drag_interaction, render_left_window_controls, render_right_window_controls,
    render_titlebar_drag_spacer, render_window_resize_borders, WindowControlsPlatform,
    TITLEBAR_HEIGHT,
};

use crate::actions::AppAction;
use crate::overlay::{OverlayLayer, OverlayStack};
use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::theme::ActiveTheme;
use crate::views::modals::ProjectPickerSessionInfo;
use eframe::egui;
use uwu_core_workspace::WorkspaceStore;

/// Context chứa thông tin cấp App cần thiết cho Header & Popups liên quan
pub struct HeaderContext<'a> {
    pub overlay_stack: &'a mut OverlayStack,
    pub store: &'a WorkspaceStore,
    pub sessions: &'a [ProjectPickerSessionInfo],
    pub container_sessions: &'a [devcontainer::ActiveContainerSessionInfo],
    pub active_index: usize,
    pub project_search_query: &'a mut String,
    pub project_shortcut: &'a str,
    pub source_switcher_state: &'a mut devcontainer::SwitcherContainersState,
    pub rt: &'a tokio::runtime::Handle,
}

pub fn render_header(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    cx: &mut HeaderContext<'_>,
    dispatch: &mut impl FnMut(AppAction),
) {
    let theme = ui.app_theme();
    let mut proj_btn_rect = None;
    let mut menu_btn_rect = None;
    let mut source_btn_rect = None;

    // Bất kỳ chỗ trống nào trên thanh Header mà con trỏ chuột trỏ vào đều có thể kéo di chuyển cửa sổ và nhấp đúp để phóng to / thu nhỏ
    let header_rect = ui.max_rect();
    let mut bg_response = ui.interact(
        header_rect,
        ui.id().with("__header_titlebar_drag_bg"),
        egui::Sense::click_and_drag(),
    );
    bg_response = bg_response.on_hover_cursor(egui::CursorIcon::Default);
    if bg_response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }
    handle_titlebar_drag_interaction(ui.ctx(), &bg_response);

    let available_w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(available_w, TITLEBAR_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // Nút điều khiển cửa sổ phía bên trái (chỉ hiển thị trên macOS theo chuẩn Apple Traffic Lights của Zed)
            if render_left_window_controls(ui) {
                ui.add_space(4.0);
            }

            // 1. Menu Icon Button - Flat style
            let is_menu_open = cx.overlay_stack.is_open(OverlayLayer::MainMenu);
            let menu_resp =
                crate::components::ui::IconButton::new(crate::components::ui::IconName::Menu)
                    .size(24.0)
                    .selected(is_menu_open)
                    .tooltip("Open Application Menu")
                    .show(ui);
            menu_btn_rect = Some(menu_resp.rect);

            if menu_resp.clicked() {
                dispatch(AppAction::ToggleMainMenu);
            }

            let is_docker_session = matches!(
                session.session.location.as_remote(),
                Some(uwu_core_workspace::RemoteConnectionOptions::Docker(_))
            );

            let mut has_breadcrumb_item = false;

            // Nhận diện môi trường remote (WSL / SSH) hiển thị trước nút Project;
            // Với Docker / Dev Container: icon + chấm xanh nằm gộp ngay trong nút Project Picker.
            if let Some(remote) = session.session.location.as_remote() {
                if !is_docker_session {
                    let remote_tip = format!(
                        "Connected to {}: {}\nSwitch or manage remote servers",
                        remote.connection_type().to_uppercase(),
                        remote.display_name()
                    );
                    let is_remote_open = cx.overlay_stack.is_open(OverlayLayer::RemoteServersModal);
                    let remote_variant = if is_remote_open {
                        crate::components::ui::ButtonVariant::Selected
                    } else {
                        crate::components::ui::ButtonVariant::Ghost
                    };
                    let remote_resp = crate::components::ui::AppButton::new()
                        .icon(remote.icon())
                        .icon_gap(9.0)
                        .label(remote.short_label())
                        .status_dot(theme.status.success)
                        .variant(remote_variant)
                        .text_color(if is_remote_open {
                            theme.text.accent
                        } else {
                            theme.text.primary
                        })
                        .tooltip(&remote_tip)
                        .show(ui);

                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("server_button_rect"), remote_resp.rect);
                    });

                    if remote_resp.clicked() {
                        dispatch(AppAction::ToggleRemoteServersModal);
                    }
                    has_breadcrumb_item = true;
                }
            }

            // Project Button (nếu là Dev Container thì hiển thị thêm icon Box + chấm xanh ngay đầu nút)
            let is_project_picker_open = cx.overlay_stack.is_open(OverlayLayer::ProjectPicker);
            if !session.session.name.is_empty() {
                if has_breadcrumb_item {
                    render_header_slash_separator(ui, theme.text.muted);
                }

                let summary = session.session.target_summary();
                let shortcut_suffix = if cx.project_shortcut.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", cx.project_shortcut)
                };
                let tooltip = if summary.is_empty() {
                    format!(
                        "Project: {}\nSwitch or manage workspace projects{}",
                        session.session.name, shortcut_suffix
                    )
                } else {
                    format!(
                        "Project: {}\nPath: {}\nSwitch or manage workspace projects{}",
                        session.session.name, summary, shortcut_suffix
                    )
                };

                let proj_variant = if is_project_picker_open {
                    crate::components::ui::ButtonVariant::Selected
                } else {
                    crate::components::ui::ButtonVariant::Ghost
                };

                let mut proj_btn = crate::components::ui::AppButton::new()
                    .label(&session.session.name)
                    .variant(proj_variant)
                    .text_color(if is_project_picker_open {
                        theme.text.accent
                    } else {
                        theme.text.primary
                    })
                    .tooltip(&tooltip);
                if is_docker_session {
                    proj_btn = proj_btn
                        .icon(crate::components::ui::IconName::Box)
                        .icon_gap(9.0)
                        .status_dot(theme.status.success);
                }
                let proj_resp = proj_btn.show(ui);
                proj_btn_rect = Some(proj_resp.rect);
                has_breadcrumb_item = true;

                if proj_resp.clicked() {
                    dispatch(AppAction::ToggleProjectPicker);
                }
            }

            // Chỉ báo nguồn log + Quick Switcher (cho Dev Container / Docker)
            if has_breadcrumb_item
                && is_docker_session
                && session.session.active_docker_short_label().is_some()
            {
                render_header_slash_separator(ui, theme.text.muted);
            }
            source_btn_rect = devcontainer::render_docker_source_switcher_pill(
                ui,
                session,
                cx.overlay_stack,
                dispatch,
            );
            if source_btn_rect.is_some() {
                has_breadcrumb_item = true;
            }

            // Environment Status Indicator
            toolbar::render_environment_status(ui, session, dispatch);

            // 2. Stream Tabs: Main / Filtered and Raw Stream
            if has_breadcrumb_item {
                render_header_slash_separator(ui, theme.text.muted);
            }
            let is_filtered_tab = session.view.active_tab == ActiveTab::Filtered;
            let is_unfiltered_tab = session.view.active_tab == ActiveTab::Unfiltered;
            let is_filtering = !session.view.search.query.trim().is_empty();

            let filtered_tab_text = if is_filtering { "Filtered" } else { "Main" };

            let filtered_tooltip = if is_filtering {
                "Switch to Filtered Logs view (Alt+1, Ctrl+Tab)"
            } else {
                "Main Logs view"
            };

            if crate::components::ui::TabButton::new(filtered_tab_text, is_filtered_tab)
                .active_background(false)
                .show(ui)
                .on_hover_text(filtered_tooltip)
                .clicked()
            {
                dispatch(AppAction::SwitchTab(ActiveTab::Filtered));
            }

            // Tab 2: Raw Stream (Chỉ xuất hiện khi người dùng đang có bộ lọc)
            if is_filtering {
                render_header_pipe_separator(ui, theme.borders.border);
                if crate::components::ui::TabButton::new("Raw", is_unfiltered_tab)
                    .active_background(false)
                    .show(ui)
                    .on_hover_text("Switch to Raw Stream view (Alt+2, Ctrl+Tab)")
                    .clicked()
                {
                    dispatch(AppAction::SwitchTab(ActiveTab::Unfiltered));
                }
            }

            // 3. Phía bên phải: Window Controls, Toolbar & Search Box
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Giữ nguyên item_spacing mặc định cho thanh filter và các nút toolbar bên phải
                ui.spacing_mut().item_spacing.x = ui.style().spacing.item_spacing.x;
                // Nút điều khiển cửa sổ phía bên phải (Windows / Linux chuẩn Zed)
                if render_right_window_controls(ui) {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                }

                // Các nút công cụ: Columns modal, Settings, Stop/Restart, Snapshot, Latch, Log counter
                let is_columns_open = cx.overlay_stack.is_open(OverlayLayer::ColumnsModal);
                render_toolbar(ui, session, is_columns_open, dispatch);

                ui.add_space(2.0);

                // 4. Ở giữa: Search Box Command Palette Style
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    render_search_bar(ui, session, dispatch);
                });
            });
        },
    );

    // Render Main Menu Popover
    if let Some(rect) = menu_btn_rect {
        crate::components::render_main_menu_popup(ui.ctx(), cx.overlay_stack, dispatch, rect);
    }

    // Render Zed-Style Project Picker Popover
    if let Some(rect) = proj_btn_rect {
        let is_open = cx.overlay_stack.is_open(OverlayLayer::ProjectPicker);
        let args = crate::views::modals::ProjectPickerArgs {
            is_open,
            store: cx.store,
            sessions: cx.sessions,
            active_index: cx.active_index,
            project_search_query: cx.project_search_query,
            trigger_rect: rect,
        };
        crate::views::modals::render_project_picker_popup(ui.ctx(), args, dispatch);
    }

    // Render Docker / Dev Container Quick Source Switcher Popover
    if let Some(rect) = source_btn_rect {
        devcontainer::render_source_switcher_popup(
            ui.ctx(),
            session,
            cx.container_sessions,
            cx.overlay_stack,
            cx.source_switcher_state,
            cx.rt,
            dispatch,
            rect,
        );
    }
}

/// Render Header tinh giản khi ứng dụng chưa mở dự án nào
pub fn render_empty_header(
    ui: &mut egui::Ui,
    cx: &mut HeaderContext<'_>,
    dispatch: &mut impl FnMut(AppAction),
) {
    let theme = ui.app_theme();
    let mut proj_btn_rect = None;
    let mut menu_btn_rect = None;

    let header_rect = ui.max_rect();
    let mut bg_response = ui.interact(
        header_rect,
        ui.id().with("__header_titlebar_drag_bg"),
        egui::Sense::click_and_drag(),
    );
    bg_response = bg_response.on_hover_cursor(egui::CursorIcon::Default);
    if bg_response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }
    handle_titlebar_drag_interaction(ui.ctx(), &bg_response);

    let available_w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(available_w, TITLEBAR_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            if render_left_window_controls(ui) {
                ui.add_space(4.0);
            }

            // 1. Menu Icon Button
            let is_menu_open = cx.overlay_stack.is_open(OverlayLayer::MainMenu);
            let menu_resp =
                crate::components::ui::IconButton::new(crate::components::ui::IconName::Menu)
                    .size(24.0)
                    .selected(is_menu_open)
                    .tooltip("Open Application Menu")
                    .show(ui);
            menu_btn_rect = Some(menu_resp.rect);

            if menu_resp.clicked() {
                dispatch(AppAction::ToggleMainMenu);
            }

            // 2. Project Button (No Project Open)
            let is_project_picker_open = cx.overlay_stack.is_open(OverlayLayer::ProjectPicker);
            let shortcut_suffix = if cx.project_shortcut.is_empty() {
                String::new()
            } else {
                format!(" ({})", cx.project_shortcut)
            };
            let tooltip = format!("Switch or open workspace projects{}", shortcut_suffix);
            let proj_variant = if is_project_picker_open {
                crate::components::ui::ButtonVariant::Selected
            } else {
                crate::components::ui::ButtonVariant::Ghost
            };
            let proj_resp = crate::components::ui::AppButton::new()
                .label("No Project Open")
                .variant(proj_variant)
                .text_color(if is_project_picker_open {
                    theme.text.accent
                } else {
                    theme.text.muted
                })
                .tooltip(&tooltip)
                .show(ui);
            proj_btn_rect = Some(proj_resp.rect);

            if proj_resp.clicked() {
                dispatch(AppAction::ToggleProjectPicker);
            }

            // 3. Phía bên phải: Window Controls
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = ui.style().spacing.item_spacing.x;
                render_right_window_controls(ui);
            });
        },
    );

    // Render Main Menu Popover
    if let Some(rect) = menu_btn_rect {
        crate::components::render_main_menu_popup(ui.ctx(), cx.overlay_stack, dispatch, rect);
    }

    // Render Zed-Style Project Picker Popover
    if let Some(rect) = proj_btn_rect {
        let is_open = cx.overlay_stack.is_open(OverlayLayer::ProjectPicker);
        let args = crate::views::modals::ProjectPickerArgs {
            is_open,
            store: cx.store,
            sessions: cx.sessions,
            active_index: cx.active_index,
            project_search_query: cx.project_search_query,
            trigger_rect: rect,
        };
        crate::views::modals::render_project_picker_popup(ui.ctx(), args, dispatch);
    }
}

/// Vẽ dấu `/` mảnh phân tách breadcrumb phân cấp trên Header (`Remote / Project / Container / View`).
fn render_header_slash_separator(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 20.0), egui::Sense::hover());
    let c = rect.center();
    ui.painter().line_segment(
        [
            egui::pos2(c.x + 2.0, c.y - 5.5),
            egui::pos2(c.x - 2.0, c.y + 5.5),
        ],
        egui::Stroke::new(0.8, color.gamma_multiply(0.75)),
    );
}

/// Vẽ dấu `|` phân tách hai tab ngang hàng (`Filtered | Raw`) trên Header.
fn render_header_pipe_separator(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(4.0, 20.0), egui::Sense::hover());
    let c = rect.center();
    ui.painter().line_segment(
        [egui::pos2(c.x, c.y - 5.5), egui::pos2(c.x, c.y + 5.5)],
        egui::Stroke::new(1.0, color),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::OverlayStack;
    use uwu_core_workspace::{
        SourceConfig, SourceType, WorkspaceLocation, WorkspaceSession, WorkspaceStore,
    };

    fn measure_counter_gaps(query: &str) -> (f32, f32) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let ctx = egui::Context::default();
        crate::theme::setup_fonts(&ctx);
        let ws = WorkspaceSession::new(
            "psittacus-erithacus".to_string(),
            WorkspaceLocation::local("/tmp"),
            SourceConfig {
                source_type: SourceType::Process,
                command_str: String::new(),
                file_path: String::new(),
                working_dir: String::new(),
                capacity: 100,
                display_limit: 50,
            },
        );
        let mut session = GuiSession::new(ws);
        session.view.search.query = query.to_string();
        let mut stack = OverlayStack::new();
        let store = WorkspaceStore::default();
        let mut q = String::new();
        let mut switcher_state = devcontainer::SwitcherContainersState::default();

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };

        let mut output = ctx.run_ui(raw_input, |ui| {
            let mut cx = HeaderContext {
                overlay_stack: &mut stack,
                store: &store,
                sessions: &[],
                container_sessions: &[],
                active_index: 0,
                project_search_query: &mut q,
                project_shortcut: "",
                source_switcher_state: &mut switcher_state,
                rt: rt.handle(),
            };
            let mut dispatch = |_| {};
            render_header(ui, &mut session, &mut cx, &mut dispatch);
        });
        output.textures_delta.clear();

        let rects: Vec<egui::Rect> = output
            .shapes
            .iter()
            .map(|s| s.shape.visual_bounding_rect())
            .collect();

        // Nút Live/Paused là pill wide > 60px nằm quanh x ~ 1000..1120
        let live_pill = rects
            .iter()
            .find(|r| r.width() > 60.0 && r.width() < 120.0 && r.min.x > 950.0)
            .expect("live/paused pill rect");
        // Counter text nằm ngay bên trái live_pill
        let counter_text = rects
            .iter()
            .filter(|r| r.max.x < live_pill.min.x && r.min.x > 950.0)
            .max_by(|a, b| a.max.x.partial_cmp(&b.max.x).unwrap())
            .expect("counter text rect");
        // Icon history nằm ngay bên trái counter_text
        let history_glyph = rects
            .iter()
            .filter(|r| r.max.x < counter_text.min.x && r.min.x > 900.0)
            .max_by(|a, b| a.max.x.partial_cmp(&b.max.x).unwrap())
            .expect("history glyph rect");

        let left_gap = counter_text.min.x - history_glyph.max.x;
        let right_gap = live_pill.min.x - counter_text.max.x;
        (left_gap, right_gap)
    }

    #[test]
    fn test_header_log_counter_has_balanced_horizontal_gaps() {
        for query in ["err", ""] {
            let (left_gap, right_gap) = measure_counter_gaps(query);
            assert!(
                (left_gap - right_gap).abs() <= 2.5,
                "query={query:?}: expected balanced counter gaps, got left={left_gap:.1} right={right_gap:.1}"
            );
        }
    }
}
