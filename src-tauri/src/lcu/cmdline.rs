//! 进程扫描：定位 LeagueClient 凭据（对齐 Go internal/lcu/cmdline.go）。
//! 通道 A：lockfile（注册表 Location / 用户配置目录 + PID 存活）。
//! 通道 B：LeagueClientUx 命令行 --app-port / --remoting-auth-token（国服/WeGame 主路径）。

use std::sync::Mutex;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use super::lockfile::{lockfile_candidates, parse_cmdline_overrides, parse_lockfile, pid_alive};
use super::types::Credentials;

/// 大区标识缓存（按 PID，避免每轮全量扫 Ux）。
static PLAT_CACHE: Mutex<Option<(i32, String)>> = Mutex::new(None);

/// 扫描系统，找到 LCU 凭据。
/// 优先级：lockfile 通道 > LeagueClientUx 命令行通道。
pub fn detect_credentials(client_path: &str) -> Option<Credentials> {
    if let Some(c) = detect_from_lockfile(client_path) {
        log::info!("[lcu] lockfile channel hit pid={} port={}", c.pid, c.port);
        return Some(c);
    }
    if let Some(c) = detect_from_ux_cmdline() {
        log::info!(
            "[lcu] cmdline channel hit pid={} port={} platform={}",
            c.pid,
            c.port,
            c.platform_id
        );
        return Some(c);
    }
    None
}

/// 通道 A：候选 lockfile 路径解析 + PID 存活校验（对齐 Go readLockfile）。
/// 国服 lockfile 常为空文件——parse 失败即跳过，不视为命中。
fn detect_from_lockfile(client_path: &str) -> Option<Credentials> {
    for path in lockfile_candidates(client_path) {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(mut c) = parse_lockfile(&content) else {
            continue;
        };
        if !pid_alive(c.pid) {
            // 残留 lockfile：跳过
            continue;
        }
        c.platform_id = platform_id_for_pid(c.pid);
        return Some(c);
    }
    None
}

/// 通道 B：枚举 LeagueClientUx，命令行提取凭据（对齐 Go ScanLeagueClientUx）。
/// sysinfo 的 cmd 在国服常为空 → 优先 NT API `ProcessCommandLineInformation`。
fn detect_from_ux_cmdline() -> Option<Credentials> {
    for (pid, cmd) in enumerate_ux_cmdlines() {
        if let Some((port, token, platform)) = parse_cmdline_overrides(&cmd) {
            let platform_id = platform.unwrap_or_else(|| platform_id_for_pid(pid));
            return Some(Credentials {
                pid,
                port,
                token,
                platform_id,
            });
        }
    }
    None
}

/// 枚举 LeagueClientUx 的 (pid, cmdline)；cmd 走 sysinfo → NT API 回退。
fn enumerate_ux_cmdlines() -> Vec<(i32, String)> {
    let mut out = Vec::new();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );

    for (pid, proc_) in sys.processes() {
        let name = proc_.name().to_string_lossy().to_lowercase();
        if name != "leagueclientux.exe" && name != "leagueclientux" {
            continue;
        }
        let pid_i = pid.as_u32() as i32;
        let mut cmd = cmd_to_string(proc_.cmd());
        if cmd.is_empty() {
            #[cfg(windows)]
            {
                cmd = super::win_cmdline::read_command_line(pid.as_u32()).unwrap_or_default();
            }
        }
        if !cmd.is_empty() {
            out.push((pid_i, cmd));
        }
    }
    out
}

/// 从 Ux 命令行提取 `--rso_platform_id`（带 PID 缓存，对齐 Go platformIDForPID）。
fn platform_id_for_pid(pid: i32) -> String {
    if let Ok(cache) = PLAT_CACHE.lock() {
        if let Some((p, id)) = cache.as_ref() {
            if *p == pid && !id.is_empty() {
                return id.clone();
            }
        }
    }

    let id = scan_platform_id().unwrap_or_default();
    if !id.is_empty() {
        if let Ok(mut cache) = PLAT_CACHE.lock() {
            *cache = Some((pid, id.clone()));
        }
    }
    id
}

fn scan_platform_id() -> Option<String> {
    for (_, cmd) in enumerate_ux_cmdlines() {
        if let Some((_, _, Some(p))) = parse_cmdline_overrides(&cmd) {
            return Some(p);
        }
        if let Some(idx) = cmd.find("--rso_platform_id=") {
            let rest = &cmd[idx + "--rso_platform_id=".len()..];
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let v = rest[..end].trim_matches('"');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn cmd_to_string(cmd: &[std::ffi::OsString]) -> String {
    cmd.iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 单测辅助：清空平台缓存。
#[cfg(test)]
pub fn clear_platform_cache() {
    if let Ok(mut c) = PLAT_CACHE.lock() {
        *c = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机冒烟：Ux 进程在跑时必须命中；未跑则跳过。
    #[test]
    fn detect_live_when_client_running() {
        let ux_alive = {
            let mut sys = System::new();
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing(),
            );
            sys.processes().values().any(|p| {
                let n = p.name().to_string_lossy().to_lowercase();
                n == "leagueclientux.exe" || n == "leagueclientux"
            })
        };
        if !ux_alive {
            eprintln!("skip: LeagueClientUx not running");
            return;
        }
        let c = detect_credentials("").expect("Ux running must detect credentials");
        assert!(c.port > 0);
        assert!(!c.token.is_empty());
        assert!(c.pid > 0);
        eprintln!(
            "HIT pid={} port={} platform={}",
            c.pid, c.port, c.platform_id
        );
    }

    #[cfg(windows)]
    #[test]
    fn nt_reads_own_cmdline() {
        let s =
            super::super::win_cmdline::read_command_line(std::process::id()).expect("nt cmdline");
        assert!(!s.is_empty());
    }
}
