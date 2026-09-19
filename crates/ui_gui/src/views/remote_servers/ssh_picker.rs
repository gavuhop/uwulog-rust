use crate::components::ui::{AppButton, IconName, TextInput};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use crate::theme::ActiveTheme;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use uwu_core_workspace::{SshConnectionOptions, WorkspaceStore};
use uwu_driver_transport::{SshInteractiveEvent, SshInteractivePromptType, SshTransport};

use super::helpers::{
    anchor_cursor_to_end, calculate_adaptive_scroll_height, step_selected_index, ListItemRow,
};
use super::types::{
    FolderPickerState, RemoteNavAction, RemoteServerKind, RemoteSubView, SshPickerStage,
    SshPickerState,
};

/// Trạng thái phiên tương tác kết nối SSH nền (chuẩn Zed Editor)
#[derive(Clone, Default)]
pub struct SshInteractiveSession {
    pub receiver: Arc<Mutex<Option<Receiver<SshInteractiveEvent>>>>,
    pub pending_response: Arc<Mutex<Option<Sender<String>>>>,
    pub cancel_flag: Arc<AtomicBool>,
}

/// Subview: Nhập kết nối SSH và xử lý luồng xác thực Host Key (yes/no) cùng Password theo chuẩn Zed Editor
pub fn render_ssh_picker_subview(
    ui: &mut egui::Ui,
    ssh_state: &mut SshPickerState,
    keymap: &KeymapManager,
    store: &mut WorkspaceStore,
) -> RemoteNavAction {
    let theme = ui.app_theme();

    // Tiêu thụ Semantic Actions thông qua KeymapManager
    let action = keymap.consume_input(ui, KeyContext::RemoteServers);
    let key_down = action == Some(KeyAction::SelectNext);
    let key_up = action == Some(KeyAction::SelectPrev);
    let mut key_enter = action == Some(KeyAction::ConfirmSelection);
    let key_escape = action == Some(KeyAction::Back);

    match &mut ssh_state.stage {
        // ====================================================================
        // GIAI ĐOẠN 1: Nhập địa chỉ SSH (`ssh user@example -o 2222`)
        // ====================================================================
        SshPickerStage::Input => {
            let mut nav_action = RemoteNavAction::None;

            // 1. Header
            ui.horizontal(|ui| {
                if AppButton::new()
                    .label("Back")
                    .icon(IconName::ArrowLeft)
                    .variant(crate::components::ui::ButtonVariant::Ghost)
                    .show(ui)
                    .clicked()
                {
                    nav_action = RemoteNavAction::Back;
                }

                ui.add_space(4.0);
                let (icon_r, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                IconName::Server.paint(ui.painter(), icon_r, theme.text.primary);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Connect SSH Server")
                        .strong()
                        .size(13.0)
                        .color(theme.text.primary),
                );
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            if key_escape {
                return RemoteNavAction::Back;
            }

            // Hiển thị thông báo lỗi kết nối nếu lần thử trước thất bại (chuẩn Zed Editor)
            if let Some(ref err) = ssh_state.error_message {
                egui::Frame::new()
                    .fill(theme.status.error.gamma_multiply(0.15))
                    .stroke(egui::Stroke::new(1.0, theme.status.error))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_r, _) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            IconName::AlertTriangle.paint(ui.painter(), icon_r, theme.status.error);
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!("Connection Failed: {}", err))
                                    .size(11.5)
                                    .color(theme.status.error),
                            );
                        });
                    });
                ui.add_space(6.0);
            }

            // 2. Ô nhập địa chỉ SSH với định dạng chuẩn: `ssh user@example -o 2222`
            let input_id = egui::Id::new("ssh_picker_host_input");

            if ssh_state.focus_input || key_down || key_up {
                ssh_state.focus_input = false;
                anchor_cursor_to_end(ui.ctx(), input_id, &ssh_state.input_query);
            }

            let prev_query = ssh_state.input_query.clone();
            let input_resp = TextInput::new(&mut ssh_state.input_query)
                .id(input_id)
                .auto_focus(true)
                .hint_text("ssh user@example -o 2222")
                .transparent()
                .show(ui);

            if !input_resp.has_focus() {
                input_resp.request_focus();
            }

            if ssh_state.input_query != prev_query {
                ssh_state.selected_index = 0;
                ssh_state.error_message = None;
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);

            // 3. Lọc danh sách ứng viên từ SSH config
            let query_trimmed = ssh_state.input_query.trim();
            let query_lower = query_trimmed.to_lowercase();

            let matched_hosts: Vec<&String> = ssh_state
                .suggested_hosts
                .iter()
                .filter(|h| query_lower.is_empty() || h.to_lowercase().contains(&query_lower))
                .collect();

            let has_custom_input = !query_trimmed.is_empty();
            let total_items = if has_custom_input {
                1 + matched_hosts.len()
            } else {
                matched_hosts.len()
            };

            step_selected_index(&mut ssh_state.selected_index, total_items, key_down, key_up);

            let mouse_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
            let mut chosen_target: Option<String> = None;

            if key_enter {
                if has_custom_input {
                    if ssh_state.selected_index == 0 {
                        chosen_target = Some(query_trimmed.to_string());
                    } else if let Some(h) = matched_hosts.get(ssh_state.selected_index - 1) {
                        chosen_target = Some((*h).clone());
                    }
                } else if let Some(h) = matched_hosts.get(ssh_state.selected_index) {
                    chosen_target = Some((*h).clone());
                }
            }

            // 4. Danh sách các gợi ý
            let scroll_height = calculate_adaptive_scroll_height(total_items, 29.0, 320.0);

            egui::ScrollArea::vertical()
                .id_salt("ssh_picker_hosts_scroll")
                .max_height(scroll_height)
                .show(ui, |ui| {
                    let mut current_ix = 0;

                    // Mục đầu tiên: Kết nối tới chuỗi người dùng tự gõ
                    if has_custom_input {
                        let is_sel = ssh_state.selected_index == current_ix;
                        let connect_label = format!("Connect to \"{}\"", query_trimmed);
                        let resp = ListItemRow::new(IconName::Return, &connect_label)
                            .selected(is_sel)
                            .tooltip(Some("Connect to this SSH destination"))
                            .show(ui);

                        if resp.hovered() && mouse_moved {
                            ssh_state.selected_index = current_ix;
                        }
                        if is_sel && (key_down || key_up) {
                            resp.scroll_to_me(Some(egui::Align::Center));
                        }
                        if resp.clicked() {
                            chosen_target = Some(query_trimmed.to_string());
                        }
                        current_ix += 1;
                        ui.add_space(1.0);
                    }

                    // Danh sách gợi ý từ ~/.ssh/config
                    for host in &matched_hosts {
                        let is_sel = ssh_state.selected_index == current_ix;
                        let resp = ListItemRow::new(IconName::Server, host)
                            .selected(is_sel)
                            .tooltip(Some("SSH Host from configuration"))
                            .show(ui);

                        if resp.hovered() && mouse_moved {
                            ssh_state.selected_index = current_ix;
                        }
                        if is_sel && (key_down || key_up) {
                            resp.scroll_to_me(Some(egui::Align::Center));
                        }
                        if resp.clicked() {
                            chosen_target = Some((*host).clone());
                        }
                        current_ix += 1;
                        ui.add_space(1.0);
                    }

                    if total_items == 0 {
                        ui.vertical_centered(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new("No SSH hosts found.")
                                    .size(12.0)
                                    .color(theme.text.muted),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "Type an address (e.g. ssh user@example -o 2222) to connect.",
                                )
                                .size(11.0)
                                .color(theme.text.muted),
                            );
                        });
                    }
                });

            // 5. Khi người dùng xác nhận kết nối -> Bắt đầu kết nối tương tác ngay lập tức (chuẩn Zed Editor)
            if let Some(target) = chosen_target {
                match SshConnectionOptions::parse_command_line(&target, "") {
                    Ok(opts) => {
                        ssh_state.error_message = None;
                        ssh_state.parsed_options = Some(opts.clone());

                        let (event_tx, event_rx) = std::sync::mpsc::channel();
                        let cancel_flag = Arc::new(AtomicBool::new(false));
                        let pending_response = Arc::new(Mutex::new(None));

                        let session = SshInteractiveSession {
                            receiver: Arc::new(Mutex::new(Some(event_rx))),
                            pending_response: pending_response.clone(),
                            cancel_flag: cancel_flag.clone(),
                        };
                        let session_id = egui::Id::new("ssh_interactive_session");
                        ui.ctx().data_mut(|d| d.insert_temp(session_id, session));

                        let host = opts.host.clone();
                        let user = opts.username.clone();
                        let port = opts.port;
                        let args = opts.args.clone();

                        std::thread::spawn(move || {
                            SshTransport::connect_interactive(
                                &host,
                                user.as_deref(),
                                port,
                                args.as_deref(),
                                cancel_flag,
                                event_tx,
                            );
                        });

                        ssh_state.stage = SshPickerStage::Connecting {
                            status_message: format!("Connecting to {}...", opts.target_string()),
                        };
                    }
                    Err(e) => {
                        ssh_state.error_message =
                            Some(format!("Could not parse SSH address: {}", e));
                    }
                }
            }

            nav_action
        }

        // ====================================================================
        // GIAI ĐOẠN 2: Host Key Verification (yes/no) theo chuẩn Zed
        // ====================================================================
        SshPickerStage::HostKeyVerification {
            prompt_message,
            user_input,
        } => {
            let target_title = ssh_state
                .parsed_options
                .as_ref()
                .map(|o| o.target_string())
                .unwrap_or_else(|| "SSH Server".to_string());

            let mut go_back = false;

            // Header
            ui.horizontal(|ui| {
                if AppButton::new()
                    .label("Back")
                    .icon(IconName::ArrowLeft)
                    .variant(crate::components::ui::ButtonVariant::Ghost)
                    .show(ui)
                    .clicked()
                {
                    go_back = true;
                }

                ui.add_space(4.0);
                let (icon_r, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                IconName::Server.paint(ui.painter(), icon_r, theme.text.primary);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("SSH: {}", target_title))
                        .strong()
                        .size(13.0)
                        .color(theme.text.primary),
                );
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            let session_id = egui::Id::new("ssh_interactive_session");
            let session_arc: Option<SshInteractiveSession> =
                ui.ctx().data(|d| d.get_temp(session_id));

            if go_back || key_escape {
                if let Some(ref sess) = session_arc {
                    sess.cancel_flag.store(true, Ordering::Relaxed);
                }
                ui.ctx().data_mut(|d| {
                    d.remove_temp::<SshInteractiveSession>(session_id);
                });
                ssh_state.stage = SshPickerStage::Input;
                ssh_state.focus_input = true;
                return RemoteNavAction::None;
            }

            // Hộp hiển thị message xác thực host key
            egui::Frame::new()
                .fill(theme.surfaces.mantle)
                .stroke(egui::Stroke::new(1.0, theme.borders.border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (icon_r, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        IconName::AlertTriangle.paint(ui.painter(), icon_r, theme.status.warning);
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Host Authenticity Verification")
                                .strong()
                                .size(12.0)
                                .color(theme.status.warning),
                        );
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(prompt_message.as_str())
                            .size(11.5)
                            .color(theme.text.primary)
                            .family(egui::FontFamily::Monospace),
                    );
                });

            ui.add_space(8.0);

            // Ô nhập yes/no
            let input_id = egui::Id::new("ssh_hostkey_yes_no_input");
            if ssh_state.focus_input {
                ssh_state.focus_input = false;
                anchor_cursor_to_end(ui.ctx(), input_id, user_input);
            }

            let mut advance_to_password = false;
            let mut cancel_back = false;

            ui.horizontal(|ui| {
                let input_resp = TextInput::new(user_input)
                    .id(input_id)
                    .auto_focus(true)
                    .hint_text("Type 'yes' or 'no'")
                    .width(ui.available_width() - 80.0)
                    .bordered()
                    .show(ui);

                if !input_resp.has_focus() {
                    input_resp.request_focus();
                }

                if AppButton::new()
                    .label("Confirm")
                    .variant(crate::components::ui::ButtonVariant::Primary)
                    .show(ui)
                    .clicked()
                {
                    key_enter = true;
                }
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Press Enter to submit response ('yes' to accept, 'no' to abort).",
                )
                .size(11.0)
                .color(theme.text.muted),
            );

            if key_enter {
                let answer = user_input.trim().to_lowercase();
                if answer == "yes" || answer == "y" {
                    advance_to_password = true;
                } else if answer == "no" || answer == "n" {
                    cancel_back = true;
                }
            }

            if advance_to_password {
                if let Some(ref sess) = session_arc {
                    if let Ok(mut lock) = sess.pending_response.lock() {
                        if let Some(tx) = lock.take() {
                            let _ = tx.send("yes".to_string());
                        }
                    }
                }
                ssh_state.stage = SshPickerStage::Connecting {
                    status_message: "Authenticating with host...".to_string(),
                };
            } else if cancel_back {
                if let Some(ref sess) = session_arc {
                    sess.cancel_flag.store(true, Ordering::Relaxed);
                    if let Ok(mut lock) = sess.pending_response.lock() {
                        if let Some(tx) = lock.take() {
                            let _ = tx.send("no".to_string());
                        }
                    }
                }
                ui.ctx().data_mut(|d| {
                    d.remove_temp::<SshInteractiveSession>(session_id);
                });
                ssh_state.stage = SshPickerStage::Input;
                ssh_state.focus_input = true;
            }

            RemoteNavAction::None
        }

        // ====================================================================
        // GIAI ĐOẠN 3: Password Prompt theo chuẩn Zed (có nút Toggle Mask/Unmask)
        // ====================================================================
        SshPickerStage::PasswordPrompt {
            prompt_message,
            password_input,
            is_masked,
            error_message,
        } => {
            let target_title = ssh_state
                .parsed_options
                .as_ref()
                .map(|o| o.target_string())
                .unwrap_or_else(|| "SSH Server".to_string());

            let mut go_back = false;

            // Header
            ui.horizontal(|ui| {
                if AppButton::new()
                    .label("Back")
                    .icon(IconName::ArrowLeft)
                    .variant(crate::components::ui::ButtonVariant::Ghost)
                    .show(ui)
                    .clicked()
                {
                    go_back = true;
                }

                ui.add_space(4.0);
                let (icon_r, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                IconName::Server.paint(ui.painter(), icon_r, theme.text.primary);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("SSH: {}", target_title))
                        .strong()
                        .size(13.0)
                        .color(theme.text.primary),
                );
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            let session_id = egui::Id::new("ssh_interactive_session");
            let session_arc: Option<SshInteractiveSession> =
                ui.ctx().data(|d| d.get_temp(session_id));

            if go_back || key_escape {
                if let Some(ref sess) = session_arc {
                    sess.cancel_flag.store(true, Ordering::Relaxed);
                }
                ui.ctx().data_mut(|d| {
                    d.remove_temp::<SshInteractiveSession>(session_id);
                });
                ssh_state.stage = SshPickerStage::Input;
                ssh_state.focus_input = true;
                return RemoteNavAction::None;
            }

            // Hiển thị thông báo lỗi nếu lần thử trước thất bại
            if let Some(ref err) = error_message {
                egui::Frame::new()
                    .fill(theme.status.error.gamma_multiply(0.15))
                    .stroke(egui::Stroke::new(1.0, theme.status.error))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_r, _) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            IconName::AlertTriangle.paint(ui.painter(), icon_r, theme.status.error);
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!("Connection Failed: {}", err))
                                    .size(11.5)
                                    .color(theme.status.error),
                            );
                        });
                    });
                ui.add_space(6.0);
            }

            // Hàng nhãn mật khẩu + nút Toggle Show/Hide Password (mô phỏng Zed Eye/EyeOff)
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(prompt_message.as_str())
                        .strong()
                        .size(12.5)
                        .color(theme.text.primary),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle_label = if *is_masked { "Show" } else { "Hide" };
                    let toggle_tooltip = if *is_masked {
                        "Toggle to Unmask Password"
                    } else {
                        "Toggle to Mask Password"
                    };

                    if AppButton::new()
                        .label(toggle_label)
                        .variant(crate::components::ui::ButtonVariant::Ghost)
                        .show(ui)
                        .on_hover_text(toggle_tooltip)
                        .clicked()
                    {
                        *is_masked = !*is_masked;
                    }
                });
            });

            ui.add_space(6.0);

            // Ô nhập mật khẩu
            let input_id = egui::Id::new("ssh_password_input");
            if ssh_state.focus_input {
                ssh_state.focus_input = false;
                anchor_cursor_to_end(ui.ctx(), input_id, password_input);
            }

            let mut submit_password = false;

            ui.horizontal(|ui| {
                let input_resp = TextInput::new(password_input)
                    .id(input_id)
                    .password(*is_masked)
                    .auto_focus(true)
                    .hint_text("Enter password")
                    .width(ui.available_width() - 80.0)
                    .bordered()
                    .show(ui);

                if !input_resp.has_focus() {
                    input_resp.request_focus();
                }

                if AppButton::new()
                    .label("Connect")
                    .variant(crate::components::ui::ButtonVariant::Primary)
                    .show(ui)
                    .clicked()
                {
                    submit_password = true;
                }
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Press Enter to connect, Escape to cancel.")
                    .size(11.0)
                    .color(theme.text.muted),
            );

            if key_enter {
                submit_password = true;
            }

            if submit_password {
                if let Some(ref sess) = session_arc {
                    if let Ok(mut lock) = sess.pending_response.lock() {
                        if let Some(tx) = lock.take() {
                            let _ = tx.send(password_input.clone());
                        }
                    }
                }
                ssh_state.stage = SshPickerStage::Connecting {
                    status_message: format!("Authenticating with {}", target_title),
                };
                return RemoteNavAction::None;
            }

            RemoteNavAction::None
        }

        // ====================================================================
        // GIAI ĐOẠN 4: Đang kết nối nền (Connecting non-blocking)
        // ====================================================================
        SshPickerStage::Connecting { status_message } => {
            let session_id = egui::Id::new("ssh_interactive_session");
            let session_arc: Option<SshInteractiveSession> =
                ui.ctx().data(|d| d.get_temp(session_id));

            let mut pending_event = None;
            if let Some(ref sess) = session_arc {
                if let Ok(lock) = sess.receiver.lock() {
                    if let Some(ref rx) = *lock {
                        match rx.try_recv() {
                            Ok(event) => pending_event = Some(event),
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                ui.ctx()
                                    .request_repaint_after(std::time::Duration::from_millis(50));
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                pending_event = Some(SshInteractiveEvent::Failed(
                                    "SSH connection worker thread terminated unexpectedly"
                                        .to_string(),
                                ));
                            }
                        }
                    }
                }
            }

            if let Some(event) = pending_event {
                match event {
                    SshInteractiveEvent::Status(msg) => {
                        *status_message = msg;
                    }
                    SshInteractiveEvent::Prompt {
                        prompt_message,
                        prompt_type,
                        response_sender,
                    } => {
                        if let Some(ref sess) = session_arc {
                            if let Ok(mut lock) = sess.pending_response.lock() {
                                *lock = Some(response_sender);
                            }
                        }
                        match prompt_type {
                            SshInteractivePromptType::HostKeyConfirmation => {
                                ssh_state.stage = SshPickerStage::HostKeyVerification {
                                    prompt_message,
                                    user_input: String::new(),
                                };
                                ssh_state.focus_input = true;
                                return RemoteNavAction::None;
                            }
                            SshInteractivePromptType::Password => {
                                ssh_state.stage = SshPickerStage::PasswordPrompt {
                                    prompt_message,
                                    password_input: String::new(),
                                    is_masked: true,
                                    error_message: None,
                                };
                                ssh_state.focus_input = true;
                                return RemoteNavAction::None;
                            }
                        }
                    }
                    SshInteractiveEvent::Connected(success) => {
                        ui.ctx().data_mut(|d| {
                            d.remove_temp::<SshInteractiveSession>(session_id);
                        });

                        if let Some(ref opts) = ssh_state.parsed_options {
                            let conn = store.ensure_ssh_connection(&opts.host);
                            if let Some(ref u) = opts.username {
                                conn.username = Some(u.clone());
                            }
                            if let Some(p) = opts.port {
                                conn.port = Some(p);
                            }
                            if let Some(ref args) = opts.args {
                                conn.args = Some(args.clone());
                            }
                            let _ = store.save();

                            let server_kind = RemoteServerKind::Ssh {
                                host: opts.host.clone(),
                                nickname: opts.nickname.clone(),
                            };

                            let folder_state = FolderPickerState::new(
                                server_kind,
                                success.initial_dir,
                                success.entries,
                            );
                            return RemoteNavAction::navigate(RemoteSubView::FolderPicker(
                                folder_state,
                            ));
                        }
                    }
                    SshInteractiveEvent::Failed(err) => {
                        ui.ctx().data_mut(|d| {
                            d.remove_temp::<SshInteractiveSession>(session_id);
                        });

                        ssh_state.stage = SshPickerStage::Input;
                        ssh_state.error_message = Some(err);
                        ssh_state.focus_input = true;
                        return RemoteNavAction::None;
                    }
                }
            }

            let target_title = ssh_state
                .parsed_options
                .as_ref()
                .map(|o| o.target_string())
                .unwrap_or_else(|| "SSH Server".to_string());

            let mut abort_connect = false;
            if key_escape {
                abort_connect = true;
            }

            ui.horizontal(|ui| {
                if AppButton::new()
                    .label("Cancel")
                    .icon(IconName::Close)
                    .variant(crate::components::ui::ButtonVariant::Ghost)
                    .show(ui)
                    .clicked()
                {
                    abort_connect = true;
                }

                ui.add_space(4.0);
                let (icon_r, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                IconName::Server.paint(ui.painter(), icon_r, theme.text.primary);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("SSH: {}", target_title))
                        .strong()
                        .size(13.0)
                        .color(theme.text.primary),
                );
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(16.0);

            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(format!("{}…", status_message))
                        .strong()
                        .size(13.0)
                        .color(theme.text.primary),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Establishing secure SSH connection and multiplexing channel.",
                    )
                    .size(11.0)
                    .color(theme.text.muted),
                );
                ui.add_space(14.0);
                if AppButton::new()
                    .label("Cancel Connection")
                    .variant(crate::components::ui::ButtonVariant::Default)
                    .show(ui)
                    .clicked()
                {
                    abort_connect = true;
                }
            });

            if abort_connect {
                if let Some(ref sess) = session_arc {
                    sess.cancel_flag.store(true, Ordering::Relaxed);
                }
                ui.ctx().data_mut(|d| {
                    d.remove_temp::<SshInteractiveSession>(session_id);
                });
                ssh_state.stage = SshPickerStage::Input;
                ssh_state.focus_input = true;
            }

            RemoteNavAction::None
        }
    }
}
