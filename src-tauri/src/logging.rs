//! 日志：对齐 Go internal/logging。
//! 路径：%APPDATA%\LOLAssistant\logs\app-YYYYMMDD.log，保留 7 天。
//! 文件名按本地时区取日期（对齐 Go time.Now）。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// 初始化日志（幂等）：打开当日文件并注册 `log` crate 桥接。
pub fn init() {
    let dir = logs_dir();
    let _ = fs::create_dir_all(&dir);
    prune_old(&dir, 7);
    let path = dir.join(daily_log_name());
    if let Ok(f) = OpenOptions::new().create(true).append(true).open(&path) {
        *LOG_FILE.lock().unwrap() = Some(f);
    }
    install_log_bridge();
}

fn logs_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("LOLAssistant").join("logs")
}

/// 当日日志文件名：本地时区 `app-YYYYMMDD.log`（UTC 会差一天）。
fn daily_log_name() -> String {
    chrono::Local::now().format("app-%Y%m%d.log").to_string()
}

fn prune_old(dir: &Path, keep_days: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let now = chrono::Local::now().timestamp();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("app-") || !name.ends_with(".log") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let Ok(m) = modified.duration_since(std::time::UNIX_EPOCH) else {
            continue;
        };
        if now - m.as_secs() as i64 > keep_days as i64 * 86400 {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// 追加一行日志（线程安全；无 sink 时静默）。
pub fn write_line(level: &str, msg: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(f) = guard.as_mut() {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(f, "[{ts} {level}] {msg}");
            let _ = f.flush();
        }
    }
}

/// `log` → 文件 sink（log::info!/warn! 等才能落盘）。
struct FileLogger;

impl log::Log for FileLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        write_line(record.level().as_str(), &format!("{}", record.args()));
    }

    fn flush(&self) {
        if let Ok(mut guard) = LOG_FILE.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

fn install_log_bridge() {
    // set_boxed_logger 全局仅一次；重复 init 忽略错误
    let _ = log::set_boxed_logger(Box::new(FileLogger));
    log::set_max_level(log::LevelFilter::Info);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_name_format() {
        let n = daily_log_name();
        assert!(n.starts_with("app-") && n.ends_with(".log"));
        assert_eq!(n.len(), "app-YYYYMMDD.log".len());
        // 与本地日期一致（不能用 UTC）
        let expect = chrono::Local::now().format("app-%Y%m%d.log").to_string();
        assert_eq!(n, expect);
    }

    #[test]
    fn daily_name_is_local_date() {
        let n = daily_log_name();
        let local = chrono::Local::now().format("%Y%m%d").to_string();
        assert!(
            n.contains(&local),
            "file name {n} must contain local date {local}"
        );
    }
}
