use crate::components::ui::{AppButton, IconName, TextInput};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use crate::theme::ActiveTheme;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use uwu_core_workspace::{ServerConnection, SshConnectionOptions, WorkspaceStore};
use uwu_driver_transport::{SshInteractiveEvent, SshInteractivePromptType, SshTransport};

use super::helpers::{
    anchor_cursor_to_end, calculate_adaptive_scroll_height, step_selected_index, ListItemRow,
};
use super::types::{
    FolderPickerState, RemoteNavAction, RemoteSubView, SshPickerStage, SshPickerState,
};

/// Trạng thái phiên tương tác kết nối SSH nền (chuẩn Zed Editor), được sở hữu bởi `OverlayManager`.
pub struct SshInteractiveSession {
    pub receiver: Receiver<SshInteractiveEvent>,
    pub pending_response: Option<Sender<String>>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl SshInteractiveSession {
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }
}

fn render_ssh_stage_header(
    ui: &mut egui::Ui,
    button_label: &str,
    button_icon: IconName,
    title: &str,
    bottom_space: f32,
) -> bool {
    let theme = ui.app_theme();
    let mut clicked = false;

    ui.horizontal(|ui| {
        if AppButton::new()
            .label(button_label)
            .icon(button_icon)
            .variant(crate::components::ui::ButtonVariant::Ghost)
            .show(ui)
            .clicked()
        {
            clicked = true;
        }

        ui.add_space(4.0);
        let (icon_r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        IconName::Server.paint(ui.painter(), icon_r, theme.text.primary);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(title)
                .strong()
                .size(13.0)
                .color(theme.text.primary),
        );
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(bottom_space);

    clicked
}

fn render_ssh_error_banner(ui: &mut egui::Ui, err: &str) {
    let theme = ui.app_theme();
    egui::Frame::new()
        .fill(theme.status.error.gamma_multiply(0.15))
        .stroke(egui::Stroke::new(1.0, theme.status.error))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (icon_r, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
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

#[allow(clippy::too_many_arguments)]
fn render_input_stage(
    ui: &mut egui::Ui,
    ssh_state: &mut SshPickerState,
    ssh_session: &mut Option<SshInteractiveSession>,
    rt: &tokio::runtime::Handle,
    key_down: bool,
    key_up: bool,
    key_enter: bool,
    key_escape: bool,
) -> RemoteNavAction {
    let theme = ui.app_theme();
    let mut nav_action = RemoteNavAction::None;

    if render_ssh_stage_header(ui, "Back", IconName::ArrowLeft, "Connect SSH Server", 4.0) {
        nav_action = RemoteNavAction::Back;
    }

    if key_escape {
        return RemoteNavAction::Back;
    }

    if let Some(ref err) = ssh_state.error_message {
        render_ssh_error_banner(ui, err);
    }

    // Ô nhập địa chỉ SSH với định dạng chuẩn: `ssh user@example -o 2222`
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

    // Lọc danh sách ứng viên từ SSH config
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

    // Danh sách các gợi ý
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

    // Khi người dùng xác nhận kết nối -> Bắt đầu kết nối tương tác ngay lập tức (chuẩn Zed Editor)
    if let Some(target) = chosen_target {
        match SshConnectionOptions::parse_command_line(&target, "") {
            Ok(opts) => {
                ssh_state.error_message = None;
                ssh_state.parsed_options = Some(opts.clone());

                let (event_tx, event_rx) = std::sync::mpsc::channel();
                let cancel_flag = Arc::new(AtomicBool::new(false));

                if let Some(prev) = ssh_session.take() {
                    prev.cancel();
                }
                *ssh_session = Some(SshInteractiveSession {
                    receiver: event_rx,
                    pending_response: None,
                    cancel_flag: cancel_flag.clone(),
                });

                let host = opts.host.clone();
                let user = opts.username.clone();
                let port = opts.port;
                let args = opts.args.clone();

                rt.spawn_blocking(move || {
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
                ssh_state.error_message = Some(format!("Could not parse SSH address: {}", e));
            }
        }
    }

    nav_action
}

#[allow(clippy::too_many_arguments)]
fn render_host_key_stage(
    ui: &mut egui::Ui,
    target_title: &str,
    prompt_message: &str,
    user_input: &mut String,
    focus_input: &mut bool,
    ssh_session: &mut Option<SshInteractiveSession>,
    mut key_enter: bool,
    key_escape: bool,
) -> Option<SshPickerStage> {
    let theme = ui.app_theme();
    let header_title = format!("SSH: {}", target_title);
    let go_back = render_ssh_stage_header(ui, "Back", IconName::ArrowLeft, &header_title, 6.0);

    if go_back || key_escape {
        if let Some(sess) = ssh_session.take() {
            sess.cancel();
        }
        *focus_input = true;
        return Some(SshPickerStage::Input);
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
                egui::RichText::new(prompt_message)
                    .size(11.5)
                    .color(theme.text.primary)
                    .family(egui::FontFamily::Monospace),
            );
        });

    ui.add_space(8.0);

    // Ô nhập yes/no
    let input_id = egui::Id::new("ssh_hostkey_yes_no_input");
    if *focus_input {
        *focus_input = false;
        anchor_cursor_to_end(ui.ctx(), input_id, user_input);
    }

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
        egui::RichText::new("Press Enter to submit response ('yes' to accept, 'no' to abort).")
            .size(11.0)
            .color(theme.text.muted),
    );

    if key_enter {
        let answer = user_input.trim().to_lowercase();
        if answer == "yes" || answer == "y" {
            if let Some(sess) = ssh_session.as_mut() {
                if let Some(tx) = sess.pending_response.take() {
                    let _ = tx.send("yes".to_string());
                }
            }
            return Some(SshPickerStage::Connecting {
                status_message: "Authenticating with host...".to_string(),
            });
        } else if answer == "no" || answer == "n" {
            if let Some(mut sess) = ssh_session.take() {
                sess.cancel();
                if let Some(tx) = sess.pending_response.take() {
                    let _ = tx.send("no".to_string());
                }
            }
            *focus_input = true;
            return Some(SshPickerStage::Input);
        }
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn render_password_stage(
    ui: &mut egui::Ui,
    target_title: &str,
    prompt_message: &str,
    password_input: &mut String,
    is_masked: &mut bool,
    error_message: Option<&str>,
    focus_input: &mut bool,
    ssh_session: &mut Option<SshInteractiveSession>,
    key_enter: bool,
    key_escape: bool,
) -> Option<SshPickerStage> {
    let theme = ui.app_theme();
    let header_title = format!("SSH: {}", target_title);
    let go_back = render_ssh_stage_header(ui, "Back", IconName::ArrowLeft, &header_title, 6.0);

    if go_back || key_escape {
        if let Some(sess) = ssh_session.take() {
            sess.cancel();
        }
        *focus_input = true;
        return Some(SshPickerStage::Input);
    }

    if let Some(err) = error_message {
        render_ssh_error_banner(ui, err);
    }

    // Hàng nhãn mật khẩu + nút Toggle Show/Hide Password (mô phỏng Zed Eye/EyeOff)
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(prompt_message)
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

    let input_id = egui::Id::new("ssh_password_input");
    if *focus_input {
        *focus_input = false;
        anchor_cursor_to_end(ui.ctx(), input_id, password_input);
    }

    let mut submit_password = key_enter;

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

    if submit_password {
        if let Some(sess) = ssh_session.as_mut() {
            if let Some(tx) = sess.pending_response.take() {
                let _ = tx.send(password_input.clone());
            }
        }
        return Some(SshPickerStage::Connecting {
            status_message: format!("Authenticating with {}", target_title),
        });
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn render_connecting_stage(
    ui: &mut egui::Ui,
    target_title: &str,
    status_message: &mut String,
    parsed_options: Option<&SshConnectionOptions>,
    focus_input: &mut bool,
    ssh_error: &mut Option<String>,
    ssh_session: &mut Option<SshInteractiveSession>,
    store: &mut WorkspaceStore,
    key_escape: bool,
) -> (RemoteNavAction, Option<SshPickerStage>) {
    let theme = ui.app_theme();
    let mut pending_event = None;
    if let Some(sess) = ssh_session.as_ref() {
        match sess.receiver.try_recv() {
            Ok(event) => pending_event = Some(event),
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                pending_event = Some(SshInteractiveEvent::Failed(
                    "SSH connection worker thread terminated unexpectedly".to_string(),
                ));
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
                if let Some(sess) = ssh_session.as_mut() {
                    sess.pending_response = Some(response_sender);
                }
                *focus_input = true;
                let next = match prompt_type {
                    SshInteractivePromptType::HostKeyConfirmation => {
                        SshPickerStage::HostKeyVerification {
                            prompt_message,
                            user_input: String::new(),
                        }
                    }
                    SshInteractivePromptType::Password => SshPickerStage::PasswordPrompt {
                        prompt_message,
                        password_input: String::new(),
                        is_masked: true,
                        error_message: None,
                    },
                };
                return (RemoteNavAction::None, Some(next));
            }
            SshInteractiveEvent::Connected(success) => {
                *ssh_session = None;

                if let Some(opts) = parsed_options {
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

                    let server = ServerConnection::ssh_from_options(opts);
                    let folder_state =
                        FolderPickerState::new(server, success.initial_dir, success.entries);
                    return (
                        RemoteNavAction::navigate(RemoteSubView::FolderPicker(folder_state)),
                        None,
                    );
                }
            }
            SshInteractiveEvent::Failed(err) => {
                *ssh_session = None;
                *ssh_error = Some(err);
                *focus_input = true;
                return (RemoteNavAction::None, Some(SshPickerStage::Input));
            }
        }
    }

    let header_title = format!("SSH: {}", target_title);
    let mut abort_connect =
        key_escape || render_ssh_stage_header(ui, "Cancel", IconName::Close, &header_title, 16.0);

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
            egui::RichText::new("Establishing secure SSH connection and multiplexing channel.")
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
        if let Some(sess) = ssh_session.take() {
            sess.cancel();
        }
        *focus_input = true;
        return (RemoteNavAction::None, Some(SshPickerStage::Input));
    }

    (RemoteNavAction::None, None)
}

/// Subview: Nhập kết nối SSH và xử lý luồng xác thực Host Key (yes/no) cùng Password theo chuẩn Zed Editor
pub fn render_ssh_picker_subview(
    ui: &mut egui::Ui,
    ssh_state: &mut SshPickerState,
    ssh_session: &mut Option<SshInteractiveSession>,
    rt: &tokio::runtime::Handle,
    keymap: &KeymapManager,
    store: &mut WorkspaceStore,
) -> RemoteNavAction {
    // Tiêu thụ Semantic Actions thông qua KeymapManager
    let action = keymap.consume_input(ui, KeyContext::RemoteServers);
    let key_down = action == Some(KeyAction::SelectNext);
    let key_up = action == Some(KeyAction::SelectPrev);
    let key_enter = action == Some(KeyAction::ConfirmSelection);
    let key_escape = action == Some(KeyAction::Back);

    let target_title = ssh_state
        .parsed_options
        .as_ref()
        .map(|o| o.target_string())
        .unwrap_or_else(|| "SSH Server".to_string());
    let parsed_options = ssh_state.parsed_options.clone();
    let focus_input = &mut ssh_state.focus_input;
    let ssh_error = &mut ssh_state.error_message;

    let (nav_action, next_stage) = match &mut ssh_state.stage {
        SshPickerStage::Input => {
            return render_input_stage(
                ui,
                ssh_state,
                ssh_session,
                rt,
                key_down,
                key_up,
                key_enter,
                key_escape,
            );
        }
        SshPickerStage::HostKeyVerification {
            prompt_message,
            user_input,
        } => (
            RemoteNavAction::None,
            render_host_key_stage(
                ui,
                &target_title,
                prompt_message,
                user_input,
                focus_input,
                ssh_session,
                key_enter,
                key_escape,
            ),
        ),
        SshPickerStage::PasswordPrompt {
            prompt_message,
            password_input,
            is_masked,
            error_message,
        } => (
            RemoteNavAction::None,
            render_password_stage(
                ui,
                &target_title,
                prompt_message,
                password_input,
                is_masked,
                error_message.as_deref(),
                focus_input,
                ssh_session,
                key_enter,
                key_escape,
            ),
        ),
        SshPickerStage::Connecting { status_message } => render_connecting_stage(
            ui,
            &target_title,
            status_message,
            parsed_options.as_ref(),
            focus_input,
            ssh_error,
            ssh_session,
            store,
            key_escape,
        ),
    };

    if let Some(stage) = next_stage {
        ssh_state.stage = stage;
    }

    nav_action
}
