use super::render_key_customizer_popup;
use crate::actions::AppAction;
use crate::app::overlay_manager::KeymapModalState;
use crate::components::ui::{
    AppButton, ButtonVariant, IconButton, IconName, ModalContainer, TextInput,
};
use crate::keymap::{format_key, Key, KeyAction, KeyContext, KeymapManager, Keystroke};
use crate::theme::ActiveTheme;
use eframe::egui::{self, CornerRadius, FontId, Pos2, Rect, Vec2};
use egui_extras::{Column, TableBuilder};

/// Mục hiển thị đại diện cho 1 binding phím tắt theo ngữ cảnh cụ thể (chuẩn kiến trúc Zed Editor)
#[derive(Clone)]
struct DisplayRow {
    action: KeyAction,
    #[allow(dead_code)]
    arguments: &'static str,
    keystroke: Option<Keystroke>,
    context: KeyContext,
    is_user: bool,
}

/// Render modal quản lý phím tắt phong cách bảng Zed Editor:
/// Các cột chuẩn Zed: ["", "Action", "Arguments", "Keystrokes", "Context", "Source"]
pub fn render_keymap_modal(
    ctx: &egui::Context,
    is_open: bool,
    state_opt: &mut Option<KeymapModalState>,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !is_open {
        return;
    }

    let Some(state) = state_opt else {
        return;
    };

    let theme = ctx.app_theme();
    let mut action_to_dispatch: Option<AppAction> = None;
    let mut should_reset_defaults = false;
    let mut should_apply = false;

    // 1. Quản lý trạng thái bắt phím (Customizer popup hoặc Search box recording)
    let is_recording_row = state.recording.is_some();
    let is_recording_search = state.search_recording;

    if is_recording_row || is_recording_search {
        let mut captured: Option<Keystroke> = None;
        let mut cancel_pressed = false;
        let mut enter_pressed = false;
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    // Escape
                    if *key == egui::Key::Escape && modifiers.is_none() {
                        cancel_pressed = true;
                        break;
                    }

                    // Enter (không có modifier)
                    if *key == egui::Key::Enter && modifiers.is_none() {
                        enter_pressed = true;
                        break;
                    }

                    if let Some(core_key) = Key::from_egui(*key) {
                        let ks = Keystroke {
                            key: core_key,
                            ctrl: modifiers.ctrl || modifiers.command,
                            alt: modifiers.alt,
                            shift: modifiers.shift,
                            mac_cmd: modifiers.mac_cmd,
                        };
                        captured = Some(ks);
                        break;
                    }
                }
            }
        });

        if cancel_pressed {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if is_recording_search {
                state.search_recording = false;
            } else if state.is_recording_keystroke {
                state.is_recording_keystroke = false;
            } else {
                state.recording = None;
                state.pending_keystroke = None;
                state.is_recording_keystroke = false;
                state.pending_context = KeyContext::Global;
                state.conflict_warning = None;
            }
        } else if enter_pressed {
            if is_recording_search {
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                state.search_recording = false;
            } else if state.is_recording_keystroke {
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                state.is_recording_keystroke = false;
            } else {
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                state.is_recording_keystroke = true;
            }
        } else if let Some(ks) = captured {
            if is_recording_search {
                if let Some(egui_key) = ks.key.to_egui() {
                    ctx.input_mut(|i| i.consume_key(ks.to_egui_modifiers(), egui_key));
                }
                // Điền phím vừa bấm vào ô tìm kiếm (Record Keys)
                state.search_query = format_keystroke_zed(&ks);
                state.search_recording = false;
            } else if state.is_recording_keystroke {
                if let Some(egui_key) = ks.key.to_egui() {
                    ctx.input_mut(|i| i.consume_key(ks.to_egui_modifiers(), egui_key));
                }
                // Cập nhật phím mới vừa bấm và dừng record
                state.pending_keystroke = Some(ks);
                state.is_recording_keystroke = false;
            }
        }

        ctx.request_repaint();
    }

    // 2. Modal Container dạng bảng (Responsive theo diện tích cửa sổ ứng dụng: nhỏ thì nhỏ, to thì to lên tương xứng)
    let screen = ctx.viewport_rect();
    let modal_width = (screen.width() * 0.82)
        .clamp(600.0, 1150.0)
        .min((screen.width() - 32.0).max(300.0));
    let modal_height = (screen.height() * 0.78)
        .clamp(280.0, 960.0)
        .min((screen.height() - 40.0).max(220.0));

    let resp = ModalContainer::new("keymap_modal_window", "Keyboard Shortcuts")
        .width(modal_width)
        .default_height(modal_height)
        .min_height(240.0)
        .resizable(true)
        .show(
            ctx,
            |ui| {
                // Top Search Bar (Search Box + Record Keys Button + Open JSON Button)
                ui.horizontal(|ui| {
                    let record_btn_active = state.search_recording;
                    let avail_w = ui.available_width();
                    let right_buttons_w = 72.0 + if state.search_query.is_empty() { 0.0 } else { 28.0 };
                    let input_w = (avail_w - right_buttons_w - 8.0).max(180.0);

                    let hint_text = if record_btn_active {
                        "Recording keys... (Press shortcut or Esc to cancel)"
                    } else {
                        "Type to search in keybindings (e.g. 'workspace', 'search', 'Ctrl-Enter')..."
                    };

                    TextInput::new(&mut state.search_query)
                        .id(egui::Id::new("keymap_modal_search_input"))
                        .interactive(!record_btn_active)
                        .auto_focus(!is_recording_row && !is_recording_search)
                        .hint_text(hint_text)
                        .width(input_w)
                        .show(ui);

                    if !state.search_query.is_empty()
                        && IconButton::new(IconName::Close)
                            .size(24.0)
                            .tooltip("Clear search query")
                            .show(ui)
                            .clicked()
                    {
                        state.search_query.clear();
                    }

                    // Nút Record Keys (⌨)
                    let record_tip = if record_btn_active {
                        "Recording keys... (Press any shortcut or Esc to cancel)"
                    } else {
                        "Record Keys (Press shortcut to search)"
                    };

                    let record_btn = IconButton::new("⌨")
                        .size(24.0)
                        .variant(if record_btn_active {
                            ButtonVariant::Selected
                        } else {
                            ButtonVariant::Ghost
                        })
                        .tooltip(record_tip);

                    if record_btn.show(ui).clicked() {
                        state.search_recording = !state.search_recording;
                        if state.search_recording {
                            state.search_query.clear();
                        }
                        state.recording = None;
                        state.pending_keystroke = None;
                    }

                    // Nút Open keymap.json (📄)
                    if IconButton::new(IconName::File)
                        .size(24.0)
                        .tooltip("Open keymap.json in system editor")
                        .show(ui)
                        .clicked()
                    {
                        open_keymap_file();
                    }
                });

                ui.add_space(8.0);

                // 3. Thu thập danh sách hàng hiển thị đầy đủ ngữ cảnh (Context-explicit)
                let filtered_rows = collect_display_rows(&state.draft, &state.search_query);

                // 4. Bảng Keyboard Shortcuts chuẩn 6 cột của Zed: ["", "Action", "Arguments", "Keystrokes", "Context", "Source"]
                let mut row_action_edit = None;
                let mut row_action_reset = None;
                let mut row_action_remove = None;

                // Chiều cao bảng phím tắt co giãn tương xứng theo chiều cao vùng nội dung:
                // Tự động lấp đầy chiều cao còn lại của modal và tự co giãn linh hoạt khi kéo zoom
                let table_height = ui.available_height().max(80.0);

                TableBuilder::new(ui)
                    .id_salt("keyboard_shortcuts_table_zed")
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(22.0))                                                // Cột 0: Edit Icon (chiều ngang = chiều cao 22x22)
                    .column(Column::initial(300.0).at_least(180.0).resizable(true).clip(true)) // Cột 1: Action
                    // Tạm ẩn cột Arguments (để phòng sau này dùng lại khi hỗ trợ parameterized actions):
                    // .column(Column::initial(90.0).at_least(50.0).resizable(true).clip(true))   // Arguments
                    .column(Column::initial(190.0).at_least(130.0).resizable(true).clip(true)) // Cột 2: Keystrokes
                    .column(Column::remainder().at_least(180.0).clip(true))                     // Cột 3: Context (chiếm toàn bộ phần còn lại)
                    .column(Column::exact(80.0))                                                // Cột 4: Source (độ rộng vừa đủ cho chữ và icon)
                    .min_scrolled_height(80.0)
                    .max_scroll_height(table_height)
                    .header(22.0, |mut header| {
                        header.col(|_| {}); // Cột 0 rỗng theo chuẩn header của Zed: vec!["", "Action", ...]
                        header.col(|ui| {
                            let cell_rect = ui.max_rect();
                            ui.painter().text(
                                Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                "Action",
                                FontId::proportional(11.5),
                                theme.text.primary,
                            );
                        });
                        // Tạm ẩn cột Arguments trong header:
                        /*
                        header.col(|ui| {
                            let cell_rect = ui.max_rect();
                            ui.painter().text(
                                Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                "Arguments",
                                FontId::proportional(11.5),
                                theme.text.primary,
                            );
                        });
                        */
                        header.col(|ui| {
                            let cell_rect = ui.max_rect();
                            ui.painter().text(
                                Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                "Keystrokes",
                                FontId::proportional(11.5),
                                theme.text.primary,
                            );
                        });
                        header.col(|ui| {
                            let cell_rect = ui.max_rect();
                            ui.painter().text(
                                Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                "Context",
                                FontId::proportional(11.5),
                                theme.text.primary,
                            );
                        });
                        header.col(|ui| {
                            let cell_rect = ui.max_rect();
                            ui.painter().text(
                                Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                "Source",
                                FontId::proportional(11.5),
                                theme.text.primary,
                            );
                        });
                    })
                    .body(|body| {
                        body.rows(22.0, filtered_rows.len(), |mut row| {
                            let row_idx = row.index();
                            let item = &filtered_rows[row_idx];

                            // Cột 0: Icon Button Slot (Pencil ✏️ hiển thị khi hover dòng - chuẩn create_row_button của Zed)
                            row.col(|ui| {
                                let cell_rect = ui.max_rect();
                                let is_row_hovered = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().is_some_and(|p| {
                                        p.y >= cell_rect.min.y && p.y <= cell_rect.max.y
                                    })
                                });

                                if is_row_hovered {
                                    ui.painter().rect_filled(cell_rect, CornerRadius::ZERO, theme.log.row_hover);

                                    let icon_rect = Rect::from_center_size(cell_rect.center(), Vec2::splat(16.0));
                                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(icon_rect));
                                    if IconButton::new(IconName::Pencil)
                                        .size(13.0)
                                        .tooltip("Edit keybinding")
                                        .show(&mut child_ui)
                                        .clicked()
                                    {
                                        row_action_edit = Some((
                                            item.action.clone(),
                                            item.context,
                                            item.keystroke,
                                        ));
                                    }
                                }
                            });

                            // Cột 1: Action (Tên humanized chuẩn Zed như "workspace: toggle project picker")
                            row.col(|ui| {
                                let cell_rect = ui.max_rect();
                                let is_row_hovered = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().is_some_and(|p| {
                                        p.y >= cell_rect.min.y && p.y <= cell_rect.max.y
                                    })
                                });
                                if is_row_hovered {
                                    ui.painter().rect_filled(cell_rect, CornerRadius::ZERO, theme.log.row_hover);
                                }

                                let resp = ui.allocate_rect(cell_rect, egui::Sense::click());
                                let action_str = humanize_action_name(&item.action);

                                ui.painter().text(
                                    Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &action_str,
                                    FontId::proportional(11.5),
                                    theme.text.primary,
                                );

                                resp.clone().on_hover_text(format!(
                                    "{}\n{}",
                                    item.action.display_name(),
                                    item.action.description()
                                ));

                                if resp.double_clicked() {
                                    row_action_edit = Some((
                                        item.action.clone(),
                                        item.context,
                                        item.keystroke,
                                    ));
                                }
                            });

                            // Tạm ẩn cột Arguments trong dữ liệu hàng (để phòng sau này dùng lại):
                            /*
                            row.col(|ui| {
                                let cell_rect = ui.max_rect();
                                let is_row_hovered = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().map_or(false, |p| {
                                        p.y >= cell_rect.min.y && p.y <= cell_rect.max.y
                                    })
                                });
                                if is_row_hovered {
                                    ui.painter().rect_filled(cell_rect, CornerRadius::ZERO, theme.log.row_hover);
                                }

                                if !item.arguments.is_empty() {
                                    ui.painter().text(
                                        Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        item.arguments,
                                        FontId::proportional(11.0),
                                        theme.text.muted,
                                    );
                                }
                            });
                            */

                            // Cột 2: Keystrokes (Chuẩn định dạng Ctrl-Alt-Shift-PageDown của Zed)
                            row.col(|ui| {
                                let cell_rect = ui.max_rect();
                                let is_row_hovered = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().is_some_and(|p| {
                                        p.y >= cell_rect.min.y && p.y <= cell_rect.max.y
                                    })
                                });
                                if is_row_hovered {
                                    ui.painter().rect_filled(cell_rect, CornerRadius::ZERO, theme.log.row_hover);
                                }

                                let resp = ui.allocate_rect(cell_rect, egui::Sense::click());
                                if resp.double_clicked() {
                                    row_action_edit = Some((
                                        item.action.clone(),
                                        item.context,
                                        item.keystroke,
                                    ));
                                }

                                let (ks_str, color) = if let Some(ref ks) = item.keystroke {
                                    (format_keystroke_zed(ks), theme.text.primary)
                                } else {
                                    ("<none>".to_string(), theme.text.muted)
                                };

                                ui.painter().text(
                                    Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    ks_str,
                                    FontId::proportional(11.5),
                                    color,
                                );
                            });

                            // Cột 3: Context (Đầy đủ và rõ ràng: Workspace > SearchBar > Autocomplete)
                            row.col(|ui| {
                                let cell_rect = ui.max_rect();
                                let is_row_hovered = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().is_some_and(|p| {
                                        p.y >= cell_rect.min.y && p.y <= cell_rect.max.y
                                    })
                                });
                                if is_row_hovered {
                                    ui.painter().rect_filled(cell_rect, CornerRadius::ZERO, theme.log.row_hover);
                                }

                                let resp = ui.allocate_rect(cell_rect, egui::Sense::click());
                                if resp.double_clicked() {
                                    row_action_edit = Some((
                                        item.action.clone(),
                                        item.context,
                                        item.keystroke,
                                    ));
                                }

                                let ctx_str = format_context_path(item.context);
                                ui.painter().text(
                                    Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    ctx_str,
                                    FontId::proportional(11.0),
                                    theme.text.muted,
                                );
                            });

                            // Cột 4: Source (Default vs User) + Action buttons khi hover
                            row.col(|ui| {
                                let cell_rect = ui.max_rect();
                                let is_row_hovered = ui.ctx().input(|i| {
                                    i.pointer.hover_pos().is_some_and(|p| {
                                        p.y >= cell_rect.min.y && p.y <= cell_rect.max.y
                                    })
                                });
                                if is_row_hovered {
                                    ui.painter().rect_filled(cell_rect, CornerRadius::ZERO, theme.log.row_hover);
                                }

                                let (source_text, source_color) = if item.is_user {
                                    ("User", theme.text.accent)
                                } else {
                                    ("Default", theme.text.muted)
                                };

                                ui.painter().text(
                                    Pos2::new(cell_rect.min.x + 6.0, cell_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    source_text,
                                    FontId::proportional(11.0),
                                    source_color,
                                );

                                if is_row_hovered {
                                    let right_margin = 12.0;
                                    let btn_size = 16.0;
                                    let btn_rect = Rect::from_center_size(
                                        Pos2::new(cell_rect.max.x - right_margin, cell_rect.center().y),
                                        Vec2::splat(btn_size),
                                    );
                                    let mut btn_ui = ui.new_child(egui::UiBuilder::new().max_rect(btn_rect));

                                    if item.is_user {
                                        // Khôi phục về mặc định
                                        if IconButton::new(IconName::Restart)
                                            .size(13.0)
                                            .tooltip("Reset to Default")
                                            .show(&mut btn_ui)
                                            .clicked()
                                        {
                                            row_action_reset = Some((
                                                item.action.clone(),
                                                item.context,
                                            ));
                                        }
                                    } else if item.keystroke.is_some() {
                                        // Gỡ phím tắt (Unbind)
                                        if IconButton::new(IconName::Trash)
                                            .size(13.0)
                                            .tooltip("Remove shortcut")
                                            .show(&mut btn_ui)
                                            .clicked()
                                        {
                                            row_action_remove = Some((
                                                item.action.clone(),
                                                item.context,
                                                item.keystroke,
                                            ));
                                        }
                                    }
                                }
                            });
                        });
                    });

                if filtered_rows.is_empty() {
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No keybindings matching your search.")
                                .size(12.0)
                                .color(theme.text.muted),
                        );
                    });
                }

                // Xử lý các thao tác phát sinh từ các hàng của bảng
                if let Some((action, context, keystroke)) = row_action_edit {
                    state.recording = Some((action, context));
                    state.pending_keystroke = keystroke;
                    state.pending_context = context;
                    state.is_recording_keystroke = false;
                    state.conflict_warning = None;
                }
                if let Some((action, context)) = row_action_reset {
                    let def_keys = KeymapManager::new().keystrokes_for_action(&action, context);
                    state.draft.remove_action_binding(&action, context);
                    for def_ks in def_keys {
                        state.draft.bind_keystroke(def_ks, action.clone(), context);
                    }
                }
                if let Some((action, context, ks_opt)) = row_action_remove {
                    if let Some(ks) = ks_opt {
                        state.draft.remove_keystroke(&ks, context);
                    } else {
                        state.draft.remove_action_binding(&action, context);
                    }
                }
            },
            Some(|ui: &mut egui::Ui, close_req: &mut bool| {
                // Footer Bar
                if AppButton::new()
                    .label("Reset All Defaults")
                    .icon(IconName::Restart)
                    .tooltip("Restore all shortcuts to factory system defaults")
                    .show(ui)
                    .clicked()
                {
                    should_reset_defaults = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if AppButton::new()
                        .label("Save & Apply")
                        .icon(IconName::Check)
                        .variant(ButtonVariant::Success)
                        .tooltip("Save changes to keymap.json and apply immediately")
                        .show(ui)
                        .clicked()
                    {
                        should_apply = true;
                        *close_req = true;
                    }

                    ui.add_space(6.0);

                    if AppButton::new()
                        .label("Cancel")
                        .show(ui)
                        .clicked()
                    {
                        *close_req = true;
                    }
                });
            }),
        );

    // 5. Popup tùy chỉnh phím tắt nổi ở giữa màn hình (chuẩn phong cách KeybindingEditorModal của Zed)
    if state.recording.is_some() {
        render_key_customizer_popup(ctx, &theme, state);
    }

    if should_apply {
        action_to_dispatch = Some(AppAction::ApplyKeymapModal(Box::new(state.draft.clone())));
    } else if should_reset_defaults {
        state.draft.clear();
        state.draft.register_defaults();
        state.conflict_warning = None;
    }

    if resp.closed && !should_apply && state.recording.is_none() {
        action_to_dispatch = Some(AppAction::CloseKeymapModal);
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}

