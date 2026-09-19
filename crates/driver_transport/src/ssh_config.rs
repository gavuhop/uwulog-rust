use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const FILTERED_GIT_PROVIDER_HOSTNAMES: &[&str] = &[
    "dev.azure.com",
    "bitbucket.org",
    "chromium.googlesource.com",
    "codeberg.org",
    "gitea.com",
    "gitee.com",
    "github.com",
    "gist.github.com",
    "gitlab.com",
    "sourcehut.org",
    "git.sr.ht",
];

/// Phân tích nội dung file ~/.ssh/config để lấy danh sách host khả dụng (theo chuẩn Zed)
pub fn parse_ssh_config_hosts(config: &str) -> BTreeSet<String> {
    parse_host_blocks(config)
        .into_iter()
        .flat_map(HostBlock::non_git_provider_hosts)
        .collect()
}

struct HostBlock {
    aliases: BTreeSet<String>,
    hostname: Option<String>,
}

impl HostBlock {
    fn non_git_provider_hosts(self) -> impl Iterator<Item = String> {
        let hostname = self.hostname;
        let hostname_ref = hostname.as_deref().map(is_git_provider_domain);
        self.aliases
            .into_iter()
            .filter(move |alias| !hostname_ref.unwrap_or_else(|| is_git_provider_domain(alias)))
    }
}

fn parse_host_blocks(config: &str) -> Vec<HostBlock> {
    let mut blocks = Vec::new();
    let mut aliases = BTreeSet::new();
    let mut hostname = None;
    let mut needs_continuation = false;

    for line in config.lines() {
        let line = line.trim_start();

        if needs_continuation {
            needs_continuation = line.trim_end().ends_with('\\');
            parse_hosts(line, &mut aliases);
            continue;
        }

        let Some((keyword, value)) = split_keyword_and_value(line) else {
            continue;
        };

        if keyword.eq_ignore_ascii_case("host") {
            if !aliases.is_empty() {
                blocks.push(HostBlock { aliases, hostname });
                aliases = BTreeSet::new();
                hostname = None;
            }
            parse_hosts(value, &mut aliases);
            needs_continuation = line.trim_end().ends_with('\\');
        } else if keyword.eq_ignore_ascii_case("hostname") {
            hostname = value.split_whitespace().next().map(ToOwned::to_owned);
        }
    }

    if !aliases.is_empty() {
        blocks.push(HostBlock { aliases, hostname });
    }

    blocks
}

fn parse_hosts(line: &str, hosts: &mut BTreeSet<String>) {
    hosts.extend(
        line.split_whitespace()
            .map(|field| field.trim_end_matches('\\'))
            .filter(|field| !field.starts_with('!'))
            .filter(|field| !field.contains('*'))
            .filter(|field| !field.contains('?'))
            .filter(|field| *field != "\\")
            .filter(|field| !field.is_empty())
            .map(|field| field.to_owned()),
    );
}

fn split_keyword_and_value(line: &str) -> Option<(&str, &str)> {
    if line.starts_with('#') {
        return None;
    }
    let keyword_end = line.find(char::is_whitespace).unwrap_or(line.len());
    let keyword = &line[..keyword_end];
    if keyword.is_empty() {
        return None;
    }

    let value = line[keyword_end..].trim_start();
    Some((keyword, value))
}

fn is_git_provider_domain(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    FILTERED_GIT_PROVIDER_HOSTNAMES.contains(&host.as_str())
}

/// Lấy đường dẫn file cấu hình ~/.ssh/config của user
pub fn user_ssh_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(userprofile).join(".ssh").join("config");
            if p.exists() {
                return Some(p);
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".ssh").join("config");
        if p.exists() {
            return Some(p);
        }
    }

    None
}

/// Lấy đường dẫn file cấu hình SSH toàn hệ thống (ví dụ: /etc/ssh/ssh_config)
pub fn system_ssh_config_path() -> Option<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    {
        let p = Path::new("/etc/ssh/ssh_config");
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// Đọc và tổng hợp toàn bộ danh sách host từ ~/.ssh/config và system ssh config
pub fn load_system_and_user_ssh_hosts() -> Vec<String> {
    let mut hosts = BTreeSet::new();

    if let Some(user_path) = user_ssh_config_path() {
        if let Ok(content) = std::fs::read_to_string(user_path) {
            hosts.extend(parse_ssh_config_hosts(&content));
        }
    }

    if let Some(sys_path) = system_ssh_config_path() {
        if let Ok(content) = std::fs::read_to_string(sys_path) {
            hosts.extend(parse_ssh_config_hosts(&content));
        }
    }

    hosts.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_config_hosts_basic() {
        let config = r#"
Host *
  AddKeysToAgent yes
  IdentityFile ~/.ssh/id_ed25519

Host whatever.*
User another

Host !not_this
User not_me

Host something
  HostName whatever.tld

Host linux bsd host3
  User ubuntu

Host rpi
  user rpi
  hostname rpi.local

Host \
       somehost \
       anotherhost
  Hostname 192.168.3.3
"#;

        let hosts = parse_ssh_config_hosts(config);
        let expected = BTreeSet::from([
            "anotherhost".to_string(),
            "bsd".to_string(),
            "host3".to_string(),
            "linux".to_string(),
            "rpi".to_string(),
            "somehost".to_string(),
            "something".to_string(),
        ]);
        assert_eq!(hosts, expected);
    }

    #[test]
    fn test_filter_git_providers() {
        let config = r#"
Host github.com
  User git
  IdentityFile ~/.ssh/id_github

Host gitlab.com
  User git

Host my-vps
  HostName 123.45.67.89
  User deploy
"#;

        let hosts = parse_ssh_config_hosts(config);
        assert!(!hosts.contains("github.com"));
        assert!(!hosts.contains("gitlab.com"));
        assert!(hosts.contains("my-vps"));
    }
}
