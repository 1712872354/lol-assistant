//! 前端绑定命令层：参数校验 + 调用服务 + 事件桥接（对齐 Go app.go）。
//! Tauri 命令为 snake_case 函数名；具名参数 JS 侧用 camelCase（Tauri 2 默认转换）。
//! 错误边界：内部 `AppError` 在此转用户可读文案（`String`）给前端。

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, State};

use crate::config;
use crate::lcu::ConnStatus;
use crate::parser::MatchDetail;
use crate::service::{AssetResult, MatchPage, RankedInfo, SummonerResult, ViewState};
use crate::AppState;

#[tauri::command]
pub fn get_app_version() -> String {
    option_env!("CARGO_PKG_VERSION")
        .unwrap_or("dev")
        .to_string()
}

#[tauri::command]
pub fn get_config() -> Result<config::Config, String> {
    Ok(config::Store::global().get())
}

/// 校验并持久化配置，并热更新各服务生效项。
#[tauri::command]
pub async fn set_config(app: AppHandle, cfg: config::Config) -> Result<(), String> {
    config::Store::global().set(cfg.clone())?;
    let state = app.state::<AppState>();
    state.monitor.set_client_path(&cfg.client_path).await;
    state.hist.set_page_size(cfg.page_size as i32);
    state.hist.set_sgp_enabled(cfg.sgp_enabled);
    state.game.set_career_limit(cfg.career_limit as i32);
    Ok(())
}

#[tauri::command]
pub async fn get_conn_status(state: State<'_, AppState>) -> Result<ConnStatus, String> {
    Ok(state.monitor.status().await)
}

/* ── M2 历史战绩 ── */

#[tauri::command]
pub async fn get_matches(
    state: State<'_, AppState>,
    puuid: String,
    page: i32,
) -> Result<MatchPage, String> {
    Ok(state.hist.get_matches(&puuid, page).await?)
}

#[tauri::command]
pub async fn get_match_detail(
    state: State<'_, AppState>,
    game_id: i64,
    self_puuid: String,
) -> Result<MatchDetail, String> {
    Ok(state.hist.get_match_detail(game_id, &self_puuid).await?)
}

#[tauri::command]
pub async fn search_summoner(
    state: State<'_, AppState>,
    name: String,
) -> Result<SummonerResult, String> {
    Ok(state.hist.search_summoner(&name).await?)
}

#[tauri::command]
pub async fn get_self_summoner(state: State<'_, AppState>) -> Result<SummonerResult, String> {
    Ok(state.hist.get_self_summoner().await?)
}

#[tauri::command]
pub async fn get_players_ranked(
    state: State<'_, AppState>,
    query_ids: Vec<String>,
) -> Result<Vec<RankedInfo>, String> {
    Ok(state.hist.get_players_ranked(&query_ids).await?)
}

#[tauri::command]
pub async fn get_match_asset(
    state: State<'_, AppState>,
    kind: String,
    id: i32,
) -> Result<AssetResult, String> {
    Ok(state.hist.get_asset(&kind, id).await?)
}

/* ── M3 对局信息 ── */

#[tauri::command]
pub async fn get_gameflow_state(
    state: State<'_, AppState>,
    queue_filter: Option<Vec<i32>>,
) -> Result<ViewState, String> {
    Ok(state.game.get_gameflow_state(queue_filter).await?)
}

/* ── 更新（tauri-plugin-updater） ── */

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<crate::update::Info, String> {
    Ok(crate::update::check(&app).await?)
}

/// 下载并安装更新（安装版/便携版自动分流；进度经 update:progress）。
/// 成功后强制退出本进程（绕过 closeToTray）。
#[tauri::command]
pub async fn download_and_install_update(app: AppHandle) -> Result<(), String> {
    Ok(crate::update::download_install_and_quit(app).await?)
}

/// 是否便携模式（exe 旁 portable.flag）
#[tauri::command]
pub fn is_portable() -> bool {
    crate::portable_updater::is_portable()
}

/* ── 窗口控制（自绘标题栏） ── */

#[tauri::command]
pub fn window_minimise(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_toggle_maximise(window: tauri::Window) -> Result<(), String> {
    if window.is_maximized().unwrap_or(false) {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

/// 关闭窗口（自绘标题栏 X）。
/// close_to_tray=true 且非强退时隐藏到托盘；否则直接退出。
#[tauri::command]
pub fn window_close(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !state.force_quit.load(Ordering::SeqCst) {
        let cfg = config::Store::global().get();
        if cfg.close_to_tray {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
            log::info!("window hidden to tray");
            return Ok(());
        }
    }
    state.force_quit.store(true, Ordering::SeqCst);
    app.exit(0);
    Ok(())
}
