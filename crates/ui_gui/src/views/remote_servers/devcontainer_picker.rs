use crate::components::ui::{AppButton, ButtonVariant, IconName};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use crate::theme::ActiveTheme;
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use uwu_core_workspace::{DockerConnection, ServerConnection, Workspace, WorkspaceStore};
use uwu_driver_transport::{DockerContainerInfo, DockerTransport};

use super::helpers::create_server_workspace;
use super::types::{DevContainerPickerStage, DevContainerPickerState, RemoteNavAction};

struct ContainerOpenSuccess {
    connection: DockerConnection,
    workspace_folder: String,
}

/// Phiên kết nối Dev Container bất đồng bộ, được sở hữu bởi `OverlayManager`.
pub struct DevContainerConnectSession {
    receiver: Receiver<Result<ContainerOpenSuccess, String>>,
}

fn start_open_devcontainer_folder(
    ctx: &egui::Context,
    rt: &tokio::runtime::Handle,
    local_folder: PathBuf,
) -> DevContainerConnectSession {
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx_clone = ctx.clone();

    rt.spawn(async move {
        let result: anyhow::Result<ContainerOpenSuccess> = async move {
            let opened = DockerTransport::open_devcontainer_folder(&local_folder).await?;
            let mut conn = build_connection_from_info(&opened.info);
            if let Some(ref user) = opened.remote_user {
                conn = conn.with_remote_user(user);
            }

            Ok(ContainerOpenSuccess {
                connection: conn,
                workspace_folder: opened.workspace_folder,
            })
        }
        .await;

        let _ = tx.send(result.map_err(|e| format!("{:#}", e)));
        ctx_clone.request_repaint();
    });

    DevContainerConnectSession { receiver: rx }
}

fn build_connection_from_info(info: &DockerContainerInfo) -> DockerConnection {
    let mut conn = DockerConnection::new(&info.name)
        .with_container_id(&info.id)
        .with_image(&info.image)
        .with_devcontainer(info.is_devcontainer);
    if let Some(ref folder) = info.local_folder {
        conn = conn.with_local_folder(folder);
    }
    if let Some(ref project) = info.compose_project {
        conn = conn.with_compose_project(project);
    }
    if let Some(ref service) = info.compose_service {
        conn = conn.with_compose_service(service);
    }
    conn
}

/// Subview: Kết nối thẳng tới Dev Container của dự án local hiện tại (hoặc báo lỗi nếu dự án không có Dev Container)
pub fn render_devcontainer_picker_subview(
    ui: &mut egui::Ui,
    dc_state: &mut DevContainerPickerState,
    probe_session: &mut Option<DevContainerConnectSession>,
    rt: &tokio::runtime::Handle,
    keymap: &KeymapManager,
    store: &mut WorkspaceStore,
) -> (RemoteNavAction, Option<Workspace>) {
    let theme = ui.app_theme();

    let action = keymap.consume_input(ui, KeyContext::RemoteServers);
    let key_escape = action == Some(KeyAction::Back);

    let mut nav_action = RemoteNavAction::None;
    ui.horizontal(|ui| {
        if AppButton::new()
            .label("Back")
            .icon(IconName::ArrowLeft)
            .variant(ButtonVariant::Ghost)
            .show(ui)
            .clicked()
        {
            nav_action = RemoteNavAction::Back;
        }

        ui.add_space(4.0);
        let (icon_r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        IconName::Box.paint(ui.painter(), icon_r, theme.text.primary);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Connect Dev Container")
                .strong()
                .size(13.0)
                .color(theme.text.primary),
        );
    });

    if key_escape || !matches!(nav_action, RemoteNavAction::None) {
        *probe_session = None;
        return (RemoteNavAction::Back, None);
    }

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    match &dc_state.stage {
        DevContainerPickerStage::Connecting { local_folder } => {
            let folder_path = PathBuf::from(local_folder);
            let folder_name = folder_path
                .file_name()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(local_folder.as_str())
                .to_string();

            if probe_session.is_none() {
                *probe_session = Some(start_open_devcontainer_folder(ui.ctx(), rt, folder_path));
            }

            let probe_result = probe_session
                .as_ref()
                .and_then(|session| session.receiver.try_recv().ok());

            if let Some(res) = probe_result {
                *probe_session = None;
                match res {
                    Ok(success) => {
                        store.upsert_docker_connection(success.connection.clone());
                        let server = ServerConnection::Docker(success.connection.clone());
                        let ws = create_server_workspace(&server, &success.workspace_folder);
                        store.add_remote_project(&server, &success.workspace_folder);
                        store.add_or_update(ws.clone());
                        return (RemoteNavAction::None, Some(ws));
                    }
                    Err(err) => {
                        dc_state.stage = DevContainerPickerStage::Error(err);
                        return (RemoteNavAction::None, None);
                    }
                }
            }

            ui.vertical_centered(|ui| {
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Connecting to Dev Container for `{}`...",
                        folder_name
                    ))
                    .size(12.0)
                    .color(theme.text.muted),
                );
                ui.add_space(8.0);
            });
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
            (RemoteNavAction::None, None)
        }
        DevContainerPickerStage::Error(err_msg) => {
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
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(err_msg.as_str())
                                    .size(11.5)
                                    .color(theme.status.error),
                            )
                            .wrap(),
                        );
                    });
                });
            ui.add_space(8.0);
            (RemoteNavAction::None, None)
        }
    }
}
