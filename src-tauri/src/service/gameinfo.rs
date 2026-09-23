//! 对局信息页数据聚合（对齐 Go service/gameinfo）。
//! 按 gameflow 阶段选源，best-effort 补段位/近况；依赖经 trait 注入。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lcu::endpoints::{
    PATH_CHAMP_SELECT_SESSION, PATH_GAMEFLOW_PHASE, PATH_GAMEFLOW_SESSION,
    PATH_GD_CHAMPION_SUMMARY, PATH_LOBBY, PATH_SUMMONER_BY_ID, PATH_SUMMONER_BY_PUUID,
};
use crate::lcu::ConnStatus;
use crate::liveclient::{Player, TeamId};
#[cfg(test)]
use crate::parser::MatchSummary;
use crate::parser::{lookup_queue, queue_info_for};
use crate::service::history::{HistoryService, MatchPage, RankedInfo, SummonerResult};
use crate::service::http::{path_escape, BoxFut, LcuHttp};
use crate::service::ERR_NOT_CONNECTED;

const CAREER_TTL: Duration = Duration::from_secs(90);
const CAREER_SCAN_PAGES: i32 = 4;
const CAREER_CACHE_MAX: usize = 512;
const DEFAULT_CAREER_LIMIT: i32 = 20;
const CHAMP_INDEX_TTL: Duration = Duration::from_secs(10 * 60);

/* ── 依赖面 ── */

pub trait HistApi: Send + Sync {
    fn get_players_ranked<'a>(
        &'a self,
        ids: Vec<String>,
    ) -> BoxFut<'a, Result<Vec<RankedInfo>, String>>;
    fn get_matches<'a>(&'a self, puuid: String, page: i32)
        -> BoxFut<'a, Result<MatchPage, String>>;
    fn search_summoner<'a>(&'a self, name: String) -> BoxFut<'a, Result<SummonerResult, String>>;
}

impl HistApi for HistoryService {
    fn get_players_ranked<'a>(
        &'a self,
        ids: Vec<String>,
    ) -> BoxFut<'a, Result<Vec<RankedInfo>, String>> {
        Box::pin(async move { HistoryService::get_players_ranked(self, &ids).await })
    }

    fn get_matches<'a>(
        &'a self,
        puuid: String,
        page: i32,
    ) -> BoxFut<'a, Result<MatchPage, String>> {
        Box::pin(async move { HistoryService::get_matches(self, &puuid, page).await })
    }

    fn search_summoner<'a>(&'a self, name: String) -> BoxFut<'a, Result<SummonerResult, String>> {
        Box::pin(async move { HistoryService::search_summoner(self, &name).await })
    }
}

pub trait LiveApi: Send + Sync {
    fn player_list<'a>(&'a self) -> BoxFut<'a, Result<Vec<Player>, String>>;
    fn active_player_name<'a>(&'a self) -> BoxFut<'a, Result<String, String>>;
}

impl LiveApi for crate::liveclient::Client {
    fn player_list<'a>(&'a self) -> BoxFut<'a, Result<Vec<Player>, String>> {
        Box::pin(async move { crate::liveclient::Client::player_list(self).await })
    }

    fn active_player_name<'a>(&'a self) -> BoxFut<'a, Result<String, String>> {
        Box::pin(async move { crate::liveclient::Client::active_player_name(self).await })
    }
}

/* ── 输出视图模型 ── */

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentMatch {
    pub queue_short: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub queue_name: String,
    pub time_short: String,
    #[serde(skip_serializing_if = "is_zero_i64", default)]
    pub game_creation: i64,
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub champion_id: i32,
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSlot {
    pub filled: bool,
    pub is_self: bool,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub puuid: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub summoner_id: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub game_name: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub tag_line: String,
    #[serde(rename = "profileIconId", skip_serializing_if = "is_zero_i32", default)]
    pub profile_icon_id: i32,
    #[serde(rename = "championId", skip_serializing_if = "is_zero_i32", default)]
    pub champion_id: i32,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub solo: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub flex: String,
    #[serde(skip_serializing_if = "is_zero_f64", default)]
    pub win_rate: f64,
    #[serde(rename = "winRateSample", skip_serializing_if = "is_zero_i32", default)]
    pub win_rate_sample: i32,
    #[serde(skip_serializing_if = "is_zero_f64", default)]
    pub avg_kda: f64,
    #[serde(skip_serializing_if = "is_zero_f64", default)]
    pub rating: f64,
    #[serde(rename = "hiddenCareer", skip_serializing_if = "is_false", default)]
    pub hidden_career: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub recent: Vec<RecentMatch>,
}

fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}
fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}
fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeamView {
    pub key: String,
    pub label: String,
    pub side_text: String,
    pub badge: String,
    pub player_count: i32,
    pub phase_label: String,
    pub win_rate: f64,
    pub comp_score: f64,
    pub rating: i32,
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

pub fn phase_label_cn(phase: &str) -> &'static str {
    match phase {
        "None" | "" => "大厅中",
        "Lobby" => "房间内",
        "Matchmaking" => "匹配中",
        "ReadyCheck" => "接受对局",
        "ChampSelect" => "选人中",
        "GameStart" => "游戏启动",
        "InProgress" => "游戏中",
        "WaitingForStats" | "PreEndOfGame" => "结算中",
        "EndOfGame" => "对局结束",
        "Reconnect" => "重新连接",
        _ => "大厅中",
    }
}

/* ── 内部中间态 ── */

#[derive(Debug, Clone, Default)]
struct PlayerRef {
    puuid: String,
    summoner_id: String,
    game_name: String,
    tag_line: String,
    profile_icon_id: i32,
    champion_id: i32,
    is_self: bool,
}

#[derive(Debug, Clone, Default)]
struct Career {
    recent: Vec<RecentMatch>,
    hidden: bool,
}

#[derive(Deserialize, Default)]
struct ChampEntry {
    #[serde(default)]
    id: i32,
    #[serde(default)]
    alias: String,
    #[serde(default)]
    name: String,
}

#[derive(Deserialize, Default, Clone)]
struct GsParticipant {
    #[serde(default)]
    puuid: String,
    #[serde(default, rename = "summonerId", deserialize_with = "de_flex_str")]
    summoner_id: String,
    #[serde(default, rename = "gameName")]
    game_name: String,
    #[serde(default, rename = "tagLine")]
    tag_line: String,
    #[serde(default, rename = "summonerName")]
    summoner_name: String,
    #[serde(default, rename = "profileIconId")]
    profile_icon_id: i32,
    #[serde(default, rename = "championId")]
    champion_id: i32,
}

#[derive(Deserialize, Default, Clone)]
struct GsPick {
    #[serde(default)]
    puuid: String,
    #[serde(default, rename = "championId")]
    champion_id: i32,
}

#[derive(Deserialize, Default)]
struct QueueMeta {
    #[serde(default)]
    id: i32,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "numPlayersPerTeam")]
    num_players_per_team: i32,
}

#[derive(Deserialize, Default)]
struct GameData {
    #[serde(default)]
    queue: QueueMeta,
    #[serde(default, rename = "queueId")]
    queue_id: i32,
    #[serde(default, rename = "teamOne")]
    team_one: Vec<GsParticipant>,
    #[serde(default, rename = "teamTwo")]
    team_two: Vec<GsParticipant>,
    #[serde(default, rename = "playerChampionSelections")]
    picks: Vec<GsPick>,
}

#[derive(Deserialize, Default)]
struct IdOnly {
    #[serde(default)]
    id: i32,
}

#[derive(Deserialize, Default)]
struct GameflowSession {
    #[serde(default, rename = "gameData")]
    game_data: GameData,
    #[serde(default)]
    queue: IdOnly,
    #[serde(default, rename = "queueId")]
    queue_id: i32,
}

impl GameflowSession {
    fn queue_id(&self) -> i32 {
        for q in [
            self.game_data.queue.id,
            self.game_data.queue_id,
            self.queue.id,
            self.queue_id,
        ] {
            if q > 0 {
                return q;
            }
        }
        0
    }

    fn queue_name(&self) -> String {
        let q = self.queue_id();
        if q > 0 {
            if let Some(info) = lookup_queue(q) {
                return info.name;
            }
        }
        let official = self.game_data.queue.name.trim();
        if !official.is_empty() {
            return official.to_string();
        }
        if q > 0 {
            return queue_info_for(q).name;
        }
        String::new()
    }

