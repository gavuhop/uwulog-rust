pub mod counter;
pub mod search_bar;
pub mod toolbar;
pub mod window_controls;

pub use counter::render_log_counter;
pub use search_bar::render_search_bar;
pub use toolbar::render_toolbar;
pub use window_controls::render_window_controls;

use crate::actions::AppAction;
use crate::overlay::{OverlayLayer, OverlayStack};
use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::theme;
use crate::views::modals::ProjectPickerSessionInfo;
use eframe::egui::{self, Color32, Rounding, Stroke};
use uwu_core_workspace::WorkspaceStore;

/// Context chứa thông tin cấp App cần thiết cho Header & Popups liên quan
pub struct HeaderContext<'a> {
    pub overlay_stack: &'a mut OverlayStack,
    pub store: &'a WorkspaceStore,
    pub sessions: &'a [ProjectPickerSessionInfo],
    pub active_index: usize,
    pub project_search_query: &'a mut String,
}

pub fn render_header(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    cx: &mut HeaderContext<'_>,
    dispatch: &mut impl FnMut(AppAction),
) {
    let mut proj_btn_rect = None;
    let mut menu_btn_rect = None;

    ui.horizontal(|ui| {
        // 1. Menu Icon Button (☰) - Flat style
        let is_menu_open = cx.overlay_stack.is_open(OverlayLayer::MainMenu);
        let menu_btn = egui::Button::new(egui::RichText::new("☰").strong().size(13.5).color(
            if is_menu_open {
                theme::TEXT_KEY
            } else {
                theme::TEXT_PRIMARY
            },
        ))
        .fill(if is_menu_open {
            theme::BG_SURFACE1
        } else {
            Color32::TRANSPARENT
        })
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(4.0));

        let menu_resp = ui.add_sized([24.0, 22.0], menu_btn);
        menu_btn_rect = Some(menu_resp.rect);

        if menu_resp.on_hover_text("Open Application Menu").clicked() {
            dispatch(AppAction::ToggleMainMenu);
        }

        // Nhận diện môi trường remote để hiển thị badge
        if let Some(remote) = session.session.location.as_remote() {
            ui.add_space(2.0);
            let badge = egui::Label::new(
                egui::RichText::new(format!("{} {}", remote.icon(), remote.display_name()))
                    .size(11.0)
                    .strong()
                    .color(theme::COLOR_INFO),
            );
            ui.add(badge).on_hover_text(format!(
                "Connected to {}: {}",
                remote.connection_type().to_uppercase(),
                remote.display_name()
            ));
        }

        // Project Button
        let is_project_picker_open = cx.overlay_stack.is_open(OverlayLayer::ProjectPicker);
        if !session.session.name.is_empty() {
            let proj_btn =
                egui::Button::new(egui::RichText::new(&session.session.name).strong().color(
                    if is_project_picker_open {
                        theme::TEXT_KEY
                    } else {
                        theme::TEXT_PRIMARY
                    },
                ))
                .fill(if is_project_picker_open {
                    theme::BG_SURFACE1
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(4.0));

            let proj_resp = ui.add(proj_btn);
            proj_btn_rect = Some(proj_resp.rect);

            let summary = session.session.target_summary();
            let tooltip = if summary.is_empty() {
                format!(
                    "Project: {}\nSwitch or manage workspace projects (Alt+P)",
                    session.session.name
                )
            } else {
                format!(
                    "Project: {}\nPath: {}\nSwitch or manage workspace projects (Alt+P)",
                    session.session.name, summary
                )
            };

            if proj_resp.on_hover_text(tooltip).clicked() {
                dispatch(AppAction::ToggleProjectPicker);
            }
        }

        // Environment Status Indicator
        toolbar::render_environment_status(ui, session, dispatch);

        ui.add_space(4.0);

        // 2. Stream Tabs: Main / Filtered and Raw Stream
        let is_filtered_tab = session.view.active_tab == ActiveTab::Filtered;
        let is_unfiltered_tab = session.view.active_tab == ActiveTab::Unfiltered;
        let is_filtering = !session.view.search.query.trim().is_empty();

        let filtered_tab_text = if is_filtering {
            "🔍 Filtered".to_string()
        } else {
            "🔍 Main".to_string()
        };

        let tab_filtered_btn = stream_tab_button(filtered_tab_text, is_filtered_tab);

        if ui
            .add(tab_filtered_btn)
            .on_hover_text("Switch to Filtered Logs view")
            .clicked()
        {
            dispatch(AppAction::SwitchTab(ActiveTab::Filtered));
        }

        // Tab 2: Raw Stream (Chỉ xuất hiện khi người dùng đang có bộ lọc hoặc đang mở tab Raw)
        if is_filtering || is_unfiltered_tab {
            ui.add_space(2.0);

            let tab_unfil_btn = stream_tab_button("📄 Raw", is_unfiltered_tab);

            if ui
                .add(tab_unfil_btn)
                .on_hover_text("Switch to Raw Stream view (500 logs buffer)")
                .clicked()
            {
                dispatch(AppAction::SwitchTab(ActiveTab::Unfiltered));
            }
        }

        // 3. Phía bên phải: Window Controls, Toolbar & Search Box
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Nút điều khiển cửa sổ (—, 🗖/🗗, ✕)
            render_window_controls(ui);

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Các nút công cụ: Columns modal, Settings, Stop/Restart, Snapshot, Latch, Log counter
            render_toolbar(ui, session, dispatch);

            ui.add_space(6.0);

            // 4. Ở giữa: Search Box Command Palette Style
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                render_search_bar(ui, session, dispatch);
            });
        });
    });

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

fn stream_tab_button(text: impl Into<String>, is_active: bool) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text)
            .size(11.5)
            .strong()
            .color(if is_active {
                theme::TEXT_PRIMARY
            } else {
                theme::TEXT_MUTED
            }),
    )
    .fill(if is_active {
        theme::BG_SURFACE1
    } else {
        egui::Color32::TRANSPARENT
    })
    .stroke(Stroke::new(
        1.0,
        if is_active {
            theme::TEXT_KEY
        } else {
            theme::BG_SURFACE0
        },
    ))
    .rounding(Rounding::same(4.0))
}
