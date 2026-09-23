//! Live Client Data API 客户端（仅游戏进程内可达，https://127.0.0.1:2999）。
//! 自签名证书跳过校验 + 禁用系统代理 + 2s 短超时。

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const BASE: &str = "https://127.0.0.1:2999";
const PATH_PLAYER_LIST: &str = "/liveclientdata/playerlist";
const PATH_ACTIVE_PLAYER_NAME: &str = "/liveclientdata/activeplayername";
const REQ_TIMEOUT: Duration = Duration::from_secs(2);

/// 队伍编号。响应随版本浮动：数字 / 字符串 / 别名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TeamId {
    #[default]
    None,
    Blue,
    Red,
}

impl TeamId {
    pub fn as_i32(&self) -> i32 {
        match self {
            TeamId::None => 0,
            TeamId::Blue => 100,
            TeamId::Red => 200,
        }
    }
}

/// normalizeTeam 归一化队伍编号（纯函数，可测）。
pub fn normalize_team(n: i64, s: &str) -> TeamId {
    let s = s.trim().to_uppercase();
    let mut n = n;
    if n == 0 {
        if let Ok(f) = s.parse::<f64>() {
            n = f as i64;
        }
    }
    match n {
        100 => TeamId::Blue,
        200 => TeamId::Red,
        _ => match s.as_str() {
            "100" | "BLUE" | "ORDER" => TeamId::Blue,
            "200" | "RED" | "CHAOS" => TeamId::Red,
            _ => TeamId::None,
        },
    }
}

/// 从 JSON Value 容错解析 TeamId。
pub fn parse_team_id(v: &Value) -> TeamId {
    match v {
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            normalize_team(f as i64, "")
        }
        Value::String(s) => normalize_team(0, s),
        _ => TeamId::None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    #[serde(default, rename = "summonerName")]
    pub summoner_name: String,
    #[serde(default, rename = "riotIdGameName")]
    pub riot_id_game_name: String,
    #[serde(default, rename = "riotIdTagLine")]
    pub riot_id_tag_line: String,
    #[serde(default)]
    pub puuid: String,
    #[serde(default, rename = "championName")]
    pub champion_name: String,
    #[serde(default)]
    pub team: TeamId,
    #[serde(default)]
    pub level: i32,
}

impl Player {
    /// 解析显示名：riotId 优先，回落 summonerName（可能是 "name#tag"）
    pub fn name(&self) -> (String, String) {
        if !self.riot_id_game_name.is_empty() {
            return (
                self.riot_id_game_name.clone(),
                self.riot_id_tag_line.clone(),
            );
        }
        if let Some(i) = self.summoner_name.find('#') {
            return (
                self.summoner_name[..i].to_string(),
                self.summoner_name[i + 1..].to_string(),
            );
        }
        (self.summoner_name.clone(), String::new())
    }
}

/// 自定义 Deserialize：team 字段可能是数字/字符串/对象枚举
impl<'de> Deserialize<'de> for TeamId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = Value::deserialize(deserializer)?;
        Ok(parse_team_id(&v))
    }
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true) // live client 自签
            .timeout(REQ_TIMEOUT)
            .no_proxy()
            .build()
            .expect("build live client");
        Self { http }
    }

    async fn get_json(&self, path: &str) -> Result<Value, String> {
        let url = format!("{BASE}{path}");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("live client {path}: {e}"))?;
        if resp.status().as_u16() != 200 {
            return Err(format!(
                "live client {path}: http {}",
                resp.status().as_u16()
            ));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("live client {path} read: {e}"))?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(format!("live client {path}: body too large"));
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("live client {path} decode: {e}"))
    }

    pub async fn player_list(&self) -> Result<Vec<Player>, String> {
        let v = self.get_json(PATH_PLAYER_LIST).await?;
        serde_json::from_value(v).map_err(|e| format!("live client playerlist decode: {e}"))
    }

    pub async fn active_player_name(&self) -> Result<String, String> {
        let v = self.get_json(PATH_ACTIVE_PLAYER_NAME).await?;
        v.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "live client activeplayername: not a string".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_team_cases() {
        assert_eq!(normalize_team(100, ""), TeamId::Blue);
        assert_eq!(normalize_team(200, ""), TeamId::Red);
        assert_eq!(normalize_team(0, "100"), TeamId::Blue);
        assert_eq!(normalize_team(0, "200"), TeamId::Red);
        assert_eq!(normalize_team(0, "BLUE"), TeamId::Blue);
        assert_eq!(normalize_team(0, "RED"), TeamId::Red);
        assert_eq!(normalize_team(0, "blue"), TeamId::Blue);
        assert_eq!(normalize_team(0, "ORDER"), TeamId::Blue);
        assert_eq!(normalize_team(0, "CHAOS"), TeamId::Red);
        assert_eq!(normalize_team(0, " order "), TeamId::Blue);
        assert_eq!(normalize_team(0, "XX"), TeamId::None);
        assert_eq!(normalize_team(0, ""), TeamId::None);
    }

    #[test]
    fn parse_team_id_json_shapes() {
        assert_eq!(parse_team_id(&serde_json::json!(100)), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!(200)), TeamId::Red);
        assert_eq!(parse_team_id(&serde_json::json!(100.0)), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!(200.0)), TeamId::Red);
        assert_eq!(parse_team_id(&serde_json::json!("100")), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!("200")), TeamId::Red);
        assert_eq!(parse_team_id(&serde_json::json!("100.0")), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!("BLUE")), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!("RED")), TeamId::Red);
        assert_eq!(parse_team_id(&serde_json::json!("blue")), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!("ORDER")), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!("CHAOS")), TeamId::Red);
        assert_eq!(parse_team_id(&serde_json::json!(" order ")), TeamId::Blue);
        assert_eq!(parse_team_id(&serde_json::json!(0)), TeamId::None);
        assert_eq!(parse_team_id(&serde_json::json!("XX")), TeamId::None);
        assert_eq!(parse_team_id(&serde_json::Value::Null), TeamId::None);
    }

    #[test]
    fn player_name_variants() {
        let p = Player {
            riot_id_game_name: "歪比".into(),
            riot_id_tag_line: "CN1".into(),
            summoner_name: "旧名#OLD".into(),
            ..Default::default()
        };
        let (g, tag) = p.name();
        assert_eq!(g, "歪比");
        assert_eq!(tag, "CN1");

        let p = Player {
            summoner_name: "小名#60021".into(),
            ..Default::default()
        };
        let (g, tag) = p.name();
        assert_eq!(g, "小名");
        assert_eq!(tag, "60021");

        let p = Player {
            summoner_name: "纯名".into(),
            ..Default::default()
        };
        let (g, tag) = p.name();
        assert_eq!(g, "纯名");
        assert_eq!(tag, "");
    }

    #[test]
    fn team_id_serde_roundtrip() {
        let v = serde_json::json!({"team": 100, "summonerName": "x", "level": 1});
        let p: Player = serde_json::from_value(v).unwrap();
        assert_eq!(p.team, TeamId::Blue);

        let v = serde_json::json!({"team": "RED", "summonerName": "y", "level": 2});
        let p: Player = serde_json::from_value(v).unwrap();
        assert_eq!(p.team, TeamId::Red);
    }
}
