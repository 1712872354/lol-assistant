//! LCU 连接层：类型 / 端点 / 检测 / HTTP / WS / 监控状态机。

pub mod client;
pub mod cmdline;
pub mod endpoints;
pub mod lockfile;
pub mod monitor;
pub mod types;
#[cfg(windows)]
pub mod win_cmdline;
pub mod ws;

pub use client::Client;
pub use monitor::Monitor;
pub use types::{ConnStatus, Credentials, LcuEvent, State};
