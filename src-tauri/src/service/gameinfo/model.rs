//! 对局信息输出视图模型（经 Tauri 命令序列化给前端）。
//! 可空性统一：标量「未知/缺失」一律 `Option<T>` + `skip_serializing_if = "Option::is_none"`；
//! 布尔旗标（filled/is_self/win/hidden_career）与集合保持原样。
//! 阶段文案由前端 `lib/phase` 单点维护，后端不再下发 phase_label。

use serde::{Deserialize, Serialize};

/* ─── 输出视图模型 ─── */

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentMatch {
    pub queue_short: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub queue_name: Option<String>,
    pub time_short: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub game_creation: Option<i64>,
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub champion_id: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSlot {
    pub filled: bool,
    pub is_self: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub puuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub summoner_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub game_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tag_line: Option<String>,
    #[serde(
        rename = "profileIconId",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub profile_icon_id: Option<i32>,
    #[serde(
        rename = "championId",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub champion_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub solo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub flex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub win_rate: Option<f64>,
    #[serde(
        rename = "winRateSample",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub win_rate_sample: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub avg_kda: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub player_score: Option<f64>,
    #[serde(rename = "hiddenCareer", default)]
    pub hidden_career: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub recent: Vec<RecentMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeamView {
    pub key: String,
    pub label: String,
    pub side_text: String,
    pub badge: String,
    pub player_count: i32,
    pub win_rate: f64,
    pub team_score: i32,
    pub slots: Vec<PlayerSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ViewState {
    pub phase: String,
    pub queue_label: String,
    #[serde(rename = "queueId")]
    pub queue_id: i32,
    pub teams: Vec<TeamView>,
}
