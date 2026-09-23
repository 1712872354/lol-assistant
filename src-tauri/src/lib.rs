pub mod commands;
pub mod config;
pub mod lcu;
pub mod liveclient;
pub mod logging;
pub mod parser;
pub mod portable_updater;
pub mod service;
pub mod sgp;
pub mod update;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

use crate::lcu::{ConnStatus, LcuEvent, Monitor};
use crate::service::http::{LcuHttp, MonitorHttp};
use crate::service::{GameinfoService, HistApi, HistoryService, LiveApi};

/// 共享应用状态（Tauri manage）
pub struct AppState {
    pub monitor: Monitor,
    pub hist: Arc<HistoryService>,
    pub game: Arc<GameinfoService>,
    /// 强制退出：更新安装/主动退出时绕过 closeToTray 拦截
    pub force_quit: Arc<AtomicBool>,
    /// 更新下载中（CAS 防重入）
    pub is_downloading: Arc<AtomicBool>,
}

pub fn run() {
    logging::init();
    portable_updater::schedule_cleanup_from_environment();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let cfg = config::Store::global().get();
            let monitor = Monitor::new();

            let http: Arc<dyn LcuHttp> = Arc::new(MonitorHttp::new(monitor.clone()));
            let hist = Arc::new(HistoryService::new(http.clone(), cfg.page_size as i32));
            hist.set_sgp_enabled(cfg.sgp_enabled);

            let live = Arc::new(liveclient::Client::new());
            let hist_api: Arc<dyn HistApi> = hist.clone();
            let live_api: Arc<dyn LiveApi> = live;
            let game = Arc::new(GameinfoService::new(http, hist_api, live_api));
            game.set_career_limit(cfg.page_size as i32);

            let force_quit = Arc::new(AtomicBool::new(false));
            app.manage(AppState {
                monitor: monitor.clone(),
                hist: hist.clone(),
                game: game.clone(),
                force_quit: force_quit.clone(),
                is_downloading: Arc::new(AtomicBool::new(false)),
            });

            // client_path 需异步写入；spawn 循环前同步一次（block_on 在 setup 内可用）
            {
                let mon = monitor.clone();
                let path = cfg.client_path.clone();
                tauri::async_runtime::block_on(async move {
                    mon.set_client_path(&path).await;
                });
            }

            fit_window_to_screen(app.handle());

            // ─── 系统托盘：显示主界面 / 退出（对齐 Go internal/tray）───
            let show_item = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &sep, &quit_item])?;

            let _tray = TrayIconBuilder::with_id("main_tray")
                .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
                    log::warn!("default_window_icon 为 None，使用 1x1 透明像素占位");
                    tauri::image::Image::new(&[0, 0, 0, 0], 1, 1)
                }))
                .menu(&tray_menu)
                .tooltip("LOL助手")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "quit" => quit_from_tray(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            let handle = app.handle().clone();
            let mon = monitor.clone();
            tauri::async_runtime::spawn(async move {
                let h1 = handle.clone();
                let h2 = handle;
                mon.spawn(
                    move |st: ConnStatus| {
                        let _ = h1.emit("conn:status", &st);
                    },
                    move |evt: LcuEvent| {
                        let _ = h2.emit("gameinfo:update", &evt);
                    },
                );
            });

            // 启动 3s 后静默检查更新（不自动下载；前端另有 4s 用户可见检查）
            {
                let startup_app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    update::startup_check_update(startup_app).await;
                });
            }

            log::info!("app started");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if !state.force_quit.load(Ordering::SeqCst) {
                    let cfg = config::Store::global().get();
                    if cfg.close_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                        log::info!("window hidden to tray");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_version,
            commands::get_config,
            commands::set_config,
            commands::get_conn_status,
            commands::get_matches,
            commands::get_match_detail,
            commands::search_summoner,
            commands::get_self_summoner,
            commands::get_players_ranked,
            commands::get_match_asset,
            commands::get_gameflow_state,
            commands::check_update,
            commands::download_and_install_update,
            commands::is_portable,
            commands::window_minimise,
            commands::window_toggle_maximise,
            commands::window_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            if let Some(state) = app.try_state::<AppState>() {
                let mon = state.monitor.clone();
                tauri::async_runtime::block_on(mon.stop());
            }
        }
    });
}

/// 托盘唤回：显示并置于前台（短暂置顶抢焦点，对齐 Go showMainWindow）。
fn show_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
}

/// 托盘「退出」：force_quit 先落（绕过 closeToTray），停 monitor 后退出。
fn quit_from_tray(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    state.force_quit.store(true, Ordering::SeqCst);
    let mon = state.monitor.clone();
    let quit_app = app.clone();
    tauri::async_runtime::spawn(async move {
        mon.stop().await;
        quit_app.exit(0);
    });
}

/// 小屏时把默认窗口收到工作区约 92%，避免溢出；大屏保持默认尺寸。
fn fit_window_to_screen(app: &tauri::AppHandle) {
    const WANT_W: f64 = 1680.0;
    const WANT_H: f64 = 980.0;
    const MIN_W: f64 = 1200.0;
    const MIN_H: f64 = 780.0;

    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(mon)) = win.primary_monitor() else {
        let _ = win.center();
        return;
    };
    let size = mon.size();
    let scale = win.scale_factor().unwrap_or(1.0);
    let sw = size.width as f64 / scale;
    let sh = size.height as f64 / scale;
    let max_w = sw * 0.92;
    let max_h = sh * 0.92;
    let mut w = WANT_W;
    let mut h = WANT_H;
    if w > max_w {
        w = max_w;
    }
    if h > max_h {
        h = max_h;
    }
    if w < MIN_W {
        w = MIN_W;
    }
    if h < MIN_H {
        h = MIN_H;
    }
    use tauri::PhysicalSize;
    let phys = PhysicalSize::new((w * scale) as u32, (h * scale) as u32);
    let _ = win.set_size(phys);
    let _ = win.center();
    log::info!("window sized {}x{}", w, h);
}

/// 便携版更新 helper 进程入口判定（main() 第一行调用）。
/// 返回 true 表示当前进程是更新 helper 且已处理完毕，main 应立即 return。
pub fn run_portable_update_helper_if_requested() -> bool {
    portable_updater::run_helper_if_requested()
}
