//! 配置存储：与 Go `internal/config` 对齐。
//! 路径：%APPDATA%\LOLAssistant\config.json；损坏文件备份为 .bad。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub schema_version: u32,
    pub theme: String,
    pub page_size: i64,
    pub sgp_enabled: bool,
    pub close_to_tray: bool,
    pub client_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema_version: 1,
            theme: "system".into(),
            page_size: 20,
            sgp_enabled: true,
            close_to_tray: true,
            client_path: String::new(),
        }
    }
}

pub struct Store {
    inner: RwLock<Config>,
    path: PathBuf,
}

impl Store {
    pub fn global() -> &'static Store {
        static STORE: OnceLock<Store> = OnceLock::new();
        STORE.get_or_init(|| {
            let path = config_path();
            let cfg = load_or_default(&path);
            Store {
                inner: RwLock::new(cfg),
                path,
            }
        })
    }

    pub fn get(&self) -> Config {
        self.inner.read().expect("config lock").clone()
    }

    pub fn set(&self, mut cfg: Config) -> Result<(), String> {
        sanitize(&mut cfg);
        save(&self.path, &cfg)?;
        *self.inner.write().expect("config lock") = cfg;
        Ok(())
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("LOLAssistant").join("config.json")
}

fn load_or_default(path: &Path) -> Config {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<Config>(&bytes) {
            Ok(c) => c,
            Err(_) => {
                let bad = path.with_extension("json.bad");
                let _ = std::fs::rename(path, bad);
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

fn save(path: &Path, cfg: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn sanitize(cfg: &mut Config) {
    if cfg.schema_version == 0 {
        cfg.schema_version = 1;
    }
    if !(5..=50).contains(&cfg.page_size) {
        cfg.page_size = 20;
    }
    if cfg.theme != "light" && cfg.theme != "dark" && cfg.theme != "system" {
        cfg.theme = "system".into();
    }
    sanitize_client_path(&mut cfg.client_path);
}

/// ClientPath：允许空；拒绝 UNC（\\）与相对路径（防 NTLM 外泄）。
fn sanitize_client_path(p: &mut String) {
    if p.is_empty() {
        return;
    }
    let trimmed = p.trim();
    let is_unc = trimmed.starts_with("\\\\") || trimmed.starts_with("//");
    let is_absolute_drive = {
        let b = trimmed.as_bytes();
        b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/')
    };
    if is_unc || !is_absolute_drive {
        p.clear();
        return;
    }
    *p = trimmed.to_string();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_unc_and_relative() {
        let mut p = "\\\\server\\share".into();
        sanitize_client_path(&mut p);
        assert_eq!(p, "");

        let mut p = "relative/path".into();
        sanitize_client_path(&mut p);
        assert_eq!(p, "");

        let mut p = "C:\\Games\\LOL".into();
        sanitize_client_path(&mut p);
        assert_eq!(p, "C:\\Games\\LOL");
    }

    #[test]
    fn default_config() {
        let c = Config::default();
        assert_eq!(c.page_size, 20);
        assert!(c.sgp_enabled);
        assert!(c.close_to_tray);
    }

    #[test]
    fn sanitize_page_size_out_of_range() {
        let mut c = Config {
            page_size: 1,
            ..Default::default()
        };
        sanitize(&mut c);
        assert_eq!(c.page_size, 20);
        let mut c = Config {
            page_size: 51,
            ..Default::default()
        };
        sanitize(&mut c);
        assert_eq!(c.page_size, 20);
    }

    #[test]
    fn sanitize_theme_system_allowed() {
        let mut c = Config {
            theme: "system".into(),
            ..Default::default()
        };
        sanitize(&mut c);
        assert_eq!(c.theme, "system");
        let mut c = Config {
            theme: "neon".into(),
            ..Default::default()
        };
        sanitize(&mut c);
        assert_eq!(c.theme, "system");
    }

    #[test]
    fn serde_uses_camel_case() {
        let c = Config::default();
        let s = serde_json::to_string(&c).unwrap();
        assert!(s.contains("\"pageSize\""));
        assert!(s.contains("\"closeToTray\""));
        assert!(s.contains("\"clientPath\""));
    }
}