    fn roster(&self, self_st: &ConnStatus) -> (Vec<PlayerRef>, Vec<PlayerRef>) {
        let to_refs = |list: &[GsParticipant]| -> Vec<PlayerRef> {
            let mut out = Vec::with_capacity(list.len());
            for p in list {
                let mut g = p.game_name.clone();
                let mut t = p.tag_line.clone();
                if g.is_empty() {
                    let (a, b) = split_name(&p.summoner_name);
                    g = a;
                    t = b;
                }
                let sid = p.summoner_id.clone();
                if p.puuid.is_empty() && g.is_empty() && (sid.is_empty() || sid == "0") {
                    continue;
                }
                out.push(PlayerRef {
                    puuid: p.puuid.clone(),
                    summoner_id: sid,
                    is_self: is_self_match(&p.puuid, &g, &t, self_st),
                    game_name: g,
                    tag_line: t,
                    profile_icon_id: p.profile_icon_id,
                    champion_id: p.champion_id,
                });
            }
            out
        };

        let mut one = to_refs(&self.game_data.team_one);
        let mut two = to_refs(&self.game_data.team_two);

        let mut champ_by: HashMap<String, i32> = HashMap::new();
        let mut ordered: Vec<GsPick> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for pk in &self.game_data.picks {
            if pk.puuid.is_empty() {
                continue;
            }
            let k = pk.puuid.to_lowercase();
            champ_by.insert(k.clone(), pk.champion_id);
            if seen.insert(k) {
                ordered.push(pk.clone());
            }
        }

        let mut side: HashMap<String, usize> = HashMap::new();
        for r in &one {
            if !r.puuid.is_empty() {
                side.insert(r.puuid.to_lowercase(), 0);
            }
        }
        for r in &two {
            if !r.puuid.is_empty() {
                side.insert(r.puuid.to_lowercase(), 1);
            }
        }

        for r in one.iter_mut() {
            if r.champion_id == 0 && !r.puuid.is_empty() {
                r.champion_id = champ_by.get(&r.puuid.to_lowercase()).copied().unwrap_or(0);
            }
        }
        for r in two.iter_mut() {
            if r.champion_id == 0 && !r.puuid.is_empty() {
                r.champion_id = champ_by.get(&r.puuid.to_lowercase()).copied().unwrap_or(0);
            }
        }

        let mut per_team = self.game_data.queue.num_players_per_team;
        if per_team <= 0 {
            per_team = 5;
        }
        let mut extra_one = Vec::new();
        let mut extra_two = Vec::new();
        for (i, pk) in ordered.iter().enumerate() {
            if side.contains_key(&pk.puuid.to_lowercase()) {
                continue;
            }
            let r = PlayerRef {
                puuid: pk.puuid.clone(),
                champion_id: pk.champion_id,
                is_self: is_self_match(&pk.puuid, "", "", self_st),
                ..Default::default()
            };
            if (i as i32) < per_team {
                extra_one.push(r);
            } else {
                extra_two.push(r);
            }
        }
        one.extend(extra_one);
        two.extend(extra_two);

        if one.is_empty() && two.is_empty() {
            return (Vec::new(), Vec::new());
        }
        for r in &two {
            if r.is_self {
                return (two, one);
            }
        }
        (one, two)
    }
}

fn flex_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

fn de_flex_str<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Value::deserialize(d)?;
    Ok(flex_value(&v))
}

#[derive(Deserialize, Default)]
struct CsPlayer {
    #[serde(default)]
    puuid: String,
    #[serde(default, rename = "summonerId", deserialize_with = "de_flex_str")]
    summoner_id: String,
    #[serde(default, rename = "championId")]
    champion_id: i32,
    #[serde(default, rename = "gameName")]
    game_name: String,
    #[serde(default, rename = "tagLine")]
    tag_line: String,
    #[serde(default, rename = "profileIconId")]
    profile_icon_id: i32,
}

#[derive(Deserialize, Default)]
struct ChampSelectBody {
    #[serde(default, rename = "myTeam")]
    my_team: Vec<CsPlayer>,
    #[serde(default, rename = "theirTeam")]
    their_team: Vec<CsPlayer>,
}

#[derive(Deserialize, Default)]
struct LobbyBody {
    #[serde(default, rename = "gameQueueConfig")]
    game_queue_config: IdQueue,
    #[serde(default)]
    members: Vec<LobbyMember>,
}

#[derive(Deserialize, Default)]
struct IdQueue {
    #[serde(default, rename = "queueId")]
    queue_id: i32,
}

#[derive(Deserialize, Default)]
struct LobbyMember {
    #[serde(default)]
    puuid: String,
    #[serde(default, rename = "summonerId", deserialize_with = "de_flex_str")]
    summoner_id: String,
    #[serde(default, rename = "gameName")]
    game_name: String,
    #[serde(default, rename = "tagLine")]
    tag_line: String,
    #[serde(default, rename = "profileIconId")]
    profile_icon_id: i32,
    #[serde(default)]
    team: i32,
}

#[derive(Deserialize, Default)]
struct SummonerRaw {
    #[serde(default)]
    puuid: String,
    #[serde(default, rename = "gameName")]
    game_name: String,
    #[serde(default, rename = "tagLine")]
    tag_line: String,
    #[serde(default, rename = "displayName")]
    display_name: String,
    #[serde(default, rename = "profileIconId")]
    profile_icon_id: i32,
    #[serde(default, rename = "summonerId", deserialize_with = "de_flex_str")]
    summoner_id: String,
}

impl SummonerRaw {
    fn apply(&self, r: &mut PlayerRef) {
        if r.game_name.is_empty() {
            if !self.game_name.is_empty() {
                r.game_name = self.game_name.clone();
                r.tag_line = self.tag_line.clone();
            } else if !self.display_name.is_empty() {
                let (g, t) = split_name(&self.display_name);
                r.game_name = g;
                r.tag_line = t;
            }
        }
        if r.profile_icon_id == 0 && self.profile_icon_id != 0 {
            r.profile_icon_id = self.profile_icon_id;
        }
        if (r.summoner_id.is_empty() || r.summoner_id == "0")
            && !self.summoner_id.is_empty()
            && self.summoner_id != "0"
        {
            r.summoner_id = self.summoner_id.clone();
        }
        if r.puuid.is_empty() && !self.puuid.is_empty() {
            r.puuid = self.puuid.clone();
        }
    }

    fn valid(&self) -> bool {
        !(self.puuid.is_empty()
            && self.game_name.is_empty()
            && self.display_name.is_empty()
            && self.profile_icon_id == 0)
    }
}

/* ── 服务 ── */

struct ChampIndexState {
    at: Option<Instant>,
    map: HashMap<String, i32>,
}

struct CareerState {
    limit: i32,
    cache: HashMap<String, (Instant, Career)>,
}

pub struct GameinfoService {
    http: Arc<dyn LcuHttp>,
    hist: Arc<dyn HistApi>,
    live: Arc<dyn LiveApi>,
    champ: Mutex<ChampIndexState>,
    career: Mutex<CareerState>,
}

impl GameinfoService {
    pub fn new(http: Arc<dyn LcuHttp>, hist: Arc<dyn HistApi>, live: Arc<dyn LiveApi>) -> Self {
        Self {
            http,
            hist,
            live,
            champ: Mutex::new(ChampIndexState {
                at: None,
                map: HashMap::new(),
            }),
            career: Mutex::new(CareerState {
                limit: DEFAULT_CAREER_LIMIT,
                cache: HashMap::new(),
            }),
        }
    }

    pub fn set_career_limit(&self, n: i32) {
        if !(5..=50).contains(&n) {
            return;
        }
        let mut g = self.career.lock().unwrap();
        if g.limit != n {
            g.limit = n;
            g.cache.clear();
        }
    }

    pub fn current_career_limit(&self) -> i32 {
        let g = self.career.lock().unwrap();
        if g.limit <= 0 {
            DEFAULT_CAREER_LIMIT
        } else {
            g.limit
        }
    }

    pub async fn get_gameflow_state(&self, queue_filter: Vec<i32>) -> Result<ViewState, String> {
        let st = self.http.status().await;
        if st.state != crate::lcu::State::Connected {
            return Err(ERR_NOT_CONNECTED.to_string());
        }

        let phase = self.fetch_phase().await;
        let label = phase_label_cn(&phase);
        let mut queue_label = String::new();
        let mut queue_id = 0;
        let (mut ally, mut enemy): (Vec<PlayerRef>, Vec<PlayerRef>);

        match phase.as_str() {
            "Lobby" | "Matchmaking" | "ReadyCheck" => {
                let (a, e, q) = self.load_lobby(&st).await;
                ally = a;
                enemy = e;
                queue_id = q;
                if queue_id > 0 {
                    queue_label = queue_info_for(queue_id).name;
                }
            }
            "ChampSelect" => {
                let (mut ea, mut ee) = self.load_champ_select_split(&st).await;
                if let Some(sess) = self.fetch_gameflow_session().await {
                    queue_id = sess.queue_id();
                    queue_label = sess.queue_name();
                    let (one, two) = sess.roster(&st);
                    ea = backfill_from_roster(ea, one.clone());
                    enrich_from_roster(&mut ee, &[one, two]);
                }
                ally = ea;
                enemy = ee;
            }
            "GameStart" | "InProgress" | "WaitingForStats" | "PreEndOfGame" | "EndOfGame"
            | "Reconnect" => {
                let mut queue_label_opt: Option<String> = None;
                if let Some(sess) = self.fetch_gameflow_session().await {
                    queue_id = sess.queue_id();
                    queue_label_opt = Some(sess.queue_name());
                    let (a, e) = sess.roster(&st);
                    ally = a;
                    enemy = e;
                } else {
                    ally = Vec::new();
                    enemy = Vec::new();
                }
                queue_label = queue_label_opt.unwrap_or_default();
                if ally.is_empty() || enemy.is_empty() {
                    let (a2, e2) = self.load_live(&st).await;
                    if a2.len() + e2.len() > ally.len() + enemy.len() {
                        ally = a2;
                        enemy = e2;
                    }
                }
            }
            _ => {
                ally = Vec::new();
                enemy = Vec::new();
            }
        }

        let filter = resolve_queue_filter(&queue_filter, queue_id);
        let ally_slots = self.build_slots(&ally, &filter).await;
        let enemy_slots = self.build_slots(&enemy, &filter).await;

        Ok(ViewState {
            phase,
            queue_label,
            queue_id,
            teams: vec![
                sum_team("ally", "我方", "蓝方·房间", "我方", label, ally_slots),
                sum_team("enemy", "敌方", "红方", "敌方", label, enemy_slots),
            ],
        })
    }

