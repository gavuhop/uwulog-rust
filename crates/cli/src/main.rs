use anyhow::{Context, Result};
use clap::Parser;
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Parser, Debug)]
#[command(
    name = "uwulog",
    about = "🚀 Uwu Log Viewer - Fast, modern log viewer & workspace monitor",
    version = env!("CARGO_PKG_VERSION"),
    after_help = "Examples:
  uwulog                               Open GUI log viewer
  uwulog ./app.log                     Tail and view log file in GUI
  uwulog -f /var/log/syslog            Tail specific log file
  uwulog -c \"cargo run\"                Stream logs from command in GUI
  uwulog --remote Ubuntu -f app.log    Open log file in WSL distro
  uwulog --tui                         Open log viewer in Terminal UI (TUI)
  uwulog --wait ./app.log              Wait for GUI window to close before returning"
)]
pub struct CliArgs {
    /// Path to log file or project directory to open
    #[arg(value_name = "PATH", value_hint = clap::ValueHint::AnyPath)]
    pub path: Option<String>,

    /// Command to stream logs from (e.g. -c "cargo run")
    #[arg(
        short = 'c',
        short_alias = 'r',
        long = "cmd",
        visible_aliases = ["run", "command", "exec", "remote-cmd", "wsl-cmd"]
    )]
    pub cmd: Option<String>,

    /// Log file to follow/tail
    #[arg(
        short = 'f',
        long = "file",
        visible_aliases = ["remote-file", "wsl-file"],
        value_hint = clap::ValueHint::FilePath
    )]
    pub file: Option<String>,

    /// Working directory
    #[arg(
        short = 'd',
        long = "dir",
        visible_aliases = ["cwd", "working-dir", "remote-dir", "wsl-dir", "wsl-cwd"],
        value_hint = clap::ValueHint::DirPath
    )]
    pub working_dir: Option<String>,

    /// Remote target environment (e.g. "Ubuntu", "wsl:Ubuntu")
    #[arg(
        short = 'w',
        long = "remote",
        visible_aliases = ["wsl", "remote-wsl", "wsl-distro"]
    )]
    pub remote: Option<String>,

    /// Initial search query or filter (e.g. -q "level:error")
    #[arg(short = 'q', long = "query", visible_aliases = ["filter"])]
    pub query: Option<String>,

    /// Maximum lines displayed in viewport
    #[arg(short = 'n', long = "limit")]
    pub display_limit: Option<usize>,

    /// Buffer capacity (maximum lines in memory)
    #[arg(short = 'C', long = "capacity", visible_alias = "cap")]
    pub capacity: Option<usize>,

    /// Launch in Terminal UI (TUI) mode instead of GUI
    #[arg(short = 't', long = "tui")]
    pub tui: bool,

    /// Wait for the viewer process to exit before returning
    #[arg(long = "wait")]
    pub wait: bool,

    /// Trailing command arguments passed after `--`
    #[arg(last = true)]
    pub trailing_cmd: Vec<String>,
}

/// Find a companion executable (e.g. uwu-gui or uwu-tui)
pub fn find_companion_binary(binary_name: &str) -> PathBuf {
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let target_filename = format!("{}{}", binary_name, ext);

    // 1. Check next to current running executable
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let direct_sibling = parent.join(&target_filename);
            if direct_sibling.is_file() {
                return direct_sibling;
            }

            // 2. Check parent directory (e.g. exe is in {app}/bin, target is in {app})
            if let Some(grandparent) = parent.parent() {
                let parent_sibling = grandparent.join(&target_filename);
                if parent_sibling.is_file() {
                    return parent_sibling;
                }
            }
        }
    }

    // 3. Search in system PATH
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let candidate = dir.join(&target_filename);
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    // 4. Fallback to raw binary name
    PathBuf::from(target_filename)
}

