//! lockfile 解析与注册表发现（对齐 Go internal/lcu/lockfile.go）。

use std::path::PathBuf;

use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

use super::types::Credentials;

/// 解析 LeagueClient lockfile：`name:pid:port:password[:protocol]`。
/// password 可能含 `:`——按前 3 段 + 可选末段 protocol 切分（对齐 Go ParseLockfile）。
pub fn parse_lockfile(content: &str) -> Option<Credentials> {
    let s = content.trim();
    let mut head = s.splitn(4, ':');
    let name = head.next()?;
    let pid: i32 = head.next()?.trim().parse().ok()?;
    let port: u16 = head.next()?.trim().parse().ok()?;
    let rest = head.next()?;
    if name.is_empty() || pid <= 0 || port == 0 {
        return None;
    }

    // 可选末段 protocol（http/https/ws/wss）
    let mut password = rest;
    let mut protocol = "";
    if let Some(i) = rest.rfind(':') {
        let maybe = &rest[i + 1..];
        if !maybe.is_empty() && !maybe.contains('/') && !maybe.contains('\\') {
            let lower = maybe.to_ascii_lowercase();
            if lower == "http" || lower == "https" || lower == "ws" || lower == "wss" {
                password = &rest[..i];
                protocol = maybe;
            }
        }
    }
    let _ = protocol;
    if password.is_empty() {
        return None;
    }

    Some(Credentials {
        pid,
        port,
        token: password.to_string(),
        platform_id: String::new(),
    })
}

/// 从命令行提取 --app-port / --remoting-auth-token / platform（cmdline 覆盖 lockfile）。
/// platform 兼容 `--rso_platform_id` 与 `--platform-id`（Go 用 rso，旧通道用 platform-id）。
pub fn parse_cmdline_overrides(cmdline: &str) -> Option<(u16, String, Option<String>)> {
    let port = extract_arg(cmdline, "--app-port=")?.parse::<u16>().ok()?;
    let token = extract_arg_quoted(cmdline, "--remoting-auth-token=")?.to_string();
    if token.is_empty() {
        return None;
    }
    let platform = extract_arg_quoted(cmdline, "--rso_platform_id=")
        .map(|s| s.to_string())
        .or_else(|| extract_arg_quoted(cmdline, "--platform-id=").map(|s| s.to_string()));
    Some((port, token, platform))
}

fn extract_arg<'a>(cmdline: &'a str, prefix: &str) -> Option<&'a str> {
    if let Some(idx) = cmdline.find(prefix) {
        let rest = &cmdline[idx + prefix.len()..];
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let val = rest[..end].trim_matches('"');
        if !val.is_empty() {
            return Some(val);
        }
    }
    let key = prefix.trim_end_matches('=');
    if let Some(idx) = cmdline.find(key) {
        let rest = cmdline[idx + key.len()..].trim_start();
        if rest.is_empty() {
            return None;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let val = rest[..end].trim_matches('"');
        if !val.is_empty() {
            return Some(val);
        }
    }
    None
}

/// 值允许紧跟引号（`--remoting-auth-token="xxx"`），去引号后返回。
fn extract_arg_quoted<'a>(cmdline: &'a str, prefix: &str) -> Option<&'a str> {
    if let Some(idx) = cmdline.find(prefix) {
        let rest = &cmdline[idx + prefix.len()..];
        let rest = rest.trim_start_matches('"');
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '"')
            .unwrap_or(rest.len());
        let val = &rest[..end];
        if !val.is_empty() {
            return Some(val);
        }
    }
    None
}

/// 候选 lockfile 路径：
/// 注册表 Location 及其父目录（对齐 Go LockfilePaths）+ 用户配置的客户端目录。
pub fn lockfile_candidates(client_path: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = registry_locations();
    if !client_path.is_empty() {
        dirs.push(PathBuf::from(client_path));
    }
    let mut paths = Vec::new();
    for d in dirs {
        let d = normalize_dir(&d);
        paths.push(d.join("lockfile"));
        if let Some(parent) = d.parent() {
            if parent != d.as_path() {
                paths.push(parent.join("lockfile"));
            }
        }
    }
    paths
}

/// 注册表查询的 League of Legends 安装 Location（对齐 Go registryKeyPaths / Location）。
fn registry_locations() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in [
        r"SOFTWARE\WOW6432Node\Riot Games, Inc.\League of Legends",
        r"SOFTWARE\Riot Games, Inc.\League of Legends",
        r"Software\Riot Games\LeagueClient",
        r"Software\Riot Games\League of Legends",
    ] {
        if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(sub) {
            for val in ["Location", "Path", "InstallLocation"] {
                if let Ok(path) = key.get_value::<String, _>(val) {
                    if !path.is_empty() {
                        out.push(PathBuf::from(path));
                        break;
                    }
                }
            }
        }
    }
    out
}

fn normalize_dir(p: &std::path::Path) -> PathBuf {
    // 去尾部分隔符；路径存在性由调用方 lockfile.exists 控制
    let s = p.to_string_lossy();
    let trimmed = s.trim_end_matches(['\\', '/']);
    PathBuf::from(trimmed)
}

/// 进程存活检查（对齐 Go PidAlive）。
pub fn pid_alive(pid: i32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
    if pid <= 0 {
        return false;
    }
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
    );
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.process(Pid::from_u32(pid as u32)).is_some()
}