/// Thu thập danh sách hàng hiển thị cho bảng keymap với ngữ cảnh phân cấp rõ ràng
fn collect_display_rows(draft: &KeymapManager, search_query: &str) -> Vec<DisplayRow> {
    let default_mgr = KeymapManager::new();
    let mut rows = Vec::new();
    let mut actions_with_bindings = std::collections::HashSet::new();

    // 1. Thu thập tất cả các binding đang tồn tại trong draft
    for b in draft.bindings() {
        if b.action == KeyAction::Unbind {
            continue;
        }
        actions_with_bindings.insert((b.action.clone(), b.context));

        let default_keys = default_mgr.keystrokes_for_action(&b.action, b.context);
        let is_user = !default_keys.contains(&b.keystroke);

        rows.push(DisplayRow {
            action: b.action.clone(),
            arguments: "",
            keystroke: Some(b.keystroke),
            context: b.context,
            is_user,
        });
    }

    // 2. Bổ sung các Action chưa được gán phím nào trong ngữ cảnh tự nhiên của nó
    for action in KeyAction::all() {
        if *action == KeyAction::Unbind {
            continue;
        }
        let natural_ctx = default_context_for_action(action);
        if !actions_with_bindings.contains(&(action.clone(), natural_ctx)) {
            rows.push(DisplayRow {
                action: action.clone(),
                arguments: "",
                keystroke: None,
                context: natural_ctx,
                is_user: false,
            });
        }
    }

    // 3. Sắp xếp danh sách: theo tên Action, sau đó theo Context
    rows.sort_by(|a, b| {
        let a_str = humanize_action_name(&a.action);
        let b_str = humanize_action_name(&b.action);
        a_str
            .cmp(&b_str)
            .then_with(|| (a.context as u8).cmp(&(b.context as u8)))
    });

    // 4. Lọc theo search_query (nếu có)
    let query = search_query.trim().to_lowercase();
    if !query.is_empty() {
        rows.retain(|r| {
            let action_name = humanize_action_name(&r.action).to_lowercase();
            let display_name = r.action.display_name().to_lowercase();
            let desc = r.action.description().to_lowercase();
            let ctx_str = format_context_path(r.context).to_lowercase();
            let ks_str = r
                .keystroke
                .as_ref()
                .map(format_keystroke_zed)
                .unwrap_or_default()
                .to_lowercase();
            let source_str = if r.is_user { "user" } else { "default" };

            action_name.contains(&query)
                || display_name.contains(&query)
                || desc.contains(&query)
                || ctx_str.contains(&query)
                || ks_str.contains(&query)
                || source_str.contains(&query)
        });
    }

    rows
}

