use super::test_helpers::create_test_app;
use crate::app::UwuGuiApp;
use crate::cli::CliArgs;
use clap::Parser;
use std::time::{Duration, Instant};
use uwu_core_workspace::{SourceType, Workspace, WorkspaceLocation, WorkspaceStore};

#[tokio::test]
async fn test_spawn_load_environment_and_tick() {
    let mut app = create_test_app();
    app.spawn_load_environment();

    tokio::time::sleep(Duration::from_millis(350)).await;
    app.tick();

    assert!(matches!(
        app.active_session().session.env_status,
        uwu_core_workspace::EnvLoadStatus::Ready { .. }
    ));
}

#[tokio::test]
async fn test_source_running_state_transitions_to_stopped() {
    let mut app = create_test_app();
    assert!(!app.active_session().session.is_source_running);

    #[cfg(target_os = "windows")]
    {
        app.active_session_mut().session.source_config.command_str = "cmd /c echo test".to_string();
    }
    #[cfg(not(target_os = "windows"))]
    {
        app.active_session_mut().session.source_config.command_str = "echo test".to_string();
    }

    app.start_configured_source();
    assert!(app.active_session().session.is_source_running);

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(50)).await;
        app.tick();
        if !app.active_session().session.is_source_running {
            break;
        }
    }

    assert!(!app.active_session().session.is_source_running);
}

#[test]
fn test_cli_args_parsing() {
    // 1. Test default arguments
    let defaults = CliArgs::try_parse_from(["uwu-gui"]).unwrap();
    assert_eq!(defaults.display_limit, 5000);
    assert_eq!(defaults.capacity, 500_000);
    assert_eq!(defaults.cmd, None);
    assert_eq!(defaults.file, None);
    assert_eq!(defaults.working_dir, None);
    assert_eq!(defaults.remote, None);
    assert_eq!(defaults.query, None);
    assert_eq!(defaults.path, None);
    assert!(defaults.trailing_cmd.is_empty());

    // 2. Test standard short flags, query and positional path
    let parsed = CliArgs::try_parse_from([
        "uwu-gui",
        "-n",
        "1234",
        "-C",
        "50000",
        "-c",
        "cargo run",
        "-d",
        "C:/my_project",
        "-q",
        "level:error",
        "server.log",
    ])
    .unwrap();
    assert_eq!(parsed.display_limit, 1234);
    assert_eq!(parsed.capacity, 50000);
    assert_eq!(parsed.cmd.as_deref(), Some("cargo run"));
    assert_eq!(parsed.working_dir.as_deref(), Some("C:/my_project"));
    assert_eq!(parsed.query.as_deref(), Some("level:error"));
    assert_eq!(parsed.path.as_deref(), Some("server.log"));

    // 3. Test short_alias -r for command
    let r_flag = CliArgs::try_parse_from(["uwu-gui", "-r", "python app.py"]).unwrap();
    assert_eq!(r_flag.cmd.as_deref(), Some("python app.py"));

    // 4. Test trailing command after `--`
    let trailing = CliArgs::try_parse_from([
        "uwu-gui",
        "-d",
        "/repo",
        "--",
        "cargo",
        "test",
        "--workspace",
    ])
    .unwrap();
    assert_eq!(trailing.working_dir.as_deref(), Some("/repo"));
    assert_eq!(
        trailing.trailing_cmd,
        vec![
            "cargo".to_string(),
            "test".to_string(),
            "--workspace".to_string()
        ]
    );

    // 5. Test standard remote flag
    let remote_clean = CliArgs::try_parse_from([
        "uwu-gui",
        "--cmd",
        "htop",
        "--remote",
        "Debian",
        "--dir",
        "/home/user",
    ])
    .unwrap();
    assert_eq!(remote_clean.cmd.as_deref(), Some("htop"));
    assert_eq!(remote_clean.remote.as_deref(), Some("Debian"));
    assert_eq!(remote_clean.working_dir.as_deref(), Some("/home/user"));

    // 6. Test backward compatibility aliases (wsl legacy)
    let wsl_legacy = CliArgs::try_parse_from([
        "uwu-gui",
        "--wsl-cmd",
        "tail -f log",
        "--wsl-distro",
        "Ubuntu-22.04",
        "--wsl-cwd",
        "/var/log",
    ])
    .unwrap();
    assert_eq!(wsl_legacy.cmd.as_deref(), Some("tail -f log"));
    assert_eq!(wsl_legacy.remote.as_deref(), Some("Ubuntu-22.04"));
    assert_eq!(wsl_legacy.working_dir.as_deref(), Some("/var/log"));
}