fn build_forward_args(args: &CliArgs) -> Vec<String> {
    let mut forward = Vec::new();

    if let Some(ref path) = args.path {
        forward.push(path.clone());
    }
    if let Some(ref file) = args.file {
        forward.push("-f".to_string());
        forward.push(file.clone());
    }
    if let Some(ref cmd) = args.cmd {
        forward.push("-c".to_string());
        forward.push(cmd.clone());
    }
    if let Some(ref dir) = args.working_dir {
        forward.push("-d".to_string());
        forward.push(dir.clone());
    }
    if let Some(ref remote) = args.remote {
        forward.push("--remote".to_string());
        forward.push(remote.clone());
    }
    if let Some(ref query) = args.query {
        forward.push("-q".to_string());
        forward.push(query.clone());
    }
    if let Some(limit) = args.display_limit {
        forward.push("-n".to_string());
        forward.push(limit.to_string());
    }
    if let Some(cap) = args.capacity {
        forward.push("-C".to_string());
        forward.push(cap.to_string());
    }
    if !args.trailing_cmd.is_empty() {
        forward.push("--".to_string());
        forward.extend(args.trailing_cmd.clone());
    }

    forward
}

fn launch_tui(args: &CliArgs) -> Result<()> {
    let tui_bin = find_companion_binary("uwu-tui");
    let forward_args = build_forward_args(args);

    let status = Command::new(&tui_bin)
        .args(&forward_args)
        .status()
        .with_context(|| format!("Failed to launch TUI executable: {}", tui_bin.display()))?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn launch_gui(args: &CliArgs) -> Result<()> {
    let gui_bin = find_companion_binary("uwu-gui");
    let forward_args = build_forward_args(args);

    let mut command = Command::new(&gui_bin);
    command.args(&forward_args);

    if args.wait {
        let status = command
            .status()
            .with_context(|| format!("Failed to launch GUI executable: {}", gui_bin.display()))?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS = 0x00000008, CREATE_NEW_PROCESS_GROUP = 0x00000200
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        command
            .spawn()
            .with_context(|| format!("Failed to spawn GUI executable: {}", gui_bin.display()))?;
    }

    #[cfg(not(windows))]
    {
        command
            .spawn()
            .with_context(|| format!("Failed to spawn GUI executable: {}", gui_bin.display()))?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    if args.tui {
        launch_tui(&args)
    } else {
        launch_gui(&args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_args_parsing_defaults() {
        let args = CliArgs::parse_from(["uwulog"]);
        assert!(args.path.is_none());
        assert!(!args.tui);
        assert!(!args.wait);
    }

    #[test]
    fn test_cli_args_parsing_file_and_flags() {
        let args = CliArgs::parse_from([
            "uwulog",
            "-f",
            "/var/log/nginx.log",
            "-q",
            "error",
            "--remote",
            "Ubuntu",
            "--tui",
            "--wait",
        ]);
        assert_eq!(args.file.as_deref(), Some("/var/log/nginx.log"));
        assert_eq!(args.query.as_deref(), Some("error"));
        assert_eq!(args.remote.as_deref(), Some("Ubuntu"));
        assert!(args.tui);
        assert!(args.wait);
    }

    #[test]
    fn test_build_forward_args() {
        let args = CliArgs {
            path: Some("my_app.log".to_string()),
            cmd: Some("cargo run".to_string()),
            file: None,
            working_dir: Some("/tmp".to_string()),
            remote: Some("Ubuntu".to_string()),
            query: Some("level:err".to_string()),
            display_limit: Some(1000),
            capacity: Some(50000),
            tui: false,
            wait: false,
            trailing_cmd: vec!["--flag".to_string(), "val".to_string()],
        };

        let forwarded = build_forward_args(&args);
        assert_eq!(
            forwarded,
            vec![
                "my_app.log",
                "-c",
                "cargo run",
                "-d",
                "/tmp",
                "--remote",
                "Ubuntu",
                "-q",
                "level:err",
                "-n",
                "1000",
                "-C",
                "50000",
                "--",
                "--flag",
                "val"
            ]
        );
    }

    #[test]
    fn test_find_companion_binary() {
        let bin = find_companion_binary("uwu-gui");
        assert!(bin.to_string_lossy().contains("uwu-gui"));
    }
}
