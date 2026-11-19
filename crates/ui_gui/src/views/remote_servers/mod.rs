pub mod devcontainer_picker;
pub mod folder_picker;
pub mod helpers;
pub mod server_list;
pub mod server_options;
pub mod ssh_picker;
pub mod types;
pub mod wsl_picker;

pub use devcontainer_picker::{render_devcontainer_picker_subview, DevContainerConnectSession};
pub use folder_picker::render_folder_picker_subview;
pub use helpers::*;
pub use server_list::{
    collect_server_list_items, render_remote_list_subview, ServerListAction, ServerListItem,
};
pub use server_options::render_server_options_subview;
pub use ssh_picker::{render_ssh_picker_subview, SshInteractiveSession};
pub use types::*;
pub use wsl_picker::render_wsl_picker_subview;

use crate::actions::{AppAction, OverlayAction, WorkspaceAction};
use crate::app::overlay_manager::RemoteServersOverlayState;
use crate::keymap::KeymapManager;
use crate::overlay::RemoteModalPlacement;
use eframe::egui;
use uwu_core_workspace::{Workspace, WorkspaceLocation, WorkspaceStore};

/// Render modal chọn Remote Projects theo thiết kế chuẩn của Zed
#[allow(clippy::too_many_arguments)]
pub fn render_remote_servers_modal(
    ctx: &egui::Context,
    is_open: bool,
    placement: RemoteModalPlacement,
    overlay_state: &mut RemoteServersOverlayState,
    rt: &tokio::runtime::Handle,
    keymap: &KeymapManager,
    store: &mut WorkspaceStore,
    active_location: Option<&WorkspaceLocation>,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !is_open {
        return;
    }

    let state = &mut overlay_state.modal;
    let ssh_session = &mut overlay_state.ssh_session;
    let devcontainer_session = &mut overlay_state.devcontainer_session;

    let mut selected_workspace: Option<Workspace> = None;
    let mut nav_action = RemoteNavAction::None;

    let closed = RemotePickerContainer::new("remote_servers_picker_container")
        .placement(placement)
        .width(520.0)
        .max_height(480.0)
        .close_on_escape(false)
        .show(ctx, |ui| match &mut state.subview {
            RemoteSubView::List => {
                let (action, ws) = render_remote_list_subview(
                    ui,
                    &mut state.search_query,
                    &mut state.selected_index,
                    keymap,
                    store,
                    active_location,
                );
                nav_action = action;
                selected_workspace = ws;
            }
            RemoteSubView::WslPicker => {
                nav_action = render_wsl_picker_subview(ui, keymap, store);
            }
            RemoteSubView::SshPicker(ssh_state) => {
                nav_action =
                    render_ssh_picker_subview(ui, ssh_state, ssh_session, rt, keymap, store);
            }
            RemoteSubView::DevContainerPicker(dc_state) => {
                let (action, ws) = render_devcontainer_picker_subview(
                    ui,
                    dc_state,
                    devcontainer_session,
                    rt,
                    keymap,
                    store,
                );
                nav_action = action;
                selected_workspace = ws;
            }
            RemoteSubView::FolderPicker(folder_state) => {
                let (action, ws) = render_folder_picker_subview(ui, folder_state, keymap, store);
                nav_action = action;
                selected_workspace = ws;
            }
            RemoteSubView::ServerOptions(options_state) => {
                nav_action = render_server_options_subview(ui, options_state, keymap, store);
            }
        });

    match nav_action {
        RemoteNavAction::Navigate(next) => {
            overlay_state.modal.navigate(*next);
            return;
        }
        RemoteNavAction::Back => {
            if !overlay_state.modal.back() {
                // Không còn màn hình nào trong history để back -> Đóng modal
                overlay_state.reset();
                dispatch(OverlayAction::CloseRemoteServers.into());
                return;
            }
            return;
        }
        RemoteNavAction::None => {}
    }

    // Xử lý mở workspace đã chọn
    if let Some(ws) = selected_workspace {
        overlay_state.reset();
        dispatch(OverlayAction::CloseRemoteServers.into());
        dispatch(WorkspaceAction::Open(Box::new(ws)).into());
        return;
    }

    if closed {
        overlay_state.reset();
        dispatch(OverlayAction::CloseRemoteServers.into());
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests;