    async fn fetch_phase(&self) -> String {
        let Ok((status, body)) = self.http.get(PATH_GAMEFLOW_PHASE).await else {
            return "None".into();
        };
        if status != 200 {
            return "None".into();
        }
        match serde_json::from_slice::<String>(&body) {
            Ok(p) if !p.is_empty() => p,
            _ => "None".into(),
        }
    }

    async fn fetch_gameflow_session(&self) -> Option<GameflowSession> {
        let (status, body) = self.http.get(PATH_GAMEFLOW_SESSION).await.ok()?;
        if status != 200 {
            return None;
        }
        serde_json::from_slice(&body).ok()
    }

    async fn load_lobby(&self, self_st: &ConnStatus) -> (Vec<PlayerRef>, Vec<PlayerRef>, i32) {
        let Ok((status, body)) = self.http.get(PATH_LOBBY).await else {
            return (Vec::new(), Vec::new(), 0);
        };
        if status != 200 {
            return (Vec::new(), Vec::new(), 0);
        }
        let Ok(lb) = serde_json::from_slice::<LobbyBody>(&body) else {
            return (Vec::new(), Vec::new(), 0);
        };
        let mut ally = Vec::new();
        let mut enemy = Vec::new();
        for m in &lb.members {
            let r = PlayerRef {
                puuid: m.puuid.clone(),
                summoner_id: m.summoner_id.clone(),
                game_name: m.game_name.clone(),
                tag_line: m.tag_line.clone(),
                profile_icon_id: m.profile_icon_id,
                is_self: is_self_match(&m.puuid, &m.game_name, &m.tag_line, self_st),
                ..Default::default()
            };
            if m.team == 2 {
                enemy.push(r);
            } else {
                ally.push(r);
            }
        }
        (ally, enemy, lb.game_queue_config.queue_id)
    }

    async fn load_champ_select_split(
        &self,
        self_st: &ConnStatus,
    ) -> (Vec<PlayerRef>, Vec<PlayerRef>) {
        let Ok((status, body)) = self.http.get(PATH_CHAMP_SELECT_SESSION).await else {
            return (Vec::new(), Vec::new());
        };
        if status != 200 {
            return (Vec::new(), Vec::new());
        }
        let Ok(cs) = serde_json::from_slice::<ChampSelectBody>(&body) else {
            return (Vec::new(), Vec::new());
        };
        let conv = |list: &[CsPlayer], is_ally: bool| -> Vec<PlayerRef> {
            let mut out = Vec::new();
            for p in list {
                if p.puuid.is_empty() && (p.summoner_id.is_empty() || p.summoner_id == "0") {
                    continue;
                }
                out.push(PlayerRef {
                    puuid: p.puuid.clone(),
                    summoner_id: p.summoner_id.clone(),
                    game_name: p.game_name.clone(),
                    tag_line: p.tag_line.clone(),
                    profile_icon_id: p.profile_icon_id,
                    champion_id: p.champion_id,
                    is_self: is_ally && is_self_match(&p.puuid, &p.game_name, &p.tag_line, self_st),
                });
            }
            out
        };
        (conv(&cs.my_team, true), conv(&cs.their_team, false))
    }

    async fn load_live(&self, self_st: &ConnStatus) -> (Vec<PlayerRef>, Vec<PlayerRef>) {
        let list = match self.live.player_list().await {
            Ok(l) => l,
            Err(e) => {
                log::warn!("[gameinfo] live playerlist unavailable: {e}");
                return (Vec::new(), Vec::new());
            }
        };
        let active_name = self.live.active_player_name().await.unwrap_or_default();
        let (active_game, active_tag) = split_name(&active_name);
        let match_fn = |puuid: &str, g: &str, t: &str| -> bool {
            if is_self_match(puuid, g, t, self_st) {
                return true;
            }
            !active_game.is_empty()
                && g.eq_ignore_ascii_case(&active_game)
                && tag_equal(t, &active_tag)
        };

        let mut self_team = TeamId::None;
        for p in &list {
            let (g, t) = p.name();
            if match_fn(&p.puuid, &g, &t) {
                self_team = p.team;
                break;
            }
        }

        let mut ally = Vec::new();
        let mut enemy = Vec::new();
        for p in &list {
            let (g, t) = p.name();
            let r = PlayerRef {
                puuid: p.puuid.clone(),
                game_name: g.clone(),
                tag_line: t.clone(),
                champion_id: self.champion_id_by_name(&p.champion_name).await,
                is_self: match_fn(&p.puuid, &g, &t),
                ..Default::default()
            };
            let team = p.team;
            let goes_enemy = if self_team != TeamId::None {
                team != self_team
            } else {
                team == TeamId::Red
            };
            if goes_enemy {
                enemy.push(r);
            } else {
                ally.push(r);
            }
        }
        if ally.len() == 10 && enemy.is_empty() {
            let mid = ally.len() / 2;
            let rest = ally.split_off(mid);
            enemy = rest;
        } else if enemy.len() == 10 && ally.is_empty() {
            let mid = enemy.len() / 2;
            let rest = enemy.split_off(mid);
            ally = rest;
        }
        (ally, enemy)
    }

