use crate::actions::AppAction;
use crate::components::ui::{AppButton, ButtonVariant, IconName};
use crate::overlay::{OverlayLayer, OverlayStack};
use crate::session::GuiSession;
use crate::theme::ActiveTheme;
use crate::views::modals::{render_project_row, ProjectRowAction, ProjectRowConfig};
use crate::views::toasts::{ToastAction, ToastDismiss, ToastItem, ToastTone};
use eframe::egui::{self, CornerRadius, Order, Rect, Stroke};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use uwu_core_workspace::{
    DockerConnectionOptions, RemoteConnectionOptions, SourceType, Workspace, WorkspaceLocation,
};
use uwu_driver_transport::{
    find_devcontainer_config, DockerContainerInfo, DockerTransport, OpenedDevContainer,
};

#[derive(Debug, Clone)]
pub struct ActiveContainerSessionInfo {
    pub session_index: usize,
    pub container_name: String,
    pub short_label: String,
    pub source_summary: String,
    pub is_active: bool,
}

struct LocalProbeEntry {
    checked_at: Instant,
    in_flight: bool,
    result: Option<OpenedDevContainer>,
    rx: Option<Receiver<Option<OpenedDevContainer>>>,
}

/// Cache thăm dò nền xem thư mục local có Dev Container đang chạy hay không (sở hữu bởi `OverlayManager`).
#[derive(Default)]
pub struct LocalDevContainerProbeCache {
    entries: HashMap<String, LocalProbeEntry>,
    dismissed_paths: HashSet<String>,
}

impl LocalDevContainerProbeCache {
    /// Đánh dấu đã ẩn toast gợi ý Dev Container cho đường dẫn local tương ứng.
    pub fn dismiss(&mut self, dir_key: &str) {
        self.dismissed_paths.insert(dir_key.to_string());
    }
}

/// Thăm dò nền và trả về toast gợi ý "Open in Dev Container" (nếu dự án local hiện tại có
/// file `.devcontainer` VÀ Dev Container của nó đang chạy).
pub fn poll_devcontainer_suggest_toast(
    ctx: &egui::Context,
    session: &GuiSession,
    cache: &mut LocalDevContainerProbeCache,
    rt: &tokio::runtime::Handle,
) -> Option<ToastItem> {
    let WorkspaceLocation::Local { ref working_dir } = session.session.location else {
        return None;
    };
    let dir_trimmed = working_dir.trim();
    if dir_trimmed.is_empty() {
        return None;
    }

    // Kiểm tra nhanh syscall stat: chỉ probe Docker khi thư mục thực sự có `.devcontainer`
    let local_path = Path::new(dir_trimmed);
    find_devcontainer_config(local_path)?;

    let key = dir_trimmed.to_string();
    if cache.dismissed_paths.contains(&key) {
        return None;
    }

    let now = Instant::now();
    let needs_probe = match cache.entries.get(&key) {
        None => true,
        Some(entry) => {
            !entry.in_flight && now.duration_since(entry.checked_at) > Duration::from_secs(4)
        }
    };

    if let Some(entry) = cache.entries.get_mut(&key) {
        if let Some(res) = entry.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            entry.result = res;
            entry.in_flight = false;
            entry.checked_at = now;
            entry.rx = None;
        }
    }

    if needs_probe {
        let (tx, rx) = std::sync::mpsc::channel();
        let prev_result = cache.entries.get(&key).and_then(|e| e.result.clone());
        cache.entries.insert(
            key.clone(),
            LocalProbeEntry {
                checked_at: now,
                in_flight: true,
                result: prev_result,
                rx: Some(rx),
            },
        );

        let path_owned = local_path.to_path_buf();
        let ctx_clone = ctx.clone();
        rt.spawn(async move {
            let res =
                DockerTransport::detect_running_devcontainer_for_local_folder(&path_owned).await;
            let _ = tx.send(res);
            ctx_clone.request_repaint();
        });
    }

    let opened = cache.entries.get(&key).and_then(|e| e.result.clone())?;

    let mut opts = DockerConnectionOptions::new(&opened.info.name, &opened.workspace_folder)
        .with_container_id(&opened.info.id)
        .with_devcontainer(true);
    if let Some(ref user) = opened.remote_user {
        opts = opts.with_remote_user(user);
    }
    if let Some(ref folder) = opened.info.local_folder {
        opts = opts.with_local_folder(folder);
    }
    if let Some(ref project) = opened.info.compose_project {
        opts = opts.with_compose_project(project);
    }
    if let Some(ref service) = opened.info.compose_service {
        opts = opts.with_compose_service(service);
    }

    let ws_name = uwu_core_workspace::extract_project_name(&opened.workspace_folder);
    let ws = Workspace::new(
        ws_name,
        WorkspaceLocation::remote(opts),
        SourceType::Process,
    );

    Some(ToastItem {
        icon: IconName::Box,
        tone: ToastTone::Success,
        title: "Container running".to_string(),
        subtitle: opened.info.short_label().to_string(),
        action: Some(ToastAction {
            label: "Open in Dev Container".to_string(),
            variant: ButtonVariant::Success,
            action: AppAction::OpenWorkspace(Box::new(ws)),
        }),
        dismiss: ToastDismiss::DevContainerSuggest(key),
    })
}

