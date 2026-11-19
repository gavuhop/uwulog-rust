pub mod environment;
pub mod remote;
pub mod session;
pub mod store;
pub mod workspace;

pub use environment::{load_workspace_environment, parse_dot_env, EnvLoadStatus};
pub use remote::{
    DockerConnection, DockerConnectionOptions, RemoteConnectionOptions, RemoteProject,
    ServerConnection, SshConnection, SshConnectionOptions, WslConnection, WslConnectionOptions,
};
pub use session::{SourceConfig, SourceType, WorkspaceSession};
pub use store::WorkspaceStore;
pub use uwu_icons::IconName;
pub use workspace::{
    clean_path, extract_project_name, normalize_workdir, sanitize_project_name, SavedColumnConfig,
    ViewerPrefs, Workspace, WorkspaceLocation,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_store_crud() {
        let mut store = WorkspaceStore::default();
        assert!(store.recent_workspaces.is_empty());

        let ws1 = Workspace::new(
            "backend-service",
            WorkspaceLocation::remote(WslConnectionOptions::new("Ubuntu", "/home/user/backend")),
            SourceType::Process,
        );
        let id1 = ws1.id;

        store.add_or_update(ws1);
        assert_eq!(store.recent_workspaces.len(), 1);
        assert_eq!(store.active_workspace_id, Some(id1));
        assert_eq!(store.get_active().unwrap().name, "backend-service");

        let ws2 = Workspace::new(
            "frontend-app",
            WorkspaceLocation::local("D:\\Projects\\frontend"),
            SourceType::Process,
        );
        let id2 = ws2.id;
        store.add_or_update(ws2);
        assert_eq!(store.recent_workspaces.len(), 2);
        assert_eq!(store.active_workspace_id, Some(id2));

        store.remove(id2);
        assert_eq!(store.recent_workspaces.len(), 1);
        assert_eq!(store.active_workspace_id, Some(id1));
    }

    #[test]
    fn test_find_by_workdir_normalization() {
        let mut store = WorkspaceStore::default();
        let ws = Workspace::new(
            "uwulog-rust",
            WorkspaceLocation::local("D:\\Learn\\Go\\uwulog-rust"),
            SourceType::Process,
        );
        store.add_or_update(ws);

        // Exact match
        assert!(store
            .find_by_workdir("D:\\Learn\\Go\\uwulog-rust")
            .is_some());
        // Forward slash match
        assert!(store.find_by_workdir("D:/Learn/Go/uwulog-rust").is_some());
        // Trailing slash
        assert!(store.find_by_workdir("D:/Learn/Go/uwulog-rust/").is_some());
        assert!(store
            .find_by_workdir("D:\\Learn\\Go\\uwulog-rust\\")
            .is_some());
        // Windows verbatim prefix
        assert!(store
            .find_by_workdir(r"\\?\D:\Learn\Go\uwulog-rust")
            .is_some());
        // Case insensitivity
        assert!(store.find_by_workdir("d:/learn/go/uwulog-rust").is_some());
        // Non-matching dir
        assert!(store.find_by_workdir("D:/Other/Path").is_none());
    }

    #[test]
    fn test_is_same_docker_project() {
        let app = WorkspaceLocation::remote(
            DockerConnectionOptions::new("proj-app-1", "/workspaces/p")
                .with_compose_project("proj_devc")
                .with_local_folder("/home/me/proj"),
        );
        let db = WorkspaceLocation::remote(
            DockerConnectionOptions::new("proj-db-1", "/workspaces/p")
                .with_compose_project("proj_devc"),
        );
        let other = WorkspaceLocation::remote(
            DockerConnectionOptions::new("other-app-1", "/workspaces/o")
                .with_compose_project("other_devc")
                .with_local_folder("/home/me/other"),
        );
        let same_folder = WorkspaceLocation::remote(
            DockerConnectionOptions::new("proj-minio-1", "/data")
                .with_local_folder("/home/me/proj"),
        );
        assert!(app.is_same_docker_project(&db));
        assert!(app.is_same_docker_project(&same_folder));
        assert!(!app.is_same_docker_project(&other));
        assert!(!app.is_same_docker_project(&WorkspaceLocation::local("/home/me/proj")));
    }

    #[test]
    fn test_workspace_serde_json_compatibility() {
        let ws = Workspace::new(
            "test-app",
            WorkspaceLocation::local("C:\\Projects\\app"),
            SourceType::Process,
        );

        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"source_type\":\"process\""));

        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source_type, SourceType::Process);
        assert_eq!(deserialized.name, "test-app");

        let wsl_ws = Workspace::new(
            "wsl-app",
            WorkspaceLocation::remote(WslConnectionOptions::new("Ubuntu", "/home/user")),
            SourceType::Process,
        );
        let wsl_json = serde_json::to_string(&wsl_ws).unwrap();
        let deserialized_wsl: Workspace = serde_json::from_str(&wsl_json).unwrap();
        assert_eq!(deserialized_wsl.source_type, SourceType::Process);
        assert!(deserialized_wsl.location.is_remote());
        assert_eq!(deserialized_wsl.location.display_name(), "Ubuntu");
        assert_eq!(deserialized_wsl.location.icon(), IconName::Linux);
        assert_eq!(deserialized_wsl.location.working_dir(), "/home/user");

        // WorkspaceStore::viewers lưu ViewerPrefs thống nhất theo ViewerKey
        let mut store = WorkspaceStore::default();
        let prefs = ViewerPrefs {
            main_query: "level:error".to_string(),
            filter_history: vec!["level:error".to_string()],
            columns: vec![SavedColumnConfig {
                name: "aaa".to_string(),
                visible: true,
                width: 220.0,
                default_role: None,
            }],
            inspector_width_ratio: 0.42,
        };
        let key = ws.viewer_key();
        store.upsert_viewer_prefs(key.clone(), prefs.clone());
        let store_json = serde_json::to_string(&store).unwrap();
        let store_back: WorkspaceStore = serde_json::from_str(&store_json).unwrap();
        assert_eq!(store_back.get_viewer_prefs(&key), Some(&prefs));
    }

    #[test]
    fn test_viewer_key_formats_and_isolation() {
        let local_proc = WorkspaceStore::viewer_key(
            &WorkspaceLocation::local("/home/me/proj/"),
            SourceType::Process,
            "",
        );
        assert_eq!(local_proc, "local:/home/me/proj");

        let local_file_a = WorkspaceStore::viewer_key(
            &WorkspaceLocation::local("/home/me/proj"),
            SourceType::File,
            "/var/log/a.log",
        );
        let local_file_b = WorkspaceStore::viewer_key(
            &WorkspaceLocation::local("/home/me/proj"),
            SourceType::File,
            "/var/log/b.log",
        );
        assert_eq!(local_file_a, "local:/home/me/proj#file:/var/log/a.log");
        assert_ne!(local_file_a, local_file_b);

        let ssh_a = WorkspaceStore::viewer_key(
            &WorkspaceLocation::remote(
                SshConnectionOptions::new("srv-a.example.com", "/srv/proj").with_username("dev"),
            ),
            SourceType::Process,
            "",
        );
        let ssh_b = WorkspaceStore::viewer_key(
            &WorkspaceLocation::remote(
                SshConnectionOptions::new("srv-b.example.com", "/srv/proj").with_username("dev"),
            ),
            SourceType::Process,
            "",
        );
        assert_eq!(ssh_a, "ssh:dev@srv-a.example.com:22/srv/proj");
        assert_ne!(ssh_a, ssh_b);

        let wsl_key = WorkspaceStore::viewer_key(
            &WorkspaceLocation::remote(WslConnectionOptions::new("Ubuntu", "/home/me/proj")),
            SourceType::Process,
            "",
        );
        assert_eq!(wsl_key, "wsl:ubuntu:/home/me/proj");

        // Docker: ưu tiên local_folder + compose_service -> container tạo lại (-2) vẫn khớp key cũ
        let docker_db_1 = WorkspaceStore::viewer_key(
            &WorkspaceLocation::remote(
                DockerConnectionOptions::new("proj-db-1", "/workspaces/app")
                    .with_local_folder("/home/me/proj-a")
                    .with_compose_service("db"),
            ),
            SourceType::Process,
            "",
        );
        let docker_db_2 = WorkspaceStore::viewer_key(
            &WorkspaceLocation::remote(
                DockerConnectionOptions::new("proj-db-2", "/workspaces/app")
                    .with_local_folder("/home/me/proj-a")
                    .with_compose_service("db"),
            ),
            SourceType::Process,
            "",
        );
        let docker_other_proj = WorkspaceStore::viewer_key(
            &WorkspaceLocation::remote(
                DockerConnectionOptions::new("proj-db-1", "/workspaces/app")
                    .with_local_folder("/home/me/proj-b")
                    .with_compose_service("db"),
            ),
            SourceType::Process,
            "",
        );
        assert_eq!(docker_db_1, "docker:/home/me/proj-a/db");
        assert_eq!(docker_db_1, docker_db_2);
        assert_ne!(docker_db_1, docker_other_proj);
    }

    #[tokio::test]
    async fn test_workspace_domain_methods() {
        let mut local_ws = Workspace::new(
            "my-app",
            WorkspaceLocation::local("C:\\Projects\\app"),
            SourceType::Process,
        );
        assert_eq!(local_ws.icon(), IconName::Screen);
        assert_eq!(local_ws.server_name(), None);
        assert_eq!(local_ws.target_summary(), "C:\\Projects\\app");
        assert!(!local_ws.location.is_remote());
        assert_eq!(format!("{}", local_ws.location), "C:\\Projects\\app");

        local_ws.location.set_working_dir("C:\\Projects\\app2");
        assert_eq!(local_ws.location.working_dir(), "C:\\Projects\\app2");

        let mut wsl_ws = Workspace::new(
            "ubuntu-service",
            WorkspaceLocation::remote(WslConnectionOptions::new(
                "Ubuntu-22.04",
                "/home/user/service",
            )),
            SourceType::Process,
        );
        assert_eq!(wsl_ws.icon(), IconName::Linux);
        assert_eq!(wsl_ws.server_name(), Some("Ubuntu-22.04"));
        assert_eq!(wsl_ws.target_summary(), "/home/user/service (Ubuntu-22.04)");
        assert!(wsl_ws.location.is_remote());
        assert_eq!(
            format!("{}", wsl_ws.location),
            "/home/user/service (Ubuntu-22.04)"
        );

        wsl_ws.location.set_working_dir("/home/user/service2");
        assert_eq!(wsl_ws.location.working_dir(), "/home/user/service2");

        let file_ws = Workspace::new(
            "syslog",
            WorkspaceLocation::local("C:\\Logs"),
            SourceType::File,
        );
        assert_eq!(file_ws.icon(), IconName::File);

        let session = WorkspaceSession::from_workspace(&wsl_ws, 100, 50);
        assert_eq!(session.icon(), IconName::Linux);
        assert_eq!(
            session.location.as_remote().map(|r| r.display_name()),
            Some("Ubuntu-22.04")
        );
        assert_eq!(
            session.target_summary(),
            "/home/user/service2 (Ubuntu-22.04)"
        );
    }

    #[test]
    fn test_workspace_store_wsl_connections_crud() {
        let mut store = WorkspaceStore::default();
        store.ensure_wsl_connection("Ubuntu");
        store.ensure_wsl_connection("Debian");
        assert_eq!(store.wsl_connections.len(), 2);
        assert_eq!(store.wsl_connections[0].distro, "Ubuntu");
        assert_eq!(store.wsl_connections[1].distro, "Debian");

        // Thêm project vào Ubuntu
        store.add_remote_project_to_server("Ubuntu", "/home/user/backend");
        store.add_remote_project_to_server("Ubuntu", "/home/user/frontend");
        // Kiểm tra BTreeSet deduplication
        store.add_remote_project_to_server("Ubuntu", "/home/user/backend");

        let ubuntu = store.find_wsl_connection("Ubuntu").unwrap();
        assert_eq!(ubuntu.projects.len(), 2);
        assert!(ubuntu
            .projects
            .contains(&RemoteProject::new("/home/user/backend")));
        assert!(ubuntu
            .projects
            .contains(&RemoteProject::new("/home/user/frontend")));

        let ws_ubuntu = Workspace::new(
            "ubuntu-app",
            WorkspaceLocation::remote(WslConnectionOptions::new("Ubuntu", "/home/user/app")),
            SourceType::Process,
        );
        let ws_debian = Workspace::new(
            "debian-app",
            WorkspaceLocation::remote(WslConnectionOptions::new("Debian", "/home/user/app")),
            SourceType::Process,
        );
        let ws_local = Workspace::new(
            "local-app",
            WorkspaceLocation::local("C:\\Projects\\local"),
            SourceType::Process,
        );
        store.add_or_update(ws_ubuntu);
        store.add_or_update(ws_debian);
        store.add_or_update(ws_local);
        assert_eq!(store.recent_workspaces.len(), 3);

        // Xóa project khỏi Ubuntu
        store.remove_remote_project_from_server("Ubuntu", "/home/user/frontend");
        assert_eq!(
            store.find_wsl_connection("Ubuntu").unwrap().projects.len(),
            1
        );

        // Xóa server Ubuntu -> Xóa cả server lẫn workspaces remote thuộc về nó
        store.remove_wsl_connection("Ubuntu");
        assert_eq!(store.wsl_connections.len(), 1);
        assert_eq!(store.wsl_connections[0].distro, "Debian");
        assert_eq!(store.recent_workspaces.len(), 2);
        assert!(store
            .recent_workspaces
            .iter()
            .all(|w| { w.location.as_remote().map(|r| r.display_name()) != Some("Ubuntu") }));
    }

    #[test]
    fn test_workspace_project_name_deduplication() {
        // Test 1: ws.name already has (Ubuntu) suffix -> sanitized in Workspace::new
        let ws = Workspace::new(
            "ai-learn-english (Ubuntu)",
            WorkspaceLocation::remote(WslConnectionOptions::new(
                "Ubuntu",
                "/home/truongviet/project/ai-learn-english",
            )),
            SourceType::Process,
        );

        assert_eq!(ws.name, "ai-learn-english");
        assert_eq!(ws.server_name(), Some("Ubuntu"));

        // Test 2: ws.name does not have suffix
        let ws2 = Workspace::new(
            "ai-learn-english",
            WorkspaceLocation::remote(WslConnectionOptions::new(
                "Ubuntu",
                "/home/truongviet/project/ai-learn-english",
            )),
            SourceType::Process,
        );

        assert_eq!(ws2.name, "ai-learn-english");
        assert_eq!(ws2.server_name(), Some("Ubuntu"));

        // Test 3: Local workspace
        let ws_local = Workspace::new(
            "my-app",
            WorkspaceLocation::local("D:\\projects\\my-app"),
            SourceType::Process,
        );
        assert_eq!(ws_local.name, "my-app");
        assert_eq!(ws_local.server_name(), None);
    }

    #[test]
    fn test_workspace_store_ssh_connections_crud() {
        let mut store = WorkspaceStore::default();
        store.ensure_ssh_connection("prod-server");
        store.ensure_ssh_connection("staging-server");
        assert_eq!(store.ssh_connections.len(), 2);
        assert_eq!(store.ssh_connections[0].host, "prod-server");
        assert_eq!(store.ssh_connections[1].host, "staging-server");

        // Thêm project vào prod-server
        store.add_remote_project_to_ssh_server("prod-server", "/var/log/nginx");
        store.add_remote_project_to_ssh_server("prod-server", "/var/log/backend");
        // Kiểm tra deduplication
        store.add_remote_project_to_ssh_server("prod-server", "/var/log/nginx");

        let prod = store.find_ssh_connection("prod-server").unwrap();
        assert_eq!(prod.projects.len(), 2);
        assert!(prod
            .projects
            .contains(&RemoteProject::new("/var/log/nginx")));
        assert!(prod
            .projects
            .contains(&RemoteProject::new("/var/log/backend")));

        let ws_ssh = Workspace::new(
            "nginx-log",
            WorkspaceLocation::remote(SshConnectionOptions::new("prod-server", "/var/log/nginx")),
            SourceType::Process,
        );
        store.add_or_update(ws_ssh);
        assert_eq!(store.recent_workspaces.len(), 1);

        // Xóa project khỏi prod-server
        store.remove_remote_project_from_ssh_server("prod-server", "/var/log/nginx");
        assert_eq!(
            store
                .find_ssh_connection("prod-server")
                .unwrap()
                .projects
                .len(),
            1
        );
        assert_eq!(store.recent_workspaces.len(), 0);

        // Xóa SSH server
        store.remove_ssh_connection("prod-server");
        assert_eq!(store.ssh_connections.len(), 1);
        assert_eq!(store.ssh_connections[0].host, "staging-server");
    }

    #[test]
    fn test_workspace_store_docker_connections_crud() {
        let mut store = WorkspaceStore::default();
        let dc = DockerConnection::new("rust-dev")
            .with_container_id("abc123def456")
            .with_image("mcr.microsoft.com/devcontainers/rust:1")
            .with_local_folder("/home/user/app")
            .with_devcontainer(true);
        store.upsert_docker_connection(dc);
        store.ensure_docker_connection("redis-cache");

        assert_eq!(store.docker_connections.len(), 2);
        assert!(
            store
                .find_docker_connection("rust-dev")
                .unwrap()
                .is_devcontainer
        );
        assert!(
            store
                .find_docker_connection("abc123def456")
                .unwrap()
                .container_name
                == "rust-dev"
        );

        store.add_remote_project_to_docker_server("rust-dev", "/workspaces/app");
        store.add_remote_project_to_docker_server("rust-dev", "/workspaces/app");
        assert_eq!(
            store
                .find_docker_connection("rust-dev")
                .unwrap()
                .projects
                .len(),
            1
        );

        store.sync_remote_projects();
        assert_eq!(store.recent_workspaces.len(), 1);
        assert_eq!(store.recent_workspaces[0].location.icon(), IconName::Box);
        assert!(store.recent_workspaces[0].is_valid_to_open());

        store.remove_remote_project_from_docker_server("rust-dev", "/workspaces/app");
        assert_eq!(store.recent_workspaces.len(), 0);

        store.remove_docker_connection("rust-dev");
        assert_eq!(store.docker_connections.len(), 1);
        assert_eq!(store.docker_connections[0].container_name, "redis-cache");
    }

    #[test]
    fn test_docker_connection_options_parse_and_serde() {
        let opts = RemoteConnectionOptions::parse("devcontainer:my-dev", "/workspaces/app");
        assert_eq!(opts.connection_type(), "docker");
        assert_eq!(opts.display_name(), "my-dev");
        assert_eq!(opts.working_dir(), "/workspaces/app");
        assert_eq!(opts.icon(), IconName::Box);

        let opts2 = RemoteConnectionOptions::parse("docker:redis", "/data");
        assert_eq!(opts2.connection_type(), "docker");
        assert_eq!(opts2.display_name(), "redis");

        let json = serde_json::to_string(&opts).unwrap();
        let back: RemoteConnectionOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back, opts);
    }

    #[tokio::test]
    async fn test_session_source_badge_and_container_logs_roundtrip() {
        let mut ws = Workspace::new(
            "psittacus",
            WorkspaceLocation::remote(
                DockerConnectionOptions::new("psittacus_app_1", "/workspaces/psittacus")
                    .with_compose_project("psittacus_devcontainer")
                    .with_compose_service("app")
                    .with_devcontainer(true),
            ),
            SourceType::Process,
        );
        let mut session = WorkspaceSession::from_workspace(&ws, 1000, 100);
        assert_eq!(session.source_badge_label(), "Ready (Idle)");

        session.source_config.command_str = "go run .".to_string();
        assert_eq!(session.source_badge_label(), "cmd: go run .");

        if let WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(ref mut d)) =
            session.location
        {
            d.follow_container_logs = true;
        }
        assert!(session.is_following_container_logs());
        assert_eq!(session.source_badge_label(), "container: app");
        assert_eq!(session.active_docker_short_label().as_deref(), Some("app"));
        assert_eq!(session.short_server_name(), None);

        ws = session.to_workspace();
        assert_eq!(ws.source_type, SourceType::Process);
        assert_eq!(ws.command_str, "go run .");

        let restored = WorkspaceSession::from_workspace(&ws, 1000, 100);
        assert_eq!(restored.source_config.command_str, "go run .");
        assert!(restored.is_following_container_logs());
        assert_eq!(restored.source_config.source_type, SourceType::Process);
    }
}