    async fn build_slots(&self, refs: &[PlayerRef], filter: &[i32]) -> Vec<PlayerSlot> {
        let mut refs = refs.to_vec();

        // ① 标识互查
        let incomplete: Vec<usize> = refs
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.puuid.is_empty()
                    || r.game_name.is_empty()
                    || r.profile_icon_id == 0
                    || r.summoner_id.is_empty()
            })
            .map(|(i, _)| i)
            .collect();
        let filled = join_all(incomplete.into_iter().map(|i| {
            let mut r = refs[i].clone();
            async move {
                self.fill_identity(&mut r).await;
                (i, r)
            }
        }))
        .await;
        for (i, r) in filled {
            refs[i] = r;
        }

        // ② 段位批量
        let mut ids: Vec<String> = Vec::with_capacity(refs.len() * 2);
        for r in &refs {
            if !r.summoner_id.is_empty() {
                ids.push(r.summoner_id.clone());
            }
            if !r.puuid.is_empty() {
                ids.push(r.puuid.clone());
            }
        }
        let mut rank_map: HashMap<String, RankedInfo> = HashMap::new();
        if !ids.is_empty() {
            if let Ok(rows) = self.hist.get_players_ranked(ids).await {
                for r in rows {
                    if !r.summoner_id.is_empty() {
                        rank_map.insert(r.summoner_id.clone(), r.clone());
                    }
                    if !r.puuid.is_empty() {
                        rank_map.insert(r.puuid.clone(), r);
                    }
                }
            }
        }

        // ③ 近况并发
        let filter_vec = filter.to_vec();
        let careers = join_all(refs.iter().map(|r| {
            let filter = filter_vec.clone();
            async move {
                if r.puuid.is_empty() {
                    return Career {
                        recent: Vec::new(),
                        hidden: true,
                    };
                }
                self.fetch_career(&r.puuid, &filter).await
            }
        }))
        .await;

        let mut slots = Vec::with_capacity(refs.len().max(5));
        for (i, r) in refs.iter().enumerate() {
            let c = &careers[i];
            let mut slot = PlayerSlot {
                filled: true,
                is_self: r.is_self,
                puuid: r.puuid.clone(),
                summoner_id: r.summoner_id.clone(),
                game_name: r.game_name.clone(),
                tag_line: r.tag_line.clone(),
                profile_icon_id: r.profile_icon_id,
                champion_id: r.champion_id,
                hidden_career: c.hidden,
                recent: c.recent.clone(),
                ..Default::default()
            };
            if let Some(rk) = rank_map
                .get(&r.summoner_id)
                .or_else(|| rank_map.get(&r.puuid))
            {
                let (solo, flex) = pick_rank(rk);
                slot.solo = solo;
                slot.flex = flex;
            }
            let (wr, sample, avg, rating) = career_stats(&c.recent);
            slot.win_rate = wr;
            slot.win_rate_sample = sample;
            slot.avg_kda = avg;
            slot.rating = rating;
            slots.push(slot);
        }
        while slots.len() < 5 {
            slots.push(PlayerSlot::default());
        }
        slots
    }

    async fn fill_identity(&self, r: &mut PlayerRef) {
        if !r.puuid.is_empty() {
            let path = PATH_SUMMONER_BY_PUUID.replace("%s", &path_escape(&r.puuid));
            if let Some(raw) = self.lookup_summoner(&path).await {
                raw.apply(r);
            }
        }
        if (r.game_name.is_empty() || r.profile_icon_id == 0)
            && !r.summoner_id.is_empty()
            && r.summoner_id != "0"
        {
            let path = PATH_SUMMONER_BY_ID.replace("%s", &path_escape(&r.summoner_id));
            if let Some(raw) = self.lookup_summoner(&path).await {
                raw.apply(r);
            }
        }
        if (r.puuid.is_empty() || r.profile_icon_id == 0) && !r.game_name.is_empty() {
            let mut name = r.game_name.clone();
            if !r.tag_line.is_empty() {
                name.push('#');
                name.push_str(&r.tag_line);
            }
            if let Ok(res) = self.hist.search_summoner(name).await {
                SummonerRaw {
                    puuid: res.puuid,
                    game_name: res.game_name,
                    tag_line: res.tag_line,
                    profile_icon_id: res.profile_icon_id,
                    display_name: String::new(),
                    summoner_id: res.summoner_id,
                }
                .apply(r);
            }
        }
    }

    async fn lookup_summoner(&self, path: &str) -> Option<SummonerRaw> {
        let (status, body) = self.http.get(path).await.ok()?;
        if !(200..300).contains(&status) {
            return None;
        }
        let raw: SummonerRaw = serde_json::from_slice(&body).ok()?;
        raw.valid().then_some(raw)
    }

    async fn fetch_career(&self, puuid: &str, filter: &[i32]) -> Career {
        let key = format!("{puuid}|{filter:?}");
        if let Some(c) = self.cached_career(&key) {
            return c;
        }
        let c = self.load_career(puuid, filter).await;
        self.store_career(&key, c.clone());
        c
    }

    fn cached_career(&self, key: &str) -> Option<Career> {
        let g = self.career.lock().unwrap();
        g.cache
            .get(key)
            .filter(|(at, _)| at.elapsed() < CAREER_TTL)
            .map(|(_, c)| c.clone())
    }

    fn store_career(&self, key: &str, c: Career) {
        let mut g = self.career.lock().unwrap();
        if g.cache.len() >= CAREER_CACHE_MAX {
            let now = Instant::now();
            g.cache
                .retain(|_, (at, _)| now.duration_since(*at) < CAREER_TTL);
            if g.cache.len() >= CAREER_CACHE_MAX {
                g.cache.clear();
            }
        }
        g.cache.insert(key.to_string(), (Instant::now(), c));
    }

    async fn load_career(&self, puuid: &str, filter: &[i32]) -> Career {
        let limit = self.current_career_limit();
        let mut out: Vec<RecentMatch> = Vec::with_capacity(limit as usize);
        for page in 0..CAREER_SCAN_PAGES {
            match self.hist.get_matches(puuid.to_string(), page).await {
                Err(_) => {
                    if page == 0 {
                        return Career {
                            recent: Vec::new(),
                            hidden: true,
                        };
                    }
                    break;
                }
                Ok(p) => {
                    for m in &p.summaries {
                        if !queue_matches(m.queue_id, filter) {
                            continue;
                        }
                        out.push(RecentMatch {
                            queue_short: m.queue_short.clone(),
                            queue_name: m.queue_name.clone(),
                            time_short: m.short_time.clone(),
                            game_creation: m.game_creation,
                            win: m.win,
                            kills: m.kills,
                            deaths: m.deaths,
                            assists: m.assists,
                            champion_id: m.champion_id,
                        });
                        if out.len() as i32 >= limit {
                            return Career {
                                recent: out,
                                hidden: false,
                            };
                        }
                    }
                    if !p.has_more || p.summaries.is_empty() {
                        break;
                    }
                }
            }
        }
        Career {
            recent: out,
            hidden: false,
        }
    }

    async fn champion_id_by_name(&self, name: &str) -> i32 {
        let name = name.trim().to_lowercase();
        if name.is_empty() {
            return 0;
        }
        let idx = self.get_champ_index().await;
        idx.get(&name).copied().unwrap_or(0)
    }

    async fn get_champ_index(&self) -> HashMap<String, i32> {
        {
            let g = self.champ.lock().unwrap();
            if let Some(at) = g.at {
                if at.elapsed() < CHAMP_INDEX_TTL {
                    return g.map.clone();
                }
            }
        }
        let mut map = HashMap::new();
        if let Ok((status, body)) = self.http.get(PATH_GD_CHAMPION_SUMMARY).await {
            if status == 200 {
                if let Ok(list) = serde_json::from_slice::<Vec<ChampEntry>>(&body) {
                    for e in list {
                        if e.id <= 0 {
                            continue;
                        }
                        if !e.alias.is_empty() {
                            map.insert(e.alias.to_lowercase(), e.id);
                        }
                        if !e.name.is_empty() {
                            map.insert(e.name.to_lowercase(), e.id);
                        }
                    }
                }
            }
        }
        if !map.is_empty() {
            let mut g = self.champ.lock().unwrap();
            g.map = map.clone();
            g.at = Some(Instant::now());
        }
        map
    }
}

/* ── 纯函数 ── */

fn split_name(name: &str) -> (String, String) {
    let name = name.trim();
    if let Some(i) = name.find('#') {
        (name[..i].to_string(), name[i + 1..].to_string())
    } else {
        (name.to_string(), String::new())
    }
}

fn tag_equal(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return true;
    }
    a.eq_ignore_ascii_case(b)
}

fn is_self_match(puuid: &str, game_name: &str, tag: &str, self_st: &ConnStatus) -> bool {
    if !puuid.is_empty() {
        if let Some(sp) = &self_st.puuid {
            if !sp.is_empty() && puuid == sp {
                return true;
            }
        }
    }
    if let (Some(sg), Some(st)) = (&self_st.game_name, &self_st.tag_line) {
        if !sg.is_empty()
            && !game_name.is_empty()
            && game_name.eq_ignore_ascii_case(sg)
            && tag_equal(tag, st)
        {
            return true;
        }
    }
    false
}

fn pick_rank(rk: &RankedInfo) -> (String, String) {
    let solo = rk.solo.trim();
    let flex = rk.flex.trim();
    let solo = if solo.is_empty() || solo == "未定级" {
        String::new()
    } else {
        solo.to_string()
    };
    let flex = if flex.is_empty() || flex == "未定级" {
        String::new()
    } else {
        flex.to_string()
    };
    (solo, flex)
}

