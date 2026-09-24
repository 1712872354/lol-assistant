//! LCU 连接状态类型（对齐 Go internal/lcu/types.go）。

use serde::{Deserialize, Serialize};

/// 连接状态机取值（与前端 ConnState 字面量一致）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Disconnected,
    Connected,
    Unauthenticated,
}

impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Disconnected => "disconnected",
            State::Connected => "connected",
            State::Unauthenticated => "unauthenticated",
        }
    }
}

/// 连接状态快照，经 conn:status 事件推送前端
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnStatus {
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summoner_level: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_icon_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub puuid: Option<String>,
}

/// 已验证的 LCU 连接凭据
///
/// `Debug` 手写实现：token 一律掩码为 `***`，防止凭据随日志/panic 输出泄漏。
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub pid: i32,
    pub port: u16,
    pub token: String,
    pub platform_id: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("pid", &self.pid)
            .field("port", &self.port)
            .field("token", &"***")
            .field("platform_id", &self.platform_id)
            .finish()
    }
}

impl Credentials {
    /// 凭据等价判定（pid/port/token 任一变化即需重连；platform_id 不触发重连）
    pub fn equal(&self, o: &Credentials) -> bool {
        self.pid == o.pid && self.port == o.port && self.token == o.token
    }
}

/// LCU WS 事件（转发前端 gameinfo:update）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcuEvent {
    pub uri: String,
    #[serde(rename = "eventType")]
    pub event_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T2.2 回归：Debug 输出必须掩码 token，防止凭据随日志/panic 泄漏。
    #[test]
    fn credentials_debug_masks_token() {
        let c = Credentials {
            pid: 1,
            port: 2,
            token: "secret-token-abc".into(),
            platform_id: "HN1".into(),
        };
        let dbg = format!("{c:?}");
        assert!(
            !dbg.contains("secret-token-abc"),
            "token 不得出现在 Debug 输出: {dbg}"
        );
        assert!(dbg.contains("***"), "应有掩码标记: {dbg}");
        // 其余字段仍可诊断
        assert!(dbg.contains("HN1"));
        assert!(dbg.contains('1'), "pid 应保留: {dbg}");
    }
}
