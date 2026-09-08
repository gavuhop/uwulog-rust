use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Trạng thái nạp biến môi trường của Workspace
#[derive(Debug, Clone, PartialEq, Default)]
pub enum EnvLoadStatus {
    #[default]
    Idle,
    Loading {
        started_at: Instant,
    },
    Ready {
        source_summary: String,
        updated_at: Instant,
    },
    Failed {
        error: String,
    },
}

/// Phân tích cú pháp chuỗi cấu hình dạng `.env` (hỗ trợ comment '#', 'export KEY=VAL', nháy đơn/kép, và giá trị chứa dấu '=')
pub fn parse_dot_env(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let cleaned = if let Some(stripped) = trimmed.strip_prefix("export ") {
            stripped.trim()
        } else {
            trimmed
        };

        if let Some((k, v)) = cleaned.split_once('=') {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            let mut val = v.trim();
            // Xóa dấu nháy kép hoặc nháy đơn bọc ngoài nếu có
            if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
                || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
            {
                val = &val[1..val.len() - 1];
            }
            map.insert(key.to_string(), val.to_string());
        }
    }
    map
}

/// Nạp biến môi trường cho thư mục workspace:
/// 1. Kế thừa các biến môi trường hiện tại của hệ thống (`std::env::vars()`)
/// 2. Chuẩn hóa PATH trên Windows (`Path` -> `PATH`)
/// 3. Bổ sung các đường dẫn công cụ lập trình phổ biến (như `~/.cargo/bin`) vào PATH nếu chưa có
/// 4. Đọc file `.env` hoặc `.env.local` trong thư mục workspace (nếu tồn tại) và ghi đè/bổ sung
pub async fn load_workspace_environment(working_dir: &Path) -> Result<HashMap<String, String>> {
    let mut env_map: HashMap<String, String> = std::env::vars().collect();

    // Chuẩn hóa Path -> PATH trên Windows (giống cách Zed xử lý trong environment.rs)
    #[cfg(target_os = "windows")]
    {
        if let Some(path_val) = env_map.remove("Path") {
            env_map.insert("PATH".to_string(), path_val);
        }
    }

    // Tự động bổ sung thư mục bin phổ biến vào PATH nếu chưa có
    let home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok();
    if let Some(home) = home_dir {
        let home_path = PathBuf::from(home);
        let cargo_bin = home_path.join(".cargo").join("bin");
        if cargo_bin.exists() {
            let cargo_str = cargo_bin.to_string_lossy().to_string();
            let current_path = env_map.entry("PATH".to_string()).or_default();
            if !current_path.contains(&cargo_str) {
                #[cfg(target_os = "windows")]
                {
                    if !current_path.is_empty() && !current_path.ends_with(';') {
                        current_path.push(';');
                    }
                    current_path.push_str(&cargo_str);
                }
                #[cfg(not(target_os = "windows"))]
                {
                    if !current_path.is_empty() && !current_path.ends_with(':') {
                        current_path.push(':');
                    }
                    current_path.push_str(&cargo_str);
                }
            }
        }
    }

    // Đọc file .env hoặc .env.local trong working_dir nếu có
    if working_dir.exists() && working_dir.is_dir() {
        let dot_env_candidates = [working_dir.join(".env"), working_dir.join(".env.local")];

        for path in &dot_env_candidates {
            if path.is_file() {
                if let Ok(content) = tokio::fs::read_to_string(path).await {
                    let parsed = parse_dot_env(&content);
                    for (k, v) in parsed {
                        env_map.insert(k, v);
                    }
                }
            }
        }
    }

    Ok(env_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dot_env() {
        let sample = r#"
        # Đây là comment
        PORT=8080
        export DATABASE_URL="postgres://user:pass@localhost:5432/uwulog?sslmode=disable"
        APP_NAME='Uwu Log'
        EMPTY_VAL=
        # Dòng rỗng tiếp theo

        SPACED = with space = and extra
        "#;

        let parsed = parse_dot_env(sample);
        assert_eq!(parsed.get("PORT"), Some(&"8080".to_string()));
        assert_eq!(
            parsed.get("DATABASE_URL"),
            Some(&"postgres://user:pass@localhost:5432/uwulog?sslmode=disable".to_string())
        );
        assert_eq!(parsed.get("APP_NAME"), Some(&"Uwu Log".to_string()));
        assert_eq!(parsed.get("EMPTY_VAL"), Some(&"".to_string()));
        assert_eq!(
            parsed.get("SPACED"),
            Some(&"with space = and extra".to_string())
        );
    }

    #[tokio::test]
    async fn test_load_workspace_environment_with_dot_env() {
        let temp_dir = std::env::temp_dir().join(format!("uwu_test_env_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let env_file = temp_dir.join(".env");
        tokio::fs::write(
            &env_file,
            "TEST_UWU_KEY=uwu_super_value_123\nANOTHER_KEY=456\n",
        )
        .await
        .unwrap();

        let envs = load_workspace_environment(&temp_dir).await.unwrap();
        assert_eq!(
            envs.get("TEST_UWU_KEY"),
            Some(&"uwu_super_value_123".to_string())
        );
        assert_eq!(envs.get("ANOTHER_KEY"), Some(&"456".to_string()));
        assert!(envs.contains_key("PATH"));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[test]
    fn test_env_load_status_default() {
        let status = EnvLoadStatus::default();
        assert_eq!(status, EnvLoadStatus::Idle);
    }
}
