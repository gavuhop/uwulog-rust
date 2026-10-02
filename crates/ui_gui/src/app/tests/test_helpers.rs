use crate::app::overlay_manager::OverlayManager;
use crate::app::workspace_manager::WorkspaceManager;
use crate::app::UwuGuiApp;
use crate::session::GuiSession;
use uwu_core_workspace::{
    SourceConfig, SourceType, WorkspaceLocation, WorkspaceSession, WorkspaceStore,
};

pub fn create_test_app() -> UwuGuiApp {
    let rt = tokio::runtime::Handle::current();

    let source_config = SourceConfig {
        source_type: SourceType::Process,
        command_str: String::new(),
        file_path: String::new(),
        working_dir: String::new(),
        capacity: 100,
        display_limit: 50,
    };

    let session = WorkspaceSession::new(
        "Test Project".to_string(),
        WorkspaceLocation::local(""),
        source_config,
    );

    let gui_session = GuiSession::new(session);
    let store = WorkspaceStore::default();

    let workspaces = WorkspaceManager::new(gui_session, store);
    let overlays = OverlayManager::new();
    let keymap = crate::keymap::KeymapManager::new();
    let (event_tx, event_rx) = std::sync::mpsc::channel();

    UwuGuiApp {
        workspaces,
        overlays,
        keymap,
        rt,
        prev_screen_width: 0.0,
        should_quit: false,
        event_tx,
        event_rx,
        egui_ctx: None,
    }
}