/// Hiển thị nút container (đặt sau tên dự án trên Header) cho biết đang xem container nào
/// và mở Quick Source Switcher khi click (không hiển thị thanh "Ready (Idle)" dư thừa).
pub fn render_docker_source_switcher_pill(
    ui: &mut egui::Ui,
    session: &GuiSession,
    overlay_stack: &OverlayStack,
    dispatch: &mut impl FnMut(AppAction),
) -> Option<Rect> {
    let WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(ref docker_opts)) =
        session.session.location
    else {
        return None;
    };
    let label = session.session.active_docker_short_label()?;

    let theme = ui.app_theme();

    let is_open = overlay_stack.is_open(OverlayLayer::SourceSwitcher);
    let source_summary = session.session.source_badge_label();
    let tooltip = format!(
        "Container: {}\nSource: {}\nClick to switch log source or container",
        docker_opts.container_name, source_summary
    );

    let resp = AppButton::new()
        .icon(IconName::Docker)
        .label(&label)
        .variant(if is_open {
            ButtonVariant::Selected
        } else {
            ButtonVariant::Ghost
        })
        .text_color(if is_open {
            theme.text.accent
        } else {
            theme.text.primary
        })
        .tooltip(&tooltip)
        .show(ui);

    if resp.clicked() {
        dispatch(AppAction::ToggleSourceSwitcher);
    }

    Some(resp.rect)
}

/// Trạng thái quét container cho popup Source Switcher (sở hữu bởi `OverlayManager`).
#[derive(Default)]
pub struct SwitcherContainersState {
    in_flight: bool,
    loaded: Option<Vec<DockerContainerInfo>>,
    error: Option<String>,
    rx: Option<Receiver<Result<Vec<DockerContainerInfo>, String>>>,
}

impl SwitcherContainersState {
    pub fn reset(&mut self) {
        self.in_flight = false;
        self.loaded = None;
        self.error = None;
        self.rx = None;
    }
}