fn career_stats(recent: &[RecentMatch]) -> (f64, i32, f64, f64) {
    let sample = recent.len() as i32;
    if sample == 0 {
        return (0.0, 0, 0.0, 0.0);
    }
    let mut wins = 0;
    let mut kda_sum = 0.0f64;
    for r in recent {
        if r.win {
            wins += 1;
        }
        kda_sum += (r.kills + r.assists) as f64 / (r.deaths as f64).max(1.0);
    }
    let win_rate = round1(wins as f64 / sample as f64 * 100.0);
    let avg_kda = round2(kda_sum / sample as f64);
    let rating = round1(win_rate / 20.0 + avg_kda);
    (win_rate, sample, avg_kda, rating)
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn sum_team(
    key: &str,
    label: &str,
    side_text: &str,
    badge: &str,
    phase_label: &str,
    slots: Vec<PlayerSlot>,
) -> TeamView {
    let mut n = 0;
    let mut wr_sum = 0.0;
    let mut r_sum = 0.0;
    for s in &slots {
        if !s.filled {
            continue;
        }
        n += 1;
        wr_sum += s.win_rate;
        r_sum += s.rating;
    }
    let mut tv = TeamView {
        key: key.into(),
        label: label.into(),
        side_text: side_text.into(),
        badge: badge.into(),
        player_count: n,
        phase_label: phase_label.into(),
        win_rate: 0.0,
        comp_score: 0.0,
        rating: 0,
        slots,
    };
    if n > 0 {
        tv.win_rate = round1(wr_sum / n as f64);
        tv.comp_score = tv.win_rate;
        tv.rating = (r_sum / n as f64 * 10.0).round() as i32;
    }
    tv
}

fn resolve_queue_filter(queue_filter: &[i32], current_queue_id: i32) -> Vec<i32> {
    if queue_filter.len() == 1 && queue_filter[0] < 0 {
        if current_queue_id > 0 {
            return vec![current_queue_id];
        }
        return Vec::new();
    }
    queue_filter.to_vec()
}

fn queue_matches(queue_id: i32, filter: &[i32]) -> bool {
    if filter.is_empty() {
        return true;
    }
    filter.contains(&queue_id)
}

fn enrich_from_roster(refs: &mut [PlayerRef], teams: &[Vec<PlayerRef>]) {
    let mut idx: HashMap<String, PlayerRef> = HashMap::new();
    for list in teams {
        for r in list {
            if !r.puuid.is_empty() {
                idx.insert(format!("p:{}", r.puuid), r.clone());
            }
            if !r.summoner_id.is_empty() && r.summoner_id != "0" {
                idx.insert(format!("s:{}", r.summoner_id), r.clone());
            }
        }
    }
    for r in refs.iter_mut() {
        let mut src = None;
        if !r.puuid.is_empty() {
            src = idx.get(&format!("p:{}", r.puuid)).cloned();
        }
        if src.is_none() && !r.summoner_id.is_empty() && r.summoner_id != "0" {
            src = idx.get(&format!("s:{}", r.summoner_id)).cloned();
        }
        let Some(src) = src else { continue };
        if r.game_name.is_empty() {
            r.game_name = src.game_name;
            r.tag_line = src.tag_line;
        }
        if r.profile_icon_id == 0 {
            r.profile_icon_id = src.profile_icon_id;
        }
        if r.summoner_id.is_empty() || r.summoner_id == "0" {
            r.summoner_id = src.summoner_id;
        }
        if r.puuid.is_empty() {
            r.puuid = src.puuid;
        }
        if r.champion_id == 0 {
            r.champion_id = src.champion_id;
        }
    }
}

fn backfill_from_roster(mut refs: Vec<PlayerRef>, src: Vec<PlayerRef>) -> Vec<PlayerRef> {
    enrich_from_roster(&mut refs, std::slice::from_ref(&src));
    if src.is_empty() {
        return refs;
    }
    let mut have_p = HashSet::new();
    let mut have_s = HashSet::new();
    for r in &refs {
        if !r.puuid.is_empty() {
            have_p.insert(r.puuid.clone());
        }
        if !r.summoner_id.is_empty() && r.summoner_id != "0" {
            have_s.insert(r.summoner_id.clone());
        }
    }
    for r in src {
        if !r.puuid.is_empty() && have_p.contains(&r.puuid) {
            continue;
        }
        if !r.summoner_id.is_empty() && r.summoner_id != "0" && have_s.contains(&r.summoner_id) {
            continue;
        }
        if !r.puuid.is_empty() {
            have_p.insert(r.puuid.clone());
        }
        if !r.summoner_id.is_empty() && r.summoner_id != "0" {
            have_s.insert(r.summoner_id.clone());
        }
        refs.push(r);
    }
    refs
}

/* ── 测试 ── */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcu::State;
    use crate::service::http::FakeHttp;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Default)]
    struct FakeHist {
        ranked: HashMap<String, RankedInfo>,
        matches: HashMap<String, Vec<MatchSummary>>,
        pages: Option<HashMap<String, Vec<Vec<MatchSummary>>>>,
        match_err: HashSet<String>,
        summoners: HashMap<String, SummonerResult>,
    }

    impl HistApi for FakeHist {
        fn get_players_ranked<'a>(
            &'a self,
            ids: Vec<String>,
        ) -> BoxFut<'a, Result<Vec<RankedInfo>, String>> {
            Box::pin(async move {
                let mut out = Vec::new();
                for id in ids {
                    if let Some(r) = self.ranked.get(&id) {
                        out.push(r.clone());
                    }
                }
                Ok(out)
            })
        }

        fn get_matches<'a>(
            &'a self,
            puuid: String,
            page: i32,
        ) -> BoxFut<'a, Result<MatchPage, String>> {
            Box::pin(async move {
                if self.match_err.contains(&puuid) {
                    return Err("career hidden".into());
                }
                if let Some(pages) = &self.pages {
                    let pgs = pages.get(&puuid).cloned().unwrap_or_default();
                    if page as usize >= pgs.len() {
                        return Ok(MatchPage {
                            puuid: puuid.clone(),
                            page,
                            has_more: false,
                            ..Default::default()
                        });
                    }
                    return Ok(MatchPage {
                        puuid: puuid.clone(),
                        page,
                        summaries: pgs[page as usize].clone(),
                        has_more: (page as usize) < pgs.len() - 1,
                        ..Default::default()
                    });
                }
                Ok(MatchPage {
                    puuid: puuid.clone(),
                    page,
                    summaries: self.matches.get(&puuid).cloned().unwrap_or_default(),
                    ..Default::default()
                })
            })
        }

        fn search_summoner<'a>(
            &'a self,
            name: String,
        ) -> BoxFut<'a, Result<SummonerResult, String>> {
            Box::pin(async move {
                self.summoners
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| "not found".to_string())
            })
        }
    }

    #[derive(Default)]
    struct FakeLive {
        players: Vec<Player>,
        active: String,
        err: Option<String>,
    }

    impl LiveApi for FakeLive {
        fn player_list<'a>(&'a self) -> BoxFut<'a, Result<Vec<Player>, String>> {
            Box::pin(async move {
                match &self.err {
                    Some(e) => Err(e.clone()),
                    None => Ok(self.players.clone()),
                }
            })
        }

        fn active_player_name<'a>(&'a self) -> BoxFut<'a, Result<String, String>> {
            Box::pin(async move {
                match &self.err {
                    Some(e) => Err(e.clone()),
                    None => Ok(self.active.clone()),
                }
            })
        }
    }

    fn connected_self() -> ConnStatus {
        ConnStatus {
            state: State::Connected,
            puuid: Some("PSELF".into()),
            game_name: Some("我".into()),
            tag_line: Some("CN1".into()),
            profile_icon_id: Some(7),
            ..Default::default()
        }
    }

    fn new_test(http: FakeHttp, hist: FakeHist, live: FakeLive) -> GameinfoService {
        GameinfoService::new(Arc::new(http), Arc::new(hist), Arc::new(live))
    }

    fn routes_from(pairs: &[(&str, &str)]) -> FakeHttp {
        let mut map = HashMap::new();
        for (k, v) in pairs {
            map.insert(k.to_string(), (200u16, v.as_bytes().to_vec()));
        }
        FakeHttp::new(connected_self()).with_routes(map)
    }

    fn mk_sum(qid: i32, short: &str, win: bool, k: i32, d: i32, a: i32) -> MatchSummary {
        MatchSummary {
            game_id: (k * 1000 + d * 100 + a) as i64,
            queue_id: qid,
            queue_short: short.into(),
            win,
            kills: k,
            deaths: d,
            assists: a,
            champion_id: 1,
            ..Default::default()
        }
    }

    fn mixed_pages() -> Vec<Vec<MatchSummary>> {
        let mut p0 = Vec::new();
        for i in 0..10 {
            p0.push(mk_sum(2400, "海斗", i % 2 == 0, 5, 2, 3));
        }
        for _ in 0..5 {
            p0.push(mk_sum(420, "单双", true, 1, 1, 1));
        }
        let mut p1 = Vec::new();
        for _ in 0..12 {
            p1.push(mk_sum(2400, "海斗", true, 2, 2, 2));
        }
        for _ in 0..3 {
            p1.push(mk_sum(420, "单双", false, 0, 3, 0));
        }
        let mut p2 = Vec::new();
        for _ in 0..10 {
            p2.push(mk_sum(2400, "海斗", true, 9, 0, 9));
        }
        vec![p0, p1, p2]
    }

    #[test]
    fn phase_label_all() {
        let cases = [
            ("", "大厅中"),
            ("None", "大厅中"),
            ("Lobby", "房间内"),
            ("Matchmaking", "匹配中"),
            ("ReadyCheck", "接受对局"),
            ("ChampSelect", "选人中"),
            ("GameStart", "游戏启动"),
            ("InProgress", "游戏中"),
            ("WaitingForStats", "结算中"),
            ("PreEndOfGame", "结算中"),
            ("EndOfGame", "对局结束"),
            ("Reconnect", "重新连接"),
            ("Weird", "大厅中"),
        ];
        for (in_, want) in cases {
            assert_eq!(phase_label_cn(in_), want, "in={in_}");
        }
    }

    #[test]
    fn helpers_split_tag_rank() {
        assert_eq!(split_name("a#b"), ("a".into(), "b".into()));
        assert_eq!(split_name("x"), ("x".into(), "".into()));
        assert!(tag_equal("", "t"));
        assert!(tag_equal("T1", "t1"));
        assert!(!tag_equal("t1", "t2"));
        let rk = RankedInfo {
            solo: "黄金 IV 45".into(),
            flex: "未定级".into(),
            ..Default::default()
        };
        assert_eq!(pick_rank(&rk), ("黄金 IV 45".into(), "".into()));
        let (wr, sample, avg, rating) = career_stats(&[
            RecentMatch {
                win: true,
                kills: 5,
                deaths: 1,
                assists: 5,
                ..Default::default()
            },
            RecentMatch {
                win: false,
                kills: 1,
                deaths: 4,
                assists: 2,
                ..Default::default()
            },
        ]);
        assert_eq!(wr, 50.0);
        assert_eq!(sample, 2);
        assert_eq!(avg, 5.38);
        assert_eq!(rating, 7.9);
        assert_eq!(resolve_queue_filter(&[-1], 2400), vec![2400]);
        assert!(resolve_queue_filter(&[-1], 0).is_empty());
        assert_eq!(resolve_queue_filter(&[420], 2400), vec![420]);
        assert!(queue_matches(2400, &[]));
        assert!(queue_matches(2400, &[2400]));
        assert!(!queue_matches(420, &[2400]));
    }

    #[tokio::test]
    async fn lobby_state() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""Lobby""#),
            (
                PATH_LOBBY,
                r#"{
                    "gameQueueConfig": {"queueId": 420},
                    "members": [
                        {"puuid":"PSELF","summonerId":1001,"gameName":"我","tagLine":"CN1","profileIconId":7,"team":1},
                        {"puuid":"P2","summonerId":"1002","gameName":"队友","tagLine":"CN2","profileIconId":8,"team":1},
                        {"puuid":"P3","summonerId":1003,"gameName":"敌1","tagLine":"CN3","profileIconId":9,"team":2}
                    ]
                }"#,
            ),
        ]);
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.phase, "Lobby");
        assert_eq!(st.teams[0].phase_label, "房间内");
        assert_eq!(st.queue_label, queue_info_for(420).name);
        assert_eq!(st.teams[0].player_count, 2);
        assert_eq!(st.teams[1].player_count, 1);
        assert!(st.teams[0].slots[0].is_self);
        assert_eq!(st.teams[0].slots[0].game_name, "我");
        assert_eq!(st.teams[0].slots[0].summoner_id, "1001");
        assert_eq!(st.teams[0].slots[1].summoner_id, "1002");
        assert_eq!(st.teams[0].slots.len(), 5);
        assert_eq!(st.teams[1].slots.len(), 5);
    }

    #[tokio::test]
    async fn champ_select_state() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""ChampSelect""#),
            (
                PATH_CHAMP_SELECT_SESSION,
                r#"{
                    "myTeam": [
                        {"puuid":"PSELF","summonerId":1,"championId":103},
                        {"puuid":"P2","summonerId":2,"championId":0}
                    ],
                    "theirTeam": [
                        {"puuid":"P9","summonerId":9,"championId":22},
                        {"puuid":"","summonerId":0,"championId":0}
                    ]
                }"#,
            ),
            (PATH_GAMEFLOW_SESSION, r#"{"queueId": 420}"#),
            (
                "/lol-summoner/v1/summoners/by-puuid/P2",
                r#"{"puuid":"P2","gameName":"队友","tagLine":"T2","profileIconId":33,"summonerId":"2"}"#,
            ),
            (
                "/lol-summoner/v1/summoners/by-puuid/P9",
                r#"{"puuid":"P9","gameName":"敌人","tagLine":"T9","profileIconId":44,"summonerId":"9"}"#,
            ),
        ]);
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.teams[0].phase_label, "选人中");
        assert_eq!(st.teams[0].player_count, 2);
        assert_eq!(st.teams[1].player_count, 1);
        assert_eq!(st.teams[0].slots[0].champion_id, 103);
        assert_eq!(st.teams[1].slots[0].champion_id, 22);
    }

    #[tokio::test]
    async fn champ_select_ally_backfill() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""ChampSelect""#),
            (
                PATH_CHAMP_SELECT_SESSION,
                r#"{"myTeam":[
                    {"puuid":"PSELF","summonerId":1,"championId":103},
                    {"puuid":"P2","summonerId":2,"championId":1},
                    {"puuid":"P3","summonerId":3,"championId":2},
                    {"puuid":"P4","summonerId":4,"championId":3},
                    {"puuid":"","summonerId":0,"championId":4}
                ],"theirTeam":[{"puuid":"","summonerId":0}]}"#,
            ),
            (
                PATH_GAMEFLOW_SESSION,
                r#"{"queueId":420,"gameData":{"teamOne":[
                    {"puuid":"PSELF","summonerId":1,"gameName":"我","tagLine":"CN1","profileIconId":7},
                    {"puuid":"P2","summonerId":2,"gameName":"队2","tagLine":"T2","profileIconId":8},
                    {"puuid":"P3","summonerId":3,"gameName":"队3","tagLine":"T3","profileIconId":9},
                    {"puuid":"P4","summonerId":4,"gameName":"队4","tagLine":"T4","profileIconId":10},
                    {"puuid":"P5","summonerId":5,"gameName":"队5","tagLine":"T5","profileIconId":11}
                ]}}"#,
            ),
        ]);
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.teams[0].player_count, 5, "第 5 人应从花名册补回");
        assert_eq!(st.teams[1].player_count, 0, "盲选敌方不得泄露");
    }

    #[tokio::test]
    async fn champ_select_short_session_backfill() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""ChampSelect""#),
            (
                PATH_CHAMP_SELECT_SESSION,
                r#"{"myTeam":[
                    {"puuid":"PSELF","summonerId":1,"championId":103},
                    {"puuid":"P2","summonerId":2,"championId":1},
                    {"puuid":"P3","summonerId":3,"championId":2},
                    {"puuid":"P4","summonerId":4,"championId":3}
                ],"theirTeam":[]}"#,
            ),
            (
                PATH_GAMEFLOW_SESSION,
                r#"{"queueId":420,"gameData":{"teamOne":[
                    {"puuid":"PSELF","summonerId":1,"gameName":"我","tagLine":"CN1"},
                    {"puuid":"P2","summonerId":2,"gameName":"队2","tagLine":"T2"},
                    {"puuid":"P3","summonerId":3,"gameName":"队3","tagLine":"T3"},
                    {"puuid":"P4","summonerId":4,"gameName":"队4","tagLine":"T4"},
                    {"puuid":"P5","summonerId":5,"gameName":"队5","tagLine":"T5","profileIconId":11}
                ]}}"#,
            ),
        ]);
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.teams[0].player_count, 5);
        assert_eq!(st.teams[0].slots[4].game_name, "队5");
    }

    #[tokio::test]
    async fn live_in_progress() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""InProgress""#),
            (PATH_GAMEFLOW_SESSION, r#"{"queueId": 420}"#),
            (
                PATH_GD_CHAMPION_SUMMARY,
                r#"[{"id":1,"alias":"Annie","name":"黑暗之女"},{"id":22,"alias":"Ashe","name":"寒冰射手"}]"#,
            ),
            (
                "/lol-summoner/v1/summoners/by-puuid/PA",
                r#"{"puuid":"PA","gameName":"我","tagLine":"CN1","profileIconId":7,"summonerId":"1001"}"#,
            ),
        ]);
        let live = FakeLive {
            active: "我#CN1".into(),
            players: vec![
                Player {
                    summoner_name: "我#CN1".into(),
                    puuid: "PA".into(),
                    champion_name: "Annie".into(),
                    team: TeamId::Blue,
                    ..Default::default()
                },
                Player {
                    riot_id_game_name: "队友".into(),
                    riot_id_tag_line: "CN2".into(),
                    puuid: "PB".into(),
                    champion_name: "Ashe".into(),
                    team: TeamId::Blue,
                    ..Default::default()
                },
                Player {
                    riot_id_game_name: "敌人".into(),
                    riot_id_tag_line: "CN9".into(),
                    puuid: "PC".into(),
                    champion_name: "Annie".into(),
                    team: TeamId::Red,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut hist = FakeHist::default();
        hist.summoners.insert(
            "队友#CN2".into(),
            SummonerResult {
                puuid: "PB".into(),
                game_name: "队友".into(),
                tag_line: "CN2".into(),
                profile_icon_id: 8,
                summoner_id: "1002".into(),
                ..Default::default()
            },
        );
        hist.summoners.insert(
            "敌人#CN9".into(),
            SummonerResult {
                puuid: "PC".into(),
                game_name: "敌人".into(),
                tag_line: "CN9".into(),
                profile_icon_id: 9,
                summoner_id: "1003".into(),
                ..Default::default()
            },
        );
        let svc = new_test(http, hist, live);
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.teams[0].phase_label, "游戏中");
        assert_eq!(st.teams[0].player_count, 2);
        assert_eq!(st.teams[1].player_count, 1);
        assert!(st.teams[0].slots[0].is_self);
        assert_eq!(st.teams[0].slots[0].champion_id, 1);
        assert_eq!(st.teams[1].slots[0].champion_id, 1);
        assert_eq!(st.teams[0].slots[1].profile_icon_id, 8);
        assert_eq!(st.teams[0].slots[1].summoner_id, "1002");
    }

    #[tokio::test]
    async fn live_unavailable_empty() {
        let http = routes_from(&[(PATH_GAMEFLOW_PHASE, r#""InProgress""#)]);
        let live = FakeLive {
            err: Some("connection refused".into()),
            ..Default::default()
        };
        let svc = new_test(http, FakeHist::default(), live);
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.teams[0].player_count, 0);
        assert_eq!(st.teams[1].player_count, 0);
    }

    #[tokio::test]
    async fn none_empty_view() {
        let http = FakeHttp::new(connected_self());
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.phase, "None");
        assert_eq!(st.teams[0].phase_label, "大厅中");
        assert_eq!(st.teams[0].player_count, 0);
    }

    #[tokio::test]
    async fn offline_errors() {
        let http = FakeHttp::new(ConnStatus {
            state: State::Disconnected,
            ..Default::default()
        });
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let err = svc.get_gameflow_state(vec![]).await.unwrap_err();
        assert!(err.contains("未连接"), "err={err}");
    }

    #[tokio::test]
    async fn build_slots_enrich_and_hidden() {
        let http = routes_from(&[]);
        let mut hist = FakeHist::default();
        hist.ranked.insert(
            "1001".into(),
            RankedInfo {
                summoner_id: "1001".into(),
                puuid: "PA".into(),
                solo: "黄金 IV 45".into(),
                flex: "未定级".into(),
            },
        );
        hist.matches.insert(
            "PA".into(),
            vec![
                mk_sum(2400, "海斗", true, 5, 1, 5),
                mk_sum(2400, "海斗", false, 1, 4, 2),
            ],
        );
        hist.match_err.insert("PB".into());
        let svc = new_test(http, hist, FakeLive::default());
        let refs = vec![
            PlayerRef {
                puuid: "PA".into(),
                summoner_id: "1001".into(),
                game_name: "正常".into(),
                ..Default::default()
            },
            PlayerRef {
                puuid: "PB".into(),
                summoner_id: "1002".into(),
                game_name: "隐藏".into(),
                ..Default::default()
            },
        ];
        let slots = svc.build_slots(&refs, &[]).await;
        let a = &slots[0];
        assert_eq!(a.solo, "黄金 IV 45");
        assert_eq!(a.flex, "");
        assert_eq!(a.win_rate, 50.0);
        assert_eq!(a.win_rate_sample, 2);
        assert_eq!(a.avg_kda, 5.38);
        assert_eq!(a.rating, 7.9);
        let b = &slots[1];
        assert!(b.hidden_career);
        assert!(b.recent.is_empty());
        assert_eq!(slots.len(), 5);
        assert!(!slots[4].filled);
    }

    #[tokio::test]
    async fn champ_select_session_names() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""ChampSelect""#),
            (
                PATH_CHAMP_SELECT_SESSION,
                r#"{
                    "myTeam": [
                        {"puuid":"PSELF","summonerId":1,"championId":103,"gameName":"我","tagLine":"CN1","profileIconId":7},
                        {"puuid":"P2","summonerId":2,"championId":0,"gameName":"队友","tagLine":"CN2"}
                    ],
                    "theirTeam": [{"puuid":"","summonerId":0,"championId":0}]
                }"#,
            ),
        ]);
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        let a = &st.teams[0].slots[0];
        assert_eq!(a.game_name, "我");
        assert_eq!(a.profile_icon_id, 7);
        assert!(a.is_self);
        let b = &st.teams[0].slots[1];
        assert_eq!(b.game_name, "队友");
        assert_eq!(b.tag_line, "CN2");
    }

    #[tokio::test]
    async fn champ_select_roster_enrich() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""ChampSelect""#),
            (
                PATH_CHAMP_SELECT_SESSION,
                r#"{"myTeam":[{"puuid":"P2","summonerId":2,"championId":0}],"theirTeam":[]}"#,
            ),
            (
                PATH_GAMEFLOW_SESSION,
                r#"{"gameData":{"teamOne":[
                    {"puuid":"PSELF","summonerId":1,"gameName":"我","tagLine":"CN1","profileIconId":7},
                    {"puuid":"P2","summonerId":2,"gameName":"队友","tagLine":"T2","profileIconId":33}]}}"#,
            ),
        ]);
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        let b = &st.teams[0].slots[0];
        assert_eq!(b.game_name, "队友");
        assert_eq!(b.profile_icon_id, 33);
    }

    #[tokio::test]
    async fn champ_select_summoner_id_fallback() {
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""ChampSelect""#),
            (
                PATH_CHAMP_SELECT_SESSION,
                r#"{"myTeam":[{"puuid":"PX","summonerId":777,"championId":0}],"theirTeam":[]}"#,
            ),
            (
                "/lol-summoner/v1/summoners/777",
                r#"{"puuid":"PX","displayName":"补位#T7","profileIconId":66,"summonerId":777}"#,
            ),
        ]);
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        let a = &st.teams[0].slots[0];
        assert_eq!(a.game_name, "补位");
        assert_eq!(a.tag_line, "T7");
        assert_eq!(a.profile_icon_id, 66);
        assert_eq!(a.summoner_id, "777");
    }

    #[tokio::test]
    async fn in_game_roster_dual_teams() {
        let roster = r#"{"gameData":{"queueId":420,
            "teamOne":[
                {"puuid":"PSELF","summonerId":1001,"gameName":"我","tagLine":"CN1","profileIconId":7,"championId":1},
                {"puuid":"P2","summonerId":1002,"gameName":"队2","tagLine":"T2","profileIconId":8},
                {"puuid":"P3","summonerId":1003,"gameName":"队3","tagLine":"T3"},
                {"puuid":"P4","summonerId":1004,"gameName":"队4","tagLine":"T4"},
                {"puuid":"P5","summonerId":1005,"gameName":"队5","tagLine":"T5"}],
            "teamTwo":[
                {"puuid":"E1","summonerId":2001,"gameName":"敌1","tagLine":"E1"},
                {"puuid":"E2","summonerId":2002,"gameName":"敌2","tagLine":"E2"},
                {"puuid":"E3","summonerId":2003,"gameName":"敌3","tagLine":"E3"},
                {"puuid":"E4","summonerId":2004,"gameName":"敌4","tagLine":"E4"},
                {"puuid":"E5","summonerId":2005,"gameName":"敌5","tagLine":"E5"}]}}"#;
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""GameStart""#),
            (PATH_GAMEFLOW_SESSION, roster),
        ]);
        let live = FakeLive {
            err: Some("live down".into()),
            ..Default::default()
        };
        let svc = new_test(http, FakeHist::default(), live);
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.phase, "GameStart");
        assert_eq!(st.teams[0].phase_label, "游戏启动");
        assert_eq!(st.teams[0].player_count, 5);
        assert_eq!(st.teams[1].player_count, 5);
        assert_eq!(st.queue_label, queue_info_for(420).name);
        let a = &st.teams[0].slots[0];
        assert!(a.is_self);
        assert_eq!(a.game_name, "我");
        assert_eq!(a.profile_icon_id, 7);
        assert_eq!(a.champion_id, 1);
        let e = &st.teams[1].slots[0];
        assert_eq!(e.game_name, "敌1");
        assert_eq!(e.summoner_id, "2001");
    }

    #[tokio::test]
    async fn roster_self_in_team_two_flips() {
        let http = routes_from(&[(
            PATH_GAMEFLOW_SESSION,
            r#"{"gameData":{
                "teamOne":[{"puuid":"A1","summonerId":1,"gameName":"甲","tagLine":"T1"}],
                "teamTwo":[{"puuid":"PSELF","summonerId":2,"gameName":"我","tagLine":"CN1"}]}}"#,
        )]);
        let sess = {
            let (s, b) = http.get(PATH_GAMEFLOW_SESSION).await.unwrap();
            assert_eq!(s, 200);
            serde_json::from_slice::<GameflowSession>(&b).unwrap()
        };
        let (ally, enemy) = sess.roster(&connected_self());
        assert_eq!(ally.len(), 1);
        assert_eq!(enemy.len(), 1);
        assert_eq!(ally[0].puuid, "PSELF");
        assert_eq!(enemy[0].puuid, "A1");
    }

    #[tokio::test]
    async fn live_team_unusable_half_split() {
        let mut players = Vec::new();
        for i in 0..10 {
            players.push(Player {
                riot_id_game_name: format!("P{i}"),
                riot_id_tag_line: "T".into(),
                puuid: format!("Q{i}"),
                ..Default::default()
            });
        }
        players[0] = Player {
            riot_id_game_name: "我".into(),
            riot_id_tag_line: "CN1".into(),
            puuid: "Q0".into(),
            ..Default::default()
        };
        let http = routes_from(&[(PATH_GAMEFLOW_PHASE, r#""InProgress""#)]);
        let live = FakeLive {
            active: "我#CN1".into(),
            players,
            ..Default::default()
        };
        let svc = new_test(http, FakeHist::default(), live);
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.teams[0].player_count, 5);
        assert_eq!(st.teams[1].player_count, 5);
        assert!(st.teams[0].slots[0].is_self);
    }

    #[tokio::test]
    async fn in_game_roster_hole_fill() {
        let missing = "84be6377-c593-537f-9b0c-709a1febab04";
        let sess = format!(
            r#"{{"gameData":{{
                "queue":{{"id":2400,"name":"海克斯大乱斗 ","numPlayersPerTeam":5}},
                "teamOne":[
                    {{"puuid":"PSELF","summonerId":17993441068,"summonerName":"","profileIconId":7063,"championId":157}},
                    {{"puuid":"A2","summonerId":17775323153,"summonerName":"","profileIconId":6589,"championId":11}},
                    {{"puuid":"A3","summonerId":17059462600,"summonerName":"","profileIconId":3543,"championId":99}},
                    {{"puuid":"A4","summonerId":17506727465,"summonerName":"","profileIconId":6841,"championId":111}},
                    {{"puuid":"A5","summonerId":16262047083,"summonerName":"","profileIconId":745,"championId":777}}],
                "teamTwo":[
                    {{"puuid":"E1","summonerId":17004661252,"summonerName":"","profileIconId":4745,"championId":154}},
                    {{"puuid":"E2","summonerId":4102901763434272,"summonerName":"","profileIconId":3542,"championId":45}},
                    {{"puuid":"E3","summonerId":16067475272,"summonerName":"","profileIconId":3796,"championId":876}},
                    {{"puuid":"E4","summonerId":18101421026,"summonerName":"","profileIconId":4568,"championId":203}}],
                "playerChampionSelections":[
                    {{"puuid":"A4","championId":111}},{{"puuid":"A2","championId":11}},{{"puuid":"A5","championId":777}},
                    {{"puuid":"PSELF","championId":157}},{{"puuid":"A3","championId":99}},{{"puuid":"E3","championId":876}},
                    {{"puuid":"{missing}","championId":112}},{{"puuid":"E1","championId":154}},
{{"puuid":"E2","championId":45}},{{"puuid":"E4","championId":203}}]}}}}"#
        );
        let by_puuid = format!("/lol-summoner/v1/summoners/by-puuid/{missing}");
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""InProgress""#),
            (PATH_GAMEFLOW_SESSION, &sess),
            (
                by_puuid.as_str(),
                r#"{"puuid":"84be6377-c593-537f-9b0c-709a1febab04","gameName":"漏网者","tagLine":"CN5","profileIconId":999,"summonerId":"5555"}"#,
            ),
        ]);
        let live = FakeLive {
            err: Some("live down".into()),
            ..Default::default()
        };
        let svc = new_test(http, FakeHist::default(), live);
        let st = svc.get_gameflow_state(vec![]).await.unwrap();
        assert_eq!(st.teams[0].player_count, 5);
        assert_eq!(st.teams[1].player_count, 5, "teamTwo 漏第 5 人未补回");
        assert_eq!(st.queue_label, "海克斯大乱斗");
        let a = &st.teams[0].slots[0];
        assert!(a.is_self);
        assert_eq!(a.champion_id, 157);
        assert_eq!(a.profile_icon_id, 7063);
        assert_eq!(a.summoner_id, "17993441068");
        let e = &st.teams[1].slots[4];
        assert!(e.filled);
        assert_eq!(e.game_name, "漏网者");
        assert_eq!(e.champion_id, 112);
        assert_eq!(e.profile_icon_id, 999);
        assert_eq!(e.summoner_id, "5555");
        assert_eq!(st.teams[1].slots[1].summoner_id, "4102901763434272");
    }

    async fn parse_sess(raw: &str) -> GameflowSession {
        let http = routes_from(&[(PATH_GAMEFLOW_SESSION, raw)]);
        let (s, b) = http.get(PATH_GAMEFLOW_SESSION).await.unwrap();
        assert_eq!(s, 200);
        serde_json::from_slice(&b).unwrap()
    }

    #[tokio::test]
    async fn queue_name_variants() {
        let s = parse_sess(r#"{"gameData":{"queue":{"id":2400,"name":"海克斯大乱斗 "}}}"#).await;
        assert_eq!(s.queue_name(), "海克斯大乱斗");
        let s = parse_sess(r#"{"gameData":{"queue":{"id":86783593,"name":"末日人机 "}}}"#).await;
        assert_eq!(s.queue_name(), "末日人机");
        let s = parse_sess(r#"{"gameData":{"queueId":440}}"#).await;
        assert_eq!(s.queue_name(), queue_info_for(440).name);
        let s = parse_sess(r#"{"queue":{"id":420}}"#).await;
        assert_eq!(s.queue_name(), queue_info_for(420).name);
        let s = parse_sess(r#"{"queueId":450}"#).await;
        assert_eq!(s.queue_name(), queue_info_for(450).name);
        let s = parse_sess("{}").await;
        assert_eq!(s.queue_name(), "");
    }

    #[tokio::test]
    async fn career_filter_20_and_follow_queue() {
        let mut hist = FakeHist::default();
        let mut pages = HashMap::new();
        pages.insert("PA".into(), mixed_pages());
        pages.insert("E1".into(), vec![vec![mk_sum(420, "单双", true, 3, 3, 3)]]);
        hist.pages = Some(pages);

        let sess = r#"{"gameData":{"queue":{"id":2400,"name":"海克斯大乱斗 ","numPlayersPerTeam":5},
            "teamOne":[{"puuid":"PA","summonerId":1001,"profileIconId":7,"championId":1}],
            "teamTwo":[{"puuid":"E1","summonerId":2001,"profileIconId":8,"championId":2}]}}"#;
        let http = routes_from(&[
            (PATH_GAMEFLOW_PHASE, r#""InProgress""#),
            (PATH_GAMEFLOW_SESSION, sess),
        ]);
        let live = FakeLive {
            err: Some("live down".into()),
            ..Default::default()
        };
        let svc = new_test(http, hist, live);
        let st = svc.get_gameflow_state(vec![-1]).await.unwrap();
        assert_eq!(st.queue_id, 2400);
        let a = &st.teams[0].slots[0];
        assert_eq!(a.recent.len(), 20);
        for r in &a.recent {
            assert_eq!(r.queue_short, "海斗");
        }
        let e = &st.teams[1].slots[0];
        assert!(e.recent.is_empty());
        assert!(!e.hidden_career);
    }

    #[tokio::test]
    async fn career_filter_empty_allows_mixed() {
        let mut hist = FakeHist::default();
        let mut pages = HashMap::new();
        pages.insert("PA".into(), mixed_pages());
        hist.pages = Some(pages);
        let svc = new_test(routes_from(&[]), hist, FakeLive::default());
        let refs = vec![PlayerRef {
            puuid: "PA".into(),
            summoner_id: "1001".into(),
            game_name: "甲".into(),
            profile_icon_id: 1,
            ..Default::default()
        }];
        let slots = svc.build_slots(&refs, &[]).await;
        let b = &slots[0];
        assert_eq!(b.recent.len(), 20);
        assert_eq!(b.win_rate_sample, 20);
        assert_eq!(b.recent[0].queue_short, "海斗");
        assert_eq!(b.recent[14].queue_short, "单双");
        assert_eq!(b.recent[15].queue_short, "海斗");
    }

    #[tokio::test]
    async fn career_limit_hot_reload() {
        let svc = new_test(routes_from(&[]), FakeHist::default(), FakeLive::default());
        svc.set_career_limit(10);
        assert_eq!(svc.current_career_limit(), 10);
        svc.set_career_limit(3);
        assert_eq!(svc.current_career_limit(), 10);

        let mut hist = FakeHist::default();
        let sums: Vec<MatchSummary> = (0..30)
            .map(|i| MatchSummary {
                game_id: i,
                queue_id: 420,
                queue_short: "排位".into(),
                win: true,
                champion_id: 1,
                ..Default::default()
            })
            .collect();
        hist.matches.insert("P1".into(), sums);
        let svc = new_test(routes_from(&[]), hist, FakeLive::default());
        svc.set_career_limit(10);
        let c = svc.load_career("P1", &[]).await;
        assert_eq!(c.recent.len(), 10);
        svc.set_career_limit(30);
        let c = svc.load_career("P1", &[]).await;
        assert_eq!(c.recent.len(), 30);
    }

    #[tokio::test]
    async fn identity_fill_cache_and_concurrency_guard() {
        let hits = Arc::new(AtomicU32::new(0));
        let http = FakeHttp::new(connected_self()).with_handler({
            let hits = hits.clone();
            move |path: &str| {
                if path.starts_with("/lol-summoner/v1/summoners/by-puuid/P2") {
                    hits.fetch_add(1, Ordering::SeqCst);
                    Ok((
                        200,
                        r#"{"puuid":"P2","gameName":"队友","tagLine":"T2","profileIconId":33,"summonerId":"2"}"#
                            .as_bytes()
                            .to_vec(),
                    ))
                } else {
                    Ok((404, vec![]))
                }
            }
        });
        let svc = new_test(http, FakeHist::default(), FakeLive::default());
        let mut r = PlayerRef {
            puuid: "P2".into(),
            ..Default::default()
        };
        svc.fill_identity(&mut r).await;
        assert_eq!(r.game_name, "队友");
        assert_eq!(r.profile_icon_id, 33);
        assert_eq!(r.summoner_id, "2");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