#[tokio::test]
async fn test_resolve_target_and_build_initial_session() {
    let store = WorkspaceStore::default();

    // Case 1: Remote CLI with command
    let cli_remote = CliArgs::try_parse_from([
        "uwu-gui",
        "--remote",
        "Ubuntu",
        "-d",
        "/var/log",
        "-c",
        "journalctl -f",
        "-q",
        "error",
    ])
    .unwrap();
    let (loc, src_type, cmd, file, has_custom) = cli_remote.resolve_target();
    assert!(loc.is_remote());
    assert_eq!(loc.working_dir(), "/var/log");
    assert_eq!(src_type, SourceType::Process);
    assert_eq!(cmd, "journalctl -f");
    assert_eq!(file, "");
    assert!(has_custom);

    let (session, custom_src, _) = UwuGuiApp::build_initial_session(&cli_remote, &store);
    assert!(custom_src);
    assert_eq!(session.session.location, loc);
    assert_eq!(session.view.search.query, "error");

    // Case 2: Positional log file on local
    let cli_file = CliArgs::try_parse_from(["uwu-gui", "production.log"]).unwrap();
    let (loc, src_type, cmd, file, has_custom) = cli_file.resolve_target();
    assert!(!loc.is_remote());
    assert_eq!(src_type, SourceType::File);
    assert_eq!(cmd, "");
    assert_eq!(file, "production.log");
    assert!(has_custom);

    let (session, custom_src, _) = UwuGuiApp::build_initial_session(&cli_file, &store);
    assert!(custom_src);
    assert_eq!(session.session.source_config.file_path, "production.log");
}

#[tokio::test]
async fn test_build_initial_session_preserves_saved_workspace_id_and_envs() {
    let mut store = WorkspaceStore::default();
    let mut saved_ws = Workspace::new(
        "SavedProj",
        WorkspaceLocation::local("D:\\my\\repo"),
        SourceType::Process,
    );
    let saved_uuid = saved_ws.id;
    saved_ws
        .env_vars
        .insert("ENV_KEY".to_string(), "ENV_VAL".to_string());
    store.recent_workspaces.push(saved_ws);

    let cli =
        CliArgs::try_parse_from(["uwu-gui", "-d", "D:\\my\\repo", "-c", "custom-cmd"]).unwrap();
    let (session, has_custom, saved_id) = UwuGuiApp::build_initial_session(&cli, &store);

    assert!(has_custom);
    assert_eq!(saved_id, Some(saved_uuid));
    assert_eq!(session.session.id, saved_uuid);
    assert_eq!(session.session.source_config.command_str, "custom-cmd");
    assert_eq!(
        session.session.env_vars.get("ENV_KEY").map(String::as_str),
        Some("ENV_VAL")
    );
}

#[tokio::test]
async fn test_build_initial_session_with_workspace_id() {
    let mut store = WorkspaceStore::default();
    let mut saved_ws = Workspace::new(
        "DirectWorkspace",
        WorkspaceLocation::local("D:\\target\\project"),
        SourceType::Process,
    );
    let target_uuid = saved_ws.id;
    saved_ws.command_str = "cargo test".to_string();
    store.recent_workspaces.push(saved_ws);

    let cli =
        CliArgs::try_parse_from(["uwu-gui", "--workspace-id", &target_uuid.to_string()]).unwrap();
    assert!(cli.has_target());
    assert_eq!(cli.workspace_id, Some(target_uuid));

    let (session, has_custom, saved_id) = UwuGuiApp::build_initial_session(&cli, &store);
    assert!(!has_custom);
    assert_eq!(saved_id, Some(target_uuid));
    assert_eq!(session.session.id, target_uuid);
    assert_eq!(session.session.name, "DirectWorkspace");
    assert_eq!(session.session.source_config.command_str, "cargo test");
}
