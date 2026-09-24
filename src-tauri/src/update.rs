//! 更新：tauri-plugin-updater + minisign（对齐 Yuumi 模式）。
//! 安装版走 NSIS 下载安装；便携版经 portable_updater helper 热替换。
//! 进度事件对外名保持 `update:progress`（兼容前端 store）。

use std::sync::atomic::Ordering;
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::portable_updater;

pub const REPO_OWNER: &str = "1712872354";
pub const REPO_NAME: &str = "lol-assistant";

/// 更新检查结果（JSON 给前端）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub has_update: bool,
    pub current_version: String,
    pub version: String,
    pub notes: String,
    pub pub_date: String,
    pub release_url: String,
}

/// 下载/安装进度（经 update:progress 推前端）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: String,
    pub percent: i32,
    pub message: String,
}

impl Progress {
    pub fn new(stage: &str, percent: i32, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            percent,
            message: message.into(),
        }
    }
}

/// Release 说明 → 更新弹窗可读纯文本。
pub fn format_notes(md: &str) -> String {
    let mut s = md.replace("\r\n", "\n");
    s = drop_section(&s, "安装校验");
    s = drop_section(&s, "安装");

    let re_code = Regex::new(r"(?s)```[\w-]*\n([\s\S]*?)```").unwrap();
    s = re_code
        .replace_all(&s, |caps: &regex::Captures| {
            let body = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            body.lines()
                .map(|l| {
                    let t = l.trim();
                    if t.is_empty() {
                        String::new()
                    } else {
                        format!("    {t}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .to_string();

    let re_table_sep = Regex::new(r"(?m)^\|[\s:|-]+\|$").unwrap();
    s = re_table_sep.replace_all(&s, "").to_string();
    let re_table_row = Regex::new(r"(?m)^\|.+\|$").unwrap();
    s = re_table_row
        .replace_all(&s, |caps: &regex::Captures| {
            let m = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            let row = m.trim().trim_matches('|');
            row.split('|')
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .to_string();

    let re_heading = Regex::new(r"(?m)^#{1,6}[ \t]+").unwrap();
    s = re_heading.replace_all(&s, "").to_string();
    let re_bullet = Regex::new(r"(?m)^\s*[-*+][ \t]+").unwrap();
    s = re_bullet.replace_all(&s, "• ").to_string();
    let re_bold = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    s = re_bold.replace_all(&s, "$1").to_string();
    let re_inline = Regex::new(r"`([^`]+)`").unwrap();
    s = re_inline.replace_all(&s, "$1").to_string();
    let re_hr = Regex::new(r"(?m)^---+[ \t]*$").unwrap();
    s = re_hr.replace_all(&s, "").to_string();
    let re_blank = Regex::new(r"\n{3,}").unwrap();
    s = re_blank.replace_all(&s, "\n\n").to_string();
    s = s.trim().to_string();
    if s.is_empty() {
        "本次更新内容暂无说明。".into()
    } else {
        s
    }
}

fn drop_section(s: &str, title: &str) -> String {
    let mut out = Vec::new();
    let mut skipping = false;
    for line in s.lines() {
        let trim = line.trim();
        if !skipping {
            if let Some(rest) = trim.strip_prefix("###") {
                if rest.trim() == title {
                    skipping = true;
                    continue;
                }
            }
            out.push(line);
        } else if trim.starts_with('#') || trim.starts_with("---") {
            skipping = false;
            out.push(line);
        }
    }
    out.join("\n")
}

/// 构建 updater（便携版切到 windows-*-portable target）。
async fn build_updater(
    app: &AppHandle,
) -> Result<tauri_plugin_updater::Updater, crate::error::AppError> {
    let builder = app.updater_builder();
    let builder = if portable_updater::is_portable() {
        match std::env::consts::ARCH {
            "x86_64" => builder.target("windows-x86_64-portable"),
            "aarch64" => builder.target("windows-aarch64-portable"),
            other => {
                return Err(crate::error::AppError::Updater(format!(
                    "不支持的便携版更新架构: {other}"
                )))
            }
        }
    } else {
        builder
    };
    builder
        .build()
        .map_err(|e| crate::error::AppError::Updater(format!("无法初始化更新器: {e}")))
}

/// 语义化版本三段数字比较（容忍 `v` 前缀；非数字段按 0）。
/// remote 严格大于 current 才视为可更新——相等/降级一律拒绝（反回滚）。
pub(crate) fn version_newer(remote: &str, current: &str) -> bool {
    parse_ver(remote) > parse_ver(current)
}

fn parse_ver(v: &str) -> (u64, u64, u64) {
    let v = v.trim().trim_start_matches('v');
    let mut it = v.split('.');
    let major = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// check：TargetNotFound 视为无更新（latest.json 尚无 portable 平台）。
/// 反回滚：远程版本未严格高于当前版本时按“无更新”处理（防 latest.json 元数据投毒降级诱导）。
pub(crate) async fn check_update_opt(
    app: &AppHandle,
) -> Result<Option<tauri_plugin_updater::Update>, crate::error::AppError> {
    let updater = build_updater(app).await?;
    match updater.check().await {
        Ok(update) => {
            if let Some(u) = &update {
                let current = app.package_info().version.to_string();
                if !version_newer(&u.version, &current) {
                    log::warn!(
                        "[update] 拒绝非升级版本 remote={} current={}（反回滚）",
                        u.version,
                        current
                    );
                    return Ok(None);
                }
            }
            Ok(update)
        }
        Err(
            tauri_plugin_updater::Error::TargetNotFound(_)
            | tauri_plugin_updater::Error::TargetsNotFound(_),
        ) => Ok(None),
        Err(e) => Err(crate::error::AppError::Updater(format!(
            "检查更新失败: {e}"
        ))),
    }
}

fn release_url_for(version: &str) -> String {
    let v = version.trim().trim_start_matches('v');
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/tag/v{v}")
}

/// 把插件 Update 映射为前端 Info。
pub fn info_from_update(update: &tauri_plugin_updater::Update, current: &str) -> Info {
    let version = update.version.trim_start_matches('v').to_string();
    let notes = update
        .body
        .as_deref()
        .map(format_notes)
        .unwrap_or_else(|| "本次更新内容暂无说明。".into());
    let pub_date = update.date.map(|d| d.to_string()).unwrap_or_default();
    Info {
        has_update: true,
        current_version: current.trim_start_matches('v').to_string(),
        version,
        notes,
        pub_date,
        release_url: release_url_for(&update.version),
    }
}

/// CAS 防重入：占用成功返回 true，调用方负责最终释放（成功路径可不释放，因随后退出）。
pub fn try_begin_download(app: &AppHandle) -> Result<(), crate::error::AppError> {
    let state = app.state::<crate::AppState>();
    state
        .is_downloading
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| ())
        .map_err(|_| crate::error::AppError::Invalid("更新正在下载中，请稍候".into()))
}

pub fn end_download(app: &AppHandle) {
    let state = app.state::<crate::AppState>();
    state.is_downloading.store(false, Ordering::Release);
}

/// 检查更新（不下载）。便携版走 portable target。
pub async fn check(app: &AppHandle) -> Result<Info, crate::error::AppError> {
    let current = app.package_info().version.to_string();
    match check_update_opt(app).await? {
        Some(update) => Ok(info_from_update(&update, &current)),
        None => Ok(Info {
            has_update: false,
            current_version: current.trim_start_matches('v').to_string(),
            ..Info::default()
        }),
    }
}

/// 启动 3s 后静默检查：仅记录/通知，不自动下载。
pub async fn startup_check_update(app: AppHandle) {
    match check(&app).await {
        Ok(info) if info.has_update => {
            log::info!("启动检查发现新版本 v{}", info.version);
            let _ = app.emit("update:available", &info);
        }
        Ok(_) => log::info!("启动检查：已是最新版本"),
        Err(e) => log::warn!("启动检查更新失败: {e}"),
    }
}

/// 安装版：download_and_install（进度 → update:progress），成功后由调用方强退。
pub async fn download_and_install_installed(
    app: &AppHandle,
    on_progress: impl Fn(Progress) + Send + Sync + 'static,
) -> Result<(), crate::error::AppError> {
    let update = check_update_opt(app)
        .await?
        .ok_or_else(|| "没有可用的更新".to_string())?;

    let downloaded = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let downloaded2 = downloaded.clone();
    let progress = std::sync::Arc::new(on_progress);
    let progress2 = progress.clone();
    progress(Progress::new("downloading", 0, "开始下载安装包"));

    update
        .download_and_install(
            move |chunk, total| {
                let current = downloaded2.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                let percent = total
                    .filter(|t| *t > 0)
                    .map(|t| ((current * 100) / t).min(99) as i32)
                    .unwrap_or(0);
                progress2(Progress::new(
                    "downloading",
                    percent,
                    format!("下载中 {percent}%"),
                ));
            },
            || {
                log::info!("更新下载完成，准备安装");
            },
        )
        .await
        .map_err(|e| crate::error::AppError::Updater(format!("下载/安装更新失败: {e}")))?;

    progress(Progress::new("done", 100, "更新安装完成，正在重启"));
    Ok(())
}

/// 统一入口：按安装版/便携版分发；进度经 update:progress。
/// 成功返回 Ok(()) 后由调用方触发退出（安装版可等安装器拉起；便携版 helper 已 spawn）。
pub async fn download_and_install(app: AppHandle) -> Result<(), crate::error::AppError> {
    try_begin_download(&app)?;
    let result = if portable_updater::is_portable() {
        portable_updater::download_and_apply(&app).await
    } else {
        let app2 = app.clone();
        let progress_app = app.clone();
        download_and_install_installed(&app2, move |p| {
            let _ = progress_app.emit("update:progress", &p);
        })
        .await
    };
    if result.is_err() {
        end_download(&app);
    }
    result
}

/// 下载安装并强制退出（绕过 closeToTray），供 commands 调用。
pub async fn download_install_and_quit(app: AppHandle) -> Result<(), crate::error::AppError> {
    download_and_install(app.clone()).await?;

    let state = app.state::<crate::AppState>();
    state.force_quit.store(true, Ordering::SeqCst);
    let mon = state.monitor.clone();
    let quit_app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        mon.stop().await;
        quit_app.exit(0);
    });
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(900));
        log::warn!("update: force exit process for installer");
        let _ = crate::config::Store::global().get();
        std::process::exit(0);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_notes_strips_install_and_markdown() {
        let md = "### 安装\r\n\r\n| 包 | 说明 |\r\n|----|------|\r\n| `Setup-1.0.0.exe` | NSIS 安装包 |\r\n\r\nWindows x64 · 需 WebView2\r\n\r\n### 修复\r\n\r\n- **战绩数量真正生效**：不再写死 20\r\n\r\n### 安装校验\r\n\r\n```powershell\r\nGet-FileHash x\r\n```\r\n\r\n---\r\n";
        let got = format_notes(md);
        assert!(!got.contains("### "), "{got}");
        assert!(!got.contains('|'), "{got}");
        assert!(!got.contains("```"), "{got}");
        assert!(!got.contains("安装包"), "{got}");
        assert!(!got.contains("Get-FileHash"), "{got}");
        assert!(got.contains("战绩数量真正生效"), "{got}");
        assert!(got.contains("不再写死 20"), "{got}");
        assert!(got.contains("• 战绩数量真正生效"), "{got}");
    }

    #[test]
    fn format_notes_empty() {
        assert_eq!(format_notes("   "), "本次更新内容暂无说明。");
    }

    #[test]
    fn release_url_formats() {
        assert_eq!(
            release_url_for("v1.0.7"),
            "https://github.com/1712872354/lol-assistant/releases/tag/v1.0.7"
        );
        assert_eq!(
            release_url_for("1.0.7"),
            "https://github.com/1712872354/lol-assistant/releases/tag/v1.0.7"
        );
    }

    /// T2.1 回归：Info 不得再携带恒空死字段（setupUrl/portableUrl/sha256）。
    #[test]
    fn update_info_serialized_without_dead_fields() {
        let info = Info {
            has_update: true,
            current_version: "1.0.7".into(),
            version: "1.0.8".into(),
            notes: "n".into(),
            pub_date: "d".into(),
            release_url: "r".into(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert!(json.get("sha256").is_none(), "sha256 死字段必须移除");
        assert!(json.get("setupUrl").is_none(), "setupUrl 死字段必须移除");
        assert!(
            json.get("portableUrl").is_none(),
            "portableUrl 死字段必须移除"
        );
        assert_eq!(json.get("version").unwrap(), "1.0.8");
        assert_eq!(json.get("releaseUrl").unwrap(), "r");
    }

    /// T2.1 回归：更新元数据端点不得走第三方镜像（ghp.ci 等），只信任 GitHub 官方源。
    #[test]
    fn update_endpoints_exclude_third_party_mirrors() {
        let conf = include_str!("../tauri.conf.json");
        assert!(
            !conf.contains("ghp.ci"),
            "updater endpoints 不得包含第三方镜像 ghp.ci"
        );
        assert!(
            conf.contains(
                "https://github.com/1712872354/lol-assistant/releases/latest/download/latest.json"
            ),
            "必须保留 GitHub 官方 latest.json 端点"
        );
    }

    /// T2.1 回归：反回滚——仅严格更高的版本视为可更新，相等/降级一律拒绝（防 latest.json 投毒降级诱导）。
    #[test]
    fn version_newer_rejects_downgrade_and_equal() {
        assert!(version_newer("1.0.8", "1.0.7"));
        assert!(version_newer("v1.0.8", "1.0.7"), "v 前缀需容忍");
        assert!(version_newer("1.1.0", "1.0.9"));
        assert!(version_newer("2.0.0", "1.9.9"));
        assert!(version_newer("1.0.10", "1.0.9"), "按数比较非字典序");
        assert!(!version_newer("1.0.7", "1.0.7"), "相等不算更新");
        assert!(!version_newer("1.0.6", "1.0.7"), "降级必须拒绝");
        assert!(!version_newer("0.9.9", "1.0.0"));
        assert!(!version_newer("", "1.0.0"), "空版本拒绝");
    }
}