/// Chuyển đổi tên Action canonical sang tên humanized chuẩn Zed Editor (dựa trên thuật toán `command_palette::humanize_action_name` của Zed)
/// Ví dụ: `workspace::ToggleProjectPicker` -> `workspace: toggle project picker`
pub(crate) fn humanize_action_name(action: &KeyAction) -> String {
    let canonical = action.canonical_name();
    let mut result = String::with_capacity(canonical.len() + 4);
    let mut prev_char: Option<char> = None;
    let mut in_name_part = false;

    for c in canonical.chars() {
        if c == ':' {
            if !result.ends_with(':') {
                result.push(':');
            } else if !result.ends_with(": ") {
                result.push(' ');
                in_name_part = true;
            }
        } else if in_name_part {
            if c.is_uppercase() {
                if let Some(p) = prev_char {
                    if !p.is_uppercase() && p != ' ' && p != ':' {
                        result.push(' ');
                    }
                }
                result.extend(c.to_lowercase());
            } else {
                result.push(c);
            }
        } else {
            result.extend(c.to_lowercase());
        }
        prev_char = Some(c);
    }

    if !in_name_part && !result.contains(':') {
        let cat = match action.category() {
            "Window" => "window",
            "Workspace" => "workspace",
            "Search" => "search",
            "Navigation" => "menu",
            _ => "app",
        };
        return format!("{}: {}", cat, action.display_name().to_lowercase());
    }

    result
}