/// Lọc các container đang chạy thuộc cùng nhóm dự án/compose với session Docker hiện tại (chống trùng lặp và không lẫn dự án khác)
fn filter_project_candidate_containers<'a>(
    docker_opts: &DockerConnectionOptions,
    active_containers: &[ActiveContainerSessionInfo],
    containers: &'a [DockerContainerInfo],
) -> Vec<&'a DockerContainerInfo> {
    let mut seen_names: HashSet<String> = active_containers
        .iter()
        .map(|ac| ac.container_name.to_ascii_lowercase())
        .collect();

    let project_compose = docker_opts
        .compose_project
        .as_deref()
        .filter(|s| !s.is_empty());
    let project_folder = docker_opts
        .local_folder
        .as_deref()
        .filter(|s| !s.is_empty());

    let mut result = Vec::new();
    for c in containers {
        let lower_name = c.name.to_ascii_lowercase();
        if !seen_names.insert(lower_name) {
            continue;
        }

        let candidate_compose = c.compose_project.as_deref().filter(|s| !s.is_empty());
        let matches_project = if let Some(proj) = project_compose {
            candidate_compose == Some(proj)
        } else if let Some(folder) = project_folder {
            candidate_compose.is_none()
                && c.local_folder
                    .as_deref()
                    .is_some_and(|cf| cf.eq_ignore_ascii_case(folder))
        } else {
            false
        };

        if matches_project {
            result.push(c);
        }
    }
    result
}

