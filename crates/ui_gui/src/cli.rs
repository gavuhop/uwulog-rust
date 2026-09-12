use clap::Parser;
use uwu_core_workspace::{RemoteConnectionOptions, SourceType, WorkspaceLocation};

/// Tham số dòng lệnh khi khởi chạy uwu-gui
#[derive(Parser, Debug, Clone)]
#[command(
    name = "uwu-gui",
    about = "High-performance log viewer & workspace monitor",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct CliArgs {
    /// Đường dẫn file log hoặc thư mục workspace cần mở
    #[arg(value_name = "PATH", value_hint = clap::ValueHint::AnyPath)]
    pub path: Option<String>,

    /// Lệnh thực thi để thu thập log stdout/stderr (ví dụ: -c "cargo run")
    #[arg(
        short = 'c',
        short_alias = 'r',
        long = "cmd",
        visible_aliases = ["run", "command", "exec", "remote-cmd", "wsl-cmd"]
    )]
    pub cmd: Option<String>,

    /// Đường dẫn file log cần theo dõi (file tailer)
    #[arg(
        short = 'f',
        long = "file",
        visible_aliases = ["remote-file", "wsl-file"],
        value_hint = clap::ValueHint::FilePath
    )]
    pub file: Option<String>,

    /// Thư mục làm việc (working directory)
    #[arg(
        short = 'd',
        long = "dir",
        visible_aliases = ["cwd", "working-dir", "remote-dir", "wsl-dir", "wsl-cwd"],
        value_hint = clap::ValueHint::DirPath
    )]
    pub working_dir: Option<String>,

    /// Kết nối môi trường Remote (ví dụ: "Ubuntu", "wsl:Ubuntu")
    #[arg(
        long = "remote",
        visible_aliases = ["wsl", "remote-wsl", "wsl-distro"]
    )]
    pub remote: Option<String>,

    /// Truy vấn lọc log ban đầu (ví dụ: -q "level:error")
    #[arg(
        short = 'q',
        long = "query",
        visible_aliases = ["filter"]
    )]
    pub query: Option<String>,

    /// Số dòng log hiển thị tối đa trong viewport
    #[arg(short = 'n', long = "limit", default_value_t = 5000)]
    pub display_limit: usize,

    /// Dung lượng bộ đệm RingBuffer (số dòng log tối đa trong RAM)
    #[arg(
        short = 'C',
        long = "capacity",
        visible_alias = "cap",
        default_value_t = 200_000
    )]
    pub capacity: usize,

    /// Command arguments truyền sau `--` (ví dụ: `uwu-gui -- cargo run --bin server`)
    #[arg(last = true)]
    pub trailing_cmd: Vec<String>,
}

impl CliArgs {
    /// Phân giải vị trí làm việc và cấu hình nguồn chạy (cmd, file, hoặc thư mục) từ các cờ dòng lệnh
    pub fn resolve_target(&self) -> (WorkspaceLocation, SourceType, String, String, bool) {
        let mut cmd = self.cmd.clone().unwrap_or_default();
        if cmd.is_empty() && !self.trailing_cmd.is_empty() {
            cmd = self.trailing_cmd.join(" ");
        }

        let mut file = self.file.clone().unwrap_or_default();
        let mut dir = self.working_dir.clone().unwrap_or_default();
        let mut source_type = SourceType::Process;
        let mut has_custom_source = false;

        if !cmd.is_empty() {
            source_type = SourceType::Process;
            has_custom_source = true;
        } else if !file.is_empty() {
            source_type = SourceType::File;
            has_custom_source = true;
        } else if let Some(ref path_str) = self.path {
            let is_file = if self.remote.is_some() {
                std::path::Path::new(path_str).extension().is_some()
            } else {
                !std::path::Path::new(path_str).is_dir()
            };

            if is_file {
                file = path_str.clone();
                source_type = SourceType::File;
                has_custom_source = true;
            } else {
                dir = path_str.clone();
            }
        }

        // Chuẩn hóa thư mục làm việc nếu ở môi trường local
        if !dir.is_empty() && self.remote.is_none() {
            let p = std::path::Path::new(&dir);
            if let Ok(canon) = p.canonicalize() {
                dir = uwu_core_workspace::clean_path(&canon.to_string_lossy());
            } else {
                dir = uwu_core_workspace::clean_path(&dir);
            }
        }

        let location = if let Some(ref rem) = self.remote {
            let remote_dir = if !dir.is_empty() {
                dir
            } else {
                "/home".to_string()
            };
            WorkspaceLocation::remote(RemoteConnectionOptions::parse(rem, remote_dir))
        } else {
            let local_dir = if !dir.is_empty() {
                dir
            } else {
                std::env::current_dir()
                    .map(|d| uwu_core_workspace::clean_path(&d.to_string_lossy()))
                    .unwrap_or_default()
            };
            WorkspaceLocation::local(local_dir)
        };

        (location, source_type, cmd, file, has_custom_source)
    }
}