/// Định dạng chuỗi phím bấm phong cách Zed: `Ctrl-Alt-Shift-PageDown`, `Shift-Enter`, `Enter`, `F3`
pub(crate) fn format_keystroke_zed(ks: &Keystroke) -> String {
    let mut parts = Vec::new();
    if ks.ctrl {
        parts.push("Ctrl");
    }
    if ks.alt {
        parts.push("Alt");
    }
    if ks.shift {
        parts.push("Shift");
    }
    if ks.mac_cmd {
        parts.push("Cmd");
    }

    let key_str = match ks.key {
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::Enter => "Enter",
        Key::Escape => "Escape",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Insert => "Insert",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Equals => "=",
        Key::Plus => "+",
        Key::Minus => "-",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Backslash => "\\",
        Key::Semicolon => ";",
        Key::Quote => "'",
        _ => format_key(ks.key),
    };
    parts.push(key_str);
    parts.join("-")
}

/// Định dạng đường dẫn ngữ cảnh phân cấp rõ ràng (phong cách Zed: `Workspace > SearchBar > Autocomplete`)
pub(crate) fn format_context_path(context: KeyContext) -> &'static str {
    match context {
        KeyContext::Global => "Workspace",
        KeyContext::Table => "Workspace > Table",
        KeyContext::SearchInput => "Workspace > SearchBar",
        KeyContext::Autocomplete => "Workspace > SearchBar > Autocomplete",
        KeyContext::Modal => "Modal",
        KeyContext::RemoteServers => "Modal > RemoteServers",
    }
}

/// Mở file keymap.json bằng trình chỉnh sửa mặc định của hệ thống
fn open_keymap_file() {
    let cfg_path = crate::keymap::default_config_path();
    let _ = crate::keymap::ensure_sample_config_file(None);

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &cfg_path.to_string_lossy()])
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&cfg_path).spawn();
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(&cfg_path)
            .spawn();
    }
}

/// Xác định ngữ cảnh mặc định tự nhiên của từng hành động
fn default_context_for_action(action: &KeyAction) -> KeyContext {
    match action {
        KeyAction::CommitSearch | KeyAction::ClearSearch => KeyContext::SearchInput,
        KeyAction::SelectNext | KeyAction::SelectPrev | KeyAction::ConfirmSelection => {
            KeyContext::Autocomplete
        }
        KeyAction::Back | KeyAction::TabComplete => KeyContext::RemoteServers,
        _ => KeyContext::Global,
    }
}
