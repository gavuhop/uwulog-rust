use super::docker::{normalize_mount_path, DockerContainerInfo, DockerTransport};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Kết quả khi mở một thư mục local chứa `.devcontainer/devcontainer.json`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedDevContainer {
    pub info: DockerContainerInfo,
    pub remote_user: Option<String>,
    pub workspace_folder: String,
}

/// Cấu trúc cấu hình `.devcontainer/devcontainer.json` (hỗ trợ JSONC theo Dev Containers Spec & Zed)
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DevContainerConfig {
    pub workspace_folder: Option<String>,
    pub remote_user: Option<String>,
    pub container_user: Option<String>,
}

impl DevContainerConfig {
    /// Đọc và phân tích file `devcontainer.json` (hỗ trợ comments `//`, `/* */` và trailing comma)
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read devcontainer config at {}", path.display()))?;
        Self::from_jsonc_str(&raw)
            .with_context(|| format!("Invalid devcontainer.json at {}", path.display()))
    }

    /// Phân tích chuỗi JSONC của `devcontainer.json`
    pub fn from_jsonc_str(input: &str) -> Result<Self> {
        let clean_json = strip_jsonc(input);
        let cfg: Self = serde_json::from_str(&clean_json)?;
        Ok(cfg)
    }

    pub fn effective_user(&self) -> Option<&str> {
        self.remote_user
            .as_deref()
            .or(self.container_user.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// Đường dẫn workspace bên trong container (mặc định `/workspaces/<folder-name>` giống Zed & VS Code)
    pub fn effective_workspace_folder(&self, local_folder: &Path) -> String {
        if let Some(ref wf) = self.workspace_folder {
            let trimmed = wf.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        let folder_name = local_folder
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("workspace");
        format!("/workspaces/{}", folder_name)
    }
}

/// Tìm file cấu hình devcontainer trong một thư mục local theo chuẩn Dev Containers Spec
pub fn find_devcontainer_config(local_folder: &Path) -> Option<PathBuf> {
    let primary = local_folder.join(".devcontainer").join("devcontainer.json");
    if primary.is_file() {
        return Some(primary);
    }
    let root_dot = local_folder.join(".devcontainer.json");
    if root_dot.is_file() {
        return Some(root_dot);
    }

    let dc_dir = local_folder.join(".devcontainer");
    if let Ok(entries) = std::fs::read_dir(&dc_dir) {
        let mut sub_configs: Vec<PathBuf> = entries
            .flatten()
            .filter_map(|entry| {
                let candidate = entry.path().join("devcontainer.json");
                if candidate.is_file() {
                    Some(candidate)
                } else {
                    None
                }
            })
            .collect();
        sub_configs.sort();
        if let Some(first) = sub_configs.into_iter().next() {
            return Some(first);
        }
    }

    None
}

/// Loại bỏ comments (`//`, `/* */`) và trailing commas khỏi JSONC để parse bằng `serde_json`
pub(crate) fn strip_jsonc(input: &str) -> String {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut no_comments = String::with_capacity(len);
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < len {
        let ch = bytes[i] as char;
        if in_string {
            no_comments.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            no_comments.push(ch);
            i += 1;
            continue;
        }

        if ch == '/' && i + 1 < len {
            let next = bytes[i + 1] as char;
            if next == '/' {
                i += 2;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            } else if next == '*' {
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    if bytes[i] == b'\n' {
                        no_comments.push('\n');
                    }
                    i += 1;
                }
                i = (i + 2).min(len);
                continue;
            }
        }

        no_comments.push(ch);
        i += 1;
    }

    // Bước 2: bỏ trailing comma ngay trước `}` hoặc `]` (ngoài string)
    let chars: Vec<char> = no_comments.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut idx = 0;
    in_string = false;
    escaped = false;

    while idx < chars.len() {
        let ch = chars[idx];
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            idx += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            idx += 1;
            continue;
        }

        if ch == ',' {
            let mut lookahead = idx + 1;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if lookahead < chars.len() && (chars[lookahead] == '}' || chars[lookahead] == ']') {
                idx += 1;
                continue;
            }
        }

        out.push(ch);
        idx += 1;
    }

    out
}