/// Render popup chuyển đổi container theo cơ chế `This Window`:
/// mỗi container mở là một session riêng biệt chạy song song, switch qua lại không mất log.
#[allow(clippy::too_many_arguments)]
pub fn render_source_switcher_popup(
    ctx: &egui::Context,
    session: &GuiSession,
    active_containers: &[ActiveContainerSessionInfo],
    overlay_stack: &OverlayStack,
    scan_state: &mut SwitcherContainersState,
    rt: &tokio::runtime::Handle,
    dispatch: &mut impl FnMut(AppAction),
    trigger_rect: Rect,
) {
    if !overlay_stack.is_open(OverlayLayer::SourceSwitcher) {
        scan_state.reset();
        return;
    }

    let WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(ref docker_opts)) =
        session.session.location
    else {
        return;
    };

    if let Some(res) = scan_state.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
        scan_state.in_flight = false;
        scan_state.rx = None;
        match res {
            Ok(list) => {
                scan_state.loaded = Some(list);
                scan_state.error = None;
            }
            Err(err) => {
                scan_state.error = Some(err);
            }
        }
    }

    if scan_state.loaded.is_none() && scan_state.error.is_none() && !scan_state.in_flight {
        let (tx, rx) = std::sync::mpsc::channel();
        scan_state.in_flight = true;
        scan_state.rx = Some(rx);
        let ctx_clone = ctx.clone();
        rt.spawn_blocking(move || {
            let res =
                DockerTransport::list_running_containers_sync().map_err(|e| format!("{:#}", e));
            let _ = tx.send(res);
            ctx_clone.request_repaint();
        });
    }

    let theme = ctx.app_theme();
    let popup_id = egui::Id::new("docker_source_switcher_popup");
    let modal_rect_id = popup_id.with("_rect");
    let last_rect: Option<Rect> = ctx.data(|d| d.get_temp(modal_rect_id));

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        dispatch(AppAction::CloseSourceSwitcher);
        return;
    }

    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !trigger_rect.contains(pos) && last_rect.is_some_and(|r| !r.contains(pos)) {
                dispatch(AppAction::CloseSourceSwitcher);
                return;
            }
        }
    }

    const MIN_SWITCHER_WIDTH: f32 = 180.0;
    const MAX_SWITCHER_WIDTH: f32 = 420.0;

    let candidate_containers = scan_state
        .loaded
        .as_deref()
        .map(|containers| {
            filter_project_candidate_containers(docker_opts, active_containers, containers)
        })
        .unwrap_or_default();

    let mut seen_active = HashSet::new();
    let deduped_active: Vec<&ActiveContainerSessionInfo> = active_containers
        .iter()
        .filter(|c| seen_active.insert(c.container_name.to_ascii_lowercase()))
        .collect();

    let area_pos = egui::pos2(trigger_rect.min.x, trigger_rect.max.y + 4.0);
    let available_below = (ctx.viewport_rect().max.y - area_pos.y - 12.0).max(120.0);
    let popup_max_height = 420.0_f32.min(available_below);

    let min_width = trigger_rect.width().max(MIN_SWITCHER_WIDTH);
    let available_right = (ctx.viewport_rect().max.x - area_pos.x - 12.0).max(min_width);
    let max_width = MAX_SWITCHER_WIDTH.min(available_right).max(min_width);

    let is_empty_scanning = scan_state.in_flight && deduped_active.is_empty();

    let area_resp = egui::Area::new(popup_id)
        .order(Order::Foreground)
        .fixed_pos(area_pos)
        .movable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::default()
                .fill(theme.surfaces.mantle)
                .stroke(Stroke::new(1.0, theme.borders.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::same(8))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(50),
                });
            let total_margin = frame.total_margin();
            let item_spacing_x = ui.spacing().item_spacing.x;

            let desired_content_width = ui.fonts_mut(|fonts| {
                if is_empty_scanning {
                    fonts
                        .layout_no_wrap(
                            "Scanning containers...".to_owned(),
                            egui::FontId::proportional(11.5),
                            egui::Color32::PLACEHOLDER,
                        )
                        .size()
                        .x
                } else {
                    let active_max = deduped_active
                        .iter()
                        .map(|c| {
                            let text_w = fonts
                                .layout_no_wrap(
                                    c.short_label.clone(),
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::PLACEHOLDER,
                                )
                                .size()
                                .x;
                            let check_w = if c.is_active {
                                12.0 + item_spacing_x
                            } else {
                                0.0
                            };
                            // Padding 2 đầu hàng (12.0)
                            text_w + check_w + 12.0
                        })
                        .fold(0.0_f32, f32::max);

                    let candidate_max = candidate_containers
                        .iter()
                        .map(|c| {
                            let text_w = fonts
                                .layout_no_wrap(
                                    c.short_label().to_owned(),
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::PLACEHOLDER,
                                )
                                .size()
                                .x;
                            text_w + 12.0
                        })
                        .fold(0.0_f32, f32::max);

                    // Cộng thêm khoảng dự phòng cho scrollbar dọc khi danh sách dài
                    active_max.max(candidate_max) + 14.0
                }
            });

            let popup_width =
                (desired_content_width + total_margin.sum().x).clamp(min_width, max_width);
            let inner_width = (popup_width - total_margin.sum().x).max(0.0);
            let inner_max_height = (popup_max_height - total_margin.sum().y).max(80.0);

            frame.show(ui, |ui| {
                ui.set_width(inner_width);
                ui.set_max_width(inner_width);
                ui.set_max_height(inner_max_height);

                if is_empty_scanning {
                    ui.label(
                        egui::RichText::new("Scanning containers...")
                            .size(11.5)
                            .color(theme.text.muted),
                    );
                    ctx.request_repaint_after(Duration::from_millis(50));
                    return;
                }

                if scan_state.in_flight {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }

                egui::ScrollArea::vertical()
                    .id_salt("source_switcher_containers_scroll")
                    .auto_shrink([false, true])
                    .max_height(inner_max_height)
                    .show(ui, |ui| {
                        for c in deduped_active {
                            let tip = format!("{}\n{}", c.container_name, c.source_summary);
                            let action = render_project_row(
                                ui,
                                ProjectRowConfig {
                                    icon: None,
                                    label: &c.short_label,
                                    is_active: c.is_active,
                                    location_tooltip: Some(&tip),
                                    action_icon: None,
                                    action_tooltip: None,
                                    close_tooltip: "",
                                },
                            );
                            if action == ProjectRowAction::Select {
                                dispatch(AppAction::SwitchSession(c.session_index));
                                dispatch(AppAction::CloseSourceSwitcher);
                            }
                        }

                        for c in candidate_containers {
                            let tip =
                                format!("{}\nImage: {}\nStatus: {}", c.name, c.image, c.status);
                            let action = render_project_row(
                                ui,
                                ProjectRowConfig {
                                    icon: None,
                                    label: c.short_label(),
                                    is_active: false,
                                    location_tooltip: Some(&tip),
                                    action_icon: None,
                                    action_tooltip: None,
                                    close_tooltip: "",
                                },
                            );
                            if action == ProjectRowAction::Select {
                                let mut opts =
                                    DockerConnectionOptions::new(&c.name, &docker_opts.working_dir)
                                        .with_container_id(&c.id)
                                        .with_devcontainer(c.is_devcontainer)
                                        .with_follow_container_logs(true);
                                if let Some(ref folder) = c.local_folder {
                                    opts = opts.with_local_folder(folder);
                                } else if let Some(ref folder) = docker_opts.local_folder {
                                    opts = opts.with_local_folder(folder);
                                }
                                if let Some(ref proj) = c.compose_project {
                                    opts = opts.with_compose_project(proj);
                                } else if let Some(ref proj) = docker_opts.compose_project {
                                    opts = opts.with_compose_project(proj);
                                }
                                if let Some(ref svc) = c.compose_service {
                                    opts = opts.with_compose_service(svc);
                                }

                                let ws = Workspace::new(
                                    &session.session.name,
                                    WorkspaceLocation::remote(opts),
                                    SourceType::Process,
                                );
                                dispatch(AppAction::OpenContainerLogSession(Box::new(ws)));
                            }
                        }
                    });
            });
        });

    ctx.data_mut(|d| d.insert_temp(modal_rect_id, area_resp.response.rect));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_switcher_measure_labels(labels: &[String]) -> Rect {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let ctx = egui::Context::default();
        let ws = Workspace::new(
            "demo",
            WorkspaceLocation::remote(DockerConnectionOptions::new(
                "demo-app-1",
                "/workspaces/demo",
            )),
            SourceType::Process,
        );
        let session = GuiSession::from_workspace(&ws, 1000, 1000);

        let active_containers: Vec<ActiveContainerSessionInfo> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| ActiveContainerSessionInfo {
                session_index: i,
                container_name: format!("demo-svc-{i}-1"),
                short_label: label.clone(),
                source_summary: "Container Logs".to_string(),
                is_active: i == 0,
            })
            .collect();

        let mut stack = OverlayStack::new();
        stack.push(OverlayLayer::SourceSwitcher);
        let trigger_rect = Rect::from_min_size(egui::pos2(120.0, 10.0), egui::vec2(80.0, 24.0));

        // Pre-seed loaded state so test doesn't depend on host Docker daemon
        let mut scan_state = SwitcherContainersState {
            in_flight: false,
            loaded: Some(Vec::new()),
            error: None,
            rx: None,
        };

        let raw_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };

        for _ in 0..2 {
            let mut output = ctx.run_ui(raw_input.clone(), |ui| {
                let ctx = ui.ctx();
                let mut dispatch = |_| {};
                render_source_switcher_popup(
                    ctx,
                    &session,
                    &active_containers,
                    &stack,
                    &mut scan_state,
                    rt.handle(),
                    &mut dispatch,
                    trigger_rect,
                );
            });
            output.textures_delta.clear();
        }

        let modal_rect_id = egui::Id::new("docker_source_switcher_popup").with("_rect");
        ctx.data(|d| d.get_temp(modal_rect_id))
            .unwrap_or(Rect::NOTHING)
    }

    fn run_switcher_measure(item_count: usize) -> Rect {
        let labels: Vec<String> = (0..item_count).map(|i| format!("svc-{i}")).collect();
        run_switcher_measure_labels(&labels)
    }

    #[test]
    fn source_switcher_popup_auto_grows_up_to_modal_picker_max_height() {
        let two_items = run_switcher_measure(2);
        let six_items = run_switcher_measure(6);
        let thirty_items = run_switcher_measure(30);

        assert!(
            (two_items.width() - 180.0).abs() <= 2.0,
            "expected short labels to clamp to min width ~180, got {}",
            two_items.width()
        );
        assert!(
            six_items.height() > two_items.height() + 90.0,
            "expected 6 items ({}) to be taller than 2 items ({})",
            six_items.height(),
            two_items.height()
        );
        assert!(
            thirty_items.height() <= 422.0 && thirty_items.height() >= 400.0,
            "expected 30 items to clamp near 420.0 max height, got {}",
            thirty_items.height()
        );
    }

    #[test]
    fn source_switcher_popup_width_clamps_between_min_and_max_without_horizontal_overflow() {
        let short_rect = run_switcher_measure_labels(&["app".into(), "db".into()]);
        let medium_rect = run_switcher_measure_labels(&[
            "app".into(),
            "psittacus-erithacus-background-worker".into(),
        ]);
        let long_rect = run_switcher_measure_labels(&[
            "super-long-container-service-name-that-should-truncate-at-max-width-without-expanding-popup-or-scrolling-horizontally".into(),
        ]);

        assert!(
            (short_rect.width() - 180.0).abs() <= 2.0,
            "expected min width ~180, got {}",
            short_rect.width()
        );
        assert!(
            medium_rect.width() > short_rect.width() + 40.0 && medium_rect.width() <= 422.0,
            "expected medium label to grow width between 180 and 420, got {}",
            medium_rect.width()
        );
        assert!(
            (long_rect.width() - 420.0).abs() <= 2.0,
            "expected long label to clamp at max width ~420 without horizontal overflow, got {}",
            long_rect.width()
        );
    }

    fn make_container(
        name: &str,
        compose_project: Option<&str>,
        compose_service: Option<&str>,
        local_folder: Option<&str>,
    ) -> DockerContainerInfo {
        DockerContainerInfo {
            id: format!("id-{name}"),
            name: name.to_string(),
            image: "img".to_string(),
            status: "Up".to_string(),
            local_folder: local_folder.map(|s| s.to_string()),
            config_file: None,
            compose_project: compose_project.map(|s| s.to_string()),
            compose_service: compose_service.map(|s| s.to_string()),
            is_devcontainer: local_folder.is_some(),
        }
    }

    #[test]
    fn filter_project_candidates_excludes_other_compose_groups_and_duplicates() {
        let docker_opts =
            DockerConnectionOptions::new("psittacus-erithacus_devcontainer-app-1", "/workspaces/p")
                .with_compose_project("psittacus-erithacus_devcontainer")
                .with_local_folder("/home/u/psittacus-erithacus");

        let active = vec![
            ActiveContainerSessionInfo {
                session_index: 0,
                container_name: "psittacus-erithacus_devcontainer-app-1".to_string(),
                short_label: "app".to_string(),
                source_summary: "Ready".to_string(),
                is_active: false,
            },
            ActiveContainerSessionInfo {
                session_index: 1,
                container_name: "psittacus-erithacus_devcontainer-db-1".to_string(),
                short_label: "db".to_string(),
                source_summary: "Container Logs".to_string(),
                is_active: true,
            },
        ];

        let running = vec![
            make_container(
                "devcontainer-frontend-1",
                Some("devcontainer"),
                Some("frontend"),
                None,
            ),
            make_container(
                "psittacus-erithacus_devcontainer-app-1",
                Some("psittacus-erithacus_devcontainer"),
                Some("app"),
                Some("/home/u/psittacus-erithacus"),
            ),
            make_container(
                "psittacus-erithacus_devcontainer-db-1",
                Some("psittacus-erithacus_devcontainer"),
                Some("db"),
                None,
            ),
            make_container(
                "psittacus-erithacus_devcontainer-minio-1",
                Some("psittacus-erithacus_devcontainer"),
                Some("minio"),
                None,
            ),
            make_container(
                "psittacus-erithacus-front-end_devcontainer-frontend-1",
                Some("psittacus-erithacus-front-end_devcontainer"),
                Some("frontend"),
                Some("/home/u/psittacus-erithacus-front-end"),
            ),
        ];

        let candidates = filter_project_candidate_containers(&docker_opts, &active, &running);
        let names: Vec<&str> = candidates.iter().map(|c| c.short_label()).collect();
        assert_eq!(names, vec!["minio"]);
    }
}