/// 路径白名单判定（对齐 Go AllowedPath：剥离 query、拒穿越、path clean 后前缀匹配）。
pub fn allowed_path(path: &str) -> bool {
    let p = match path.find('?') {
        Some(i) => &path[..i],
        None => path,
    };
    if p.is_empty() || !p.starts_with('/') {
        return false;
    }
    // 拒绝编码穿越与反斜杠
    if p.contains("%2e") || p.contains("%2E") || p.contains('\\') {
        return false;
    }
    let clean = path_clean(p);
    if clean.is_empty() || !clean.starts_with('/') || clean.contains("..") {
        return false;
    }
    super::endpoints::PATH_PREFIX_ALLOWLIST
        .iter()
        .any(|prefix| clean.starts_with(prefix))
}

/// 规范化 URL 路径（等价 path.Clean，POSIX 语义）。
fn path_clean(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    format!("/{}", out.join("/"))
}

/// 构造 Authorization 头值（LCU: `Basic base64(riot:token)`）。
pub fn auth_header(token: &str) -> String {
    use base64::Engine;
    let raw = format!("riot:{token}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lockfile_valid() {
        let c = parse_lockfile("LeagueClient:1234:5678:AbCdEf:https").unwrap();
        assert_eq!(c.pid, 1234);
        assert_eq!(c.port, 5678);
        assert_eq!(c.token, "AbCdEf");
    }

    #[test]
    fn parse_lockfile_password_with_colon_and_protocol() {
        let c = parse_lockfile("LeagueClient:1:51234:tok:en:https").unwrap();
        assert_eq!(c.pid, 1);
        assert_eq!(c.port, 51234);
        assert_eq!(c.token, "tok:en");
    }

    #[test]
    fn parse_lockfile_invalid() {
        assert!(parse_lockfile("too:few").is_none());
        assert!(parse_lockfile("a:notanumber:2:tok:https").is_none());
        assert!(parse_lockfile("a:1:notaport:tok:https").is_none());
        assert!(parse_lockfile("a:0:2:tok:https").is_none());
        assert!(parse_lockfile("a:1:0:tok:https").is_none());
        assert!(parse_lockfile("a:1:2::https").is_none());
        assert!(parse_lockfile("").is_none());
        assert!(parse_lockfile("   ").is_none());
    }

    #[test]
    fn cmdline_overrides() {
        let line =
            r#"LeagueClient.exe --app-port=51234 --remoting-auth-token=XYZ --platform-id=HN1"#;
        let (port, token, platform) = parse_cmdline_overrides(line).unwrap();
        assert_eq!(port, 51234);
        assert_eq!(token, "XYZ");
        assert_eq!(platform.as_deref(), Some("HN1"));
    }

    #[test]
    fn cmdline_rso_platform_and_quoted_token() {
        let line = r#"LeagueClientUx.exe --app-port=9999 --remoting-auth-token="ab-cd_ef" --rso_platform_id=HN2"#;
        let (port, token, platform) = parse_cmdline_overrides(line).unwrap();
        assert_eq!(port, 9999);
        assert_eq!(token, "ab-cd_ef");
        assert_eq!(platform.as_deref(), Some("HN2"));
    }

    #[test]
    fn cmdline_missing() {
        assert!(parse_cmdline_overrides("LeagueClient.exe --foo=bar").is_none());
        assert!(parse_cmdline_overrides("").is_none());
    }

    #[test]
    fn allowed_paths() {
        assert!(allowed_path("/system/v1/builds"));
        assert!(allowed_path("/lol-gameflow/v1/gameflow-phase"));
        assert!(allowed_path("/lol-summoner/v1/current-summoner"));
        assert!(allowed_path(
            "/lol-match-history/v1/products/lol/abc/matches?begIndex=0&endIndex=20"
        ));
        assert!(allowed_path("/lol-ranked/v1/current-ranked-stats"));
        assert!(allowed_path("/lol-champ-select/v1/session"));
        assert!(allowed_path(
            "/lol-game-data/assets/v1/champion-icons/1.png"
        ));
        assert!(allowed_path("/fe/lol-loot/augment_7018.png"));
        assert!(!allowed_path("/lol-chat/v1/me"));
        assert!(!allowed_path("/lol-honor-v2/v1/ballot"));
        assert!(!allowed_path("/internal/debug"));
        assert!(!allowed_path("/lol-login/v1/session"));
        assert!(!allowed_path("/riotclient/ux-commands"));
        assert!(!allowed_path("lol-summoner/v1/current-summoner"));
        assert!(!allowed_path(""));
        assert!(!allowed_path("/lol-game-data/assets/../../../etc/passwd"));
        assert!(!allowed_path("/lol-game-data/assets/..%2f..%2f"));
        assert!(!allowed_path(r"/lol-game-data\assets\1.png"));
        assert!(!allowed_path("/lol-gameflow-malicious"));
    }

    #[test]
    fn auth_header_format() {
        let h = auth_header("tok");
        assert!(h.starts_with("Basic "));
        // base64("riot:tok") = cmlvdDp0b2s=
        assert_eq!(h, "Basic cmlvdDp0b2s=");
    }

    #[test]
    fn lockfile_candidates_include_client_path() {
        let list = lockfile_candidates(r"E:\fake\LeagueClient");
        assert!(
            list.iter().any(|p| p.ends_with("LeagueClient\\lockfile")
                || p.ends_with("LeagueClient/lockfile")
                || p.to_string_lossy().contains("fake")),
            "got {list:?}"
        );
        // 父目录候选
        assert!(list.iter().any(|p| p.to_string_lossy().contains("fake")));
        assert!(list.iter().any(|p| p.ends_with("lockfile")));
    }
}