fn resolve_devcontainer_workspace_folder(
    container_name: &str,
    local_folder: &Path,
    config: &DevContainerConfig,
    remote_user: Option<&str>,
) -> String {
    if let Some(ref explicit) = config.workspace_folder {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let local_str = local_folder.to_string_lossy();
    if let Some(mounted_dst) =
        DockerTransport::resolve_mount_destination(container_name, &local_str)
    {
        return mounted_dst;
    }

    let default_ws = config.effective_workspace_folder(local_folder);
    if DockerTransport::directory_exists(container_name, &default_ws, remote_user) {
        return default_ws;
    }

    let workdir = DockerTransport::resolve_default_workdir(container_name, remote_user);
    if workdir != "/" {
        workdir
    } else {
        default_ws
    }
}

impl DockerTransport {
    /// Tìm container đang chạy tương ứng với `local_folder` (so khớp label `devcontainer.local_folder`)
    pub async fn find_container_by_local_folder(
        local_folder: &Path,
    ) -> Result<Option<DockerContainerInfo>> {
        let canonical = local_folder
            .canonicalize()
            .unwrap_or_else(|_| local_folder.to_path_buf());
        let target_norm = normalize_mount_path(&canonical.to_string_lossy());
        let raw_norm = normalize_mount_path(&local_folder.to_string_lossy());

        let containers = Self::list_running_containers().await?;
        Ok(containers.into_iter().find(|c| {
            c.local_folder.as_deref().is_some_and(|lf| {
                let lf_norm = normalize_mount_path(lf);
                lf_norm == target_norm || lf_norm == raw_norm
            })
        }))
    }

    /// Kiểm tra nhanh thư mục local có file `.devcontainer` VÀ có Dev Container đang chạy hay không
    pub async fn detect_running_devcontainer_for_local_folder(
        local_folder: &Path,
    ) -> Option<OpenedDevContainer> {
        let config_path = find_devcontainer_config(local_folder)?;
        let config = DevContainerConfig::from_file(&config_path).ok()?;
        let existing = Self::find_container_by_local_folder(local_folder)
            .await
            .ok()??;
        let default_user = config.effective_user().map(ToString::to_string);
        let workspace_folder = resolve_devcontainer_workspace_folder(
            &existing.name,
            local_folder,
            &config,
            default_user.as_deref(),
        );
        Some(OpenedDevContainer {
            info: existing,
            remote_user: default_user,
            workspace_folder,
        })
    }

    /// Mở Dev Container từ một thư mục local:
    /// Bắt buộc thư mục phải có file `.devcontainer` VÀ Dev Container tương ứng đã đang chạy;
    /// nếu thiếu một trong hai sẽ trả lỗi rõ ràng (không tự ý start/build container mới).
    pub async fn open_devcontainer_folder(local_folder: &Path) -> Result<OpenedDevContainer> {
        let canonical_folder = local_folder
            .canonicalize()
            .unwrap_or_else(|_| local_folder.to_path_buf());
        let folder_str = canonical_folder
            .to_string_lossy()
            .strip_prefix(r"\\?\")
            .unwrap_or(&canonical_folder.to_string_lossy())
            .to_string();
        let clean_folder_path = PathBuf::from(&folder_str);

        let config_path = find_devcontainer_config(&clean_folder_path).with_context(|| {
            format!(
                "No .devcontainer/devcontainer.json or .devcontainer.json found in `{}`.",
                clean_folder_path.display()
            )
        })?;
        let config = DevContainerConfig::from_file(&config_path)?;
        let default_user = config.effective_user().map(ToString::to_string);

        let existing = Self::find_container_by_local_folder(&clean_folder_path)
            .await?
            .with_context(|| {
                format!(
                    "No running Dev Container found for `{}`. Start the Dev Container first, then try again.",
                    clean_folder_path.display()
                )
            })?;

        let workspace_folder = resolve_devcontainer_workspace_folder(
            &existing.name,
            &clean_folder_path,
            &config,
            default_user.as_deref(),
        );

        Ok(OpenedDevContainer {
            info: existing,
            remote_user: default_user,
            workspace_folder,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_jsonc_comments_and_trailing_commas() {
        let jsonc = r#"{
            // Line comment with http://example.com inside
            "name": "Rust // not a comment",
            /* Block
               comment */
            "workspaceFolder": "/workspaces/rust-app",
            "remoteUser": "vscode",
            "runArgs": [
                "--init",
                "--cap-add=SYS_PTRACE",
            ],
        }"#;

        let cfg = DevContainerConfig::from_jsonc_str(jsonc).unwrap();
        assert_eq!(
            cfg.workspace_folder.as_deref(),
            Some("/workspaces/rust-app")
        );
        assert_eq!(cfg.remote_user.as_deref(), Some("vscode"));
    }

    #[test]
    fn test_devcontainer_config_effective_defaults() {
        let cfg = DevContainerConfig::from_jsonc_str(
            r#"{
                "name": "API",
                "remoteUser": "vscode"
            }"#,
        )
        .unwrap();

        assert_eq!(cfg.effective_user(), Some("vscode"));
        assert_eq!(
            cfg.effective_workspace_folder(Path::new("/home/user/my-api")),
            "/workspaces/my-api"
        );
    }

    #[test]
    fn test_find_devcontainer_config_variants() {
        let tmp = std::env::temp_dir().join(format!(
            "uwu_dc_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dc_dir = tmp.join(".devcontainer");
        std::fs::create_dir_all(&dc_dir).unwrap();
        let cfg_path = dc_dir.join("devcontainer.json");
        std::fs::write(&cfg_path, r#"{ "image": "alpine:3.19" }"#).unwrap();

        assert_eq!(find_devcontainer_config(&tmp), Some(cfg_path));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_open_devcontainer_folder_fails_without_config() {
        let tmp = std::env::temp_dir().join(format!(
            "uwu_dc_missing_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let err = DockerTransport::open_devcontainer_folder(&tmp)
            .await
            .unwrap_err();
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("devcontainer.json"),
            "Expected missing devcontainer.json error, got: {}",
            msg
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
