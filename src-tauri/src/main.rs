// Windows 发布版隐藏额外控制台窗口（Tauri 模板要求，勿删）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 便携版更新 helper 进程入口：必须在一切初始化之前判定
    if lol_assistant_lib::run_portable_update_helper_if_requested() {
        return;
    }

    // 在任何 TLS 连接之前安装 ring 作为全局 rustls CryptoProvider，
    // 避免 ring 与 aws-lc-rs 共存时 rustls 无法自动选择而 panic。
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    // WebView2 数据目录：中文 exe 名会导致创建失败，必须在创建 WebView 之前设置
    // 与 Go main.go webviewDataDir 对齐：%LOCALAPPDATA%\LolAssistant\EBWebView
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
        let dir = std::path::Path::new(&base)
            .join("LolAssistant")
            .join("EBWebView");
        let _ = std::fs::create_dir_all(&dir);
        // edition 2021：set_var 非线程安全，须在 unsafe 块中调用
        unsafe {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
        }
    }

    lol_assistant_lib::run()
}
