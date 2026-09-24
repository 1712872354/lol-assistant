//! 对局信息页数据聚合（对齐 Go service/gameinfo）。
//! 按 gameflow 阶段选源，best-effort 补段位/近况；依赖经 trait 注入。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::future::join_all;

use crate::lcu::endpoints::{
    PATH_CHAMP_SELECT_SESSION, PATH_GAMEFLOW_PHASE, PATH_GAMEFLOW_SESSION,
    PATH_GD_CHAMPION_SUMMARY, PATH_LOBBY, PATH_SUMMONER_BY_ID, PATH_SUMMONER_BY_PUUID,
};
use crate::lcu::ConnStatus;
use crate::liveclient::{Player, TeamId};
use crate::parser::queue_info_for;
#[cfg(test)]
use crate::parser::MatchSummary;
use crate::service::history::{HistoryService, MatchPage, RankedInfo, SummonerResult};
use crate::service::http::{path_escape, BoxFut, LcuHttp};

const CAREER_TTL: Duration = Duration::from_secs(90);
const CAREER_SCAN_PAGES: i32 = 4;
const CAREER_CACHE_MAX: usize = 512;
const DEFAULT_CAREER_LIMIT: i32 = 20;
const CHAMP_INDEX_TTL: Duration = Duration::from_secs(10 * 60);

mod cache;
mod model;
mod protocol;
mod util;

#[cfg(test)]
mod tests;

use crate::service::key_gate::KeyGate;
use crate::util::{opt_id, opt_name};
use cache::{CareerState, ChampIndexState};
pub use model::{PlayerSlot, RecentMatch, TeamView, ViewState};
use protocol::*;
use util::*;

/* ── 依赖面 ── */

pub trait HistApi: Send + Sync {
    fn get_players_ranked<'a>(
        &'a self,
        ids: Vec<String>,
    ) -> BoxFut<'a, Result<Vec<RankedInfo>, crate::error::AppError>>;
    fn get_matches<'a>(
        &'a self,
        puuid: String,
        page: i32,
    ) -> BoxFut<'a, Result<MatchPage, crate::error::AppError>>;
    fn search_summoner<'a>(
        &'a self,
        name: String,
    ) -> BoxFut<'a, Result<SummonerResult, crate::error::AppError>>;
}

impl HistApi for HistoryService {
    fn get_players_ranked<'a>(
        &'a self,
        ids: Vec<String>,
    ) -> BoxFut<'a, Result<Vec<RankedInfo>, crate::error::AppError>> {
        Box::pin(async move { HistoryService::get_players_ranked(self, &ids).await })
    }

    fn get_matches<'a>(
        &'a self,
        puuid: String,
        page: i32,
    ) -> BoxFut<'a, Result<MatchPage, crate::error::AppError>> {
        Box::pin(async move { HistoryService::get_matches(self, &puuid, page).await })
    }

    fn search_summoner<'a>(
        &'a self,
        name: String,
    ) -> BoxFut<'a, Result<SummonerResult, crate::error::AppError>> {
        Box::pin(async move { HistoryService::search_summoner(self, &name).await })
    }
}

pub trait LiveApi: Send + Sync {
    fn player_list<'a>(&'a self) -> BoxFut<'a, Result<Vec<Player>, crate::error::AppError>>;
    fn active_player_name<'a>(&'a self) -> BoxFut<'a, Result<String, crate::error::AppError>>;
}

impl LiveApi for crate::liveclient::Client {
    fn player_list<'a>(&'a self) -> BoxFut<'a, Result<Vec<Player>, crate::error::AppError>> {
        Box::pin(async move { crate::liveclient::Client::player_list(self).await })
    }

    fn active_player_name<'a>(&'a self) -> BoxFut<'a, Result<String, crate::error::AppError>> {
        Box::pin(async move { crate::liveclient::Client::active_player_name(self).await })
    }
}

/* ── 服务 ── */

pub struct GameinfoService {
    http: Arc<dyn LcuHttp>,
    hist: Arc<dyn HistApi>,
    live: Arc<dyn LiveApi>,
    champ: Mutex<ChampIndexState>,
    career: Mutex<CareerState>,
    /// 同 key 并发合并（近况 / 英雄索引）
    inflight: KeyGate,
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
            inflight: KeyGate::new(),
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

    pub async fn get_gameflow_state(
        &self,
        queue_filter: Option<Vec<i32>>,
    ) -> Result<ViewState, crate::error::AppError> {
        let st = self.http.status().await;
        if st.state != crate::lcu::State::Connected {
            return Err(crate::error::AppError::NotConnected);
        }

        let phase = self.fetch_phase().await;
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

        let filter = resolve_queue_filter(queue_filter.as_deref(), queue_id);
        let ally_slots = self.build_slots(&ally, &filter).await;
        let enemy_slots = self.build_slots(&enemy, &filter).await;

        Ok(ViewState {
            phase,
            queue_label,
            queue_id,
            teams: vec![
                sum_team("ally", "我方", "蓝方·房间", "我方", ally_slots),
                sum_team("enemy", "敌方", "红方", "敌方", enemy_slots),
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
                puuid: opt_id(m.puuid.clone()),
                summoner_id: opt_id(m.summoner_id.clone()),
                game_name: opt_name(m.game_name.clone()),
                tag_line: opt_name(m.tag_line.clone()),
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
                let pu = opt_id(p.puuid.clone());
                let sid = opt_id(p.summoner_id.clone());
                if pu.is_none() && sid.is_none() {
                    continue;
                }
                out.push(PlayerRef {
                    puuid: pu,
                    summoner_id: sid,
                    game_name: opt_name(p.game_name.clone()),
                    tag_line: opt_name(p.tag_line.clone()),
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
                puuid: opt_id(p.puuid.clone()),
                game_name: opt_name(g.clone()),
                tag_line: opt_name(t.clone()),
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
                r.puuid.is_none()
                    || r.game_name.is_none()
                    || r.profile_icon_id == 0
                    || r.summoner_id.is_none()
            })
            .map(|(i, _)| i)
            .collect();
        let sem = Arc::new(tokio::sync::Semaphore::new(4));
        let filled = join_all(incomplete.into_iter().map(|i| {
            let mut r = refs[i].clone();
            let sem = sem.clone();
            async move {
                let _p = sem.acquire().await.expect("semaphore");
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
            if let Some(sid) = &r.summoner_id {
                ids.push(sid.clone());
            }
            if let Some(pu) = &r.puuid {
                ids.push(pu.clone());
            }
        }
        let mut rank_map: HashMap<String, RankedInfo> = HashMap::new();
        if !ids.is_empty() {
            if let Ok(rows) = self.hist.get_players_ranked(ids).await {
                for r in rows {
                    if !r.query_id.is_empty() {
                        rank_map.insert(r.query_id.clone(), r.clone());
                    }
                    if !r.puuid.is_empty() {
                        rank_map.insert(r.puuid.clone(), r);
                    }
                }
            }
        }

        // ③ 近况并发
        let filter_vec = filter.to_vec();
        let sem = Arc::new(tokio::sync::Semaphore::new(4));
        let careers = join_all(refs.iter().map(|r| {
            let filter = filter_vec.clone();
            let sem = sem.clone();
            async move {
                let _p = sem.acquire().await.expect("semaphore");
                let Some(puuid) = &r.puuid else {
                    return Career {
                        recent: Vec::new(),
                        hidden: true,
                    };
                };
                self.fetch_career(puuid, &filter).await
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
                profile_icon_id: (r.profile_icon_id > 0).then_some(r.profile_icon_id),
                champion_id: (r.champion_id > 0).then_some(r.champion_id),
                hidden_career: c.hidden,
                recent: c.recent.clone(),
                ..Default::default()
            };
            let rk = r
                .summoner_id
                .as_deref()
                .and_then(|sid| rank_map.get(sid))
                .or_else(|| r.puuid.as_deref().and_then(|pu| rank_map.get(pu)));
            if let Some(rk) = rk {
                let (solo, flex) = pick_rank(rk);
                slot.solo = opt_name(solo);
                slot.flex = opt_name(flex);
            }
            let (wr, sample, avg, rating) = career_stats(&c.recent);
            slot.win_rate = (wr > 0.0).then_some(wr);
            slot.win_rate_sample = (sample > 0).then_some(sample);
            slot.avg_kda = (avg > 0.0).then_some(avg);
            slot.player_score = Some(rating);
            slots.push(slot);
        }
        while slots.len() < 5 {
            slots.push(PlayerSlot::default());
        }
        slots
    }

    async fn fill_identity(&self, r: &mut PlayerRef) {
        if let Some(pu) = r.puuid.clone() {
            let path = PATH_SUMMONER_BY_PUUID.replace("%s", &path_escape(&pu));
            if let Some(raw) = self.lookup_summoner(&path).await {
                raw.apply(r);
            }
        }
        if (r.game_name.is_none() || r.profile_icon_id == 0) && r.summoner_id.is_some() {
            let sid = r.summoner_id.clone().unwrap_or_default();
            let path = PATH_SUMMONER_BY_ID.replace("%s", &path_escape(&sid));
            if let Some(raw) = self.lookup_summoner(&path).await {
                raw.apply(r);
            }
        }
        if (r.puuid.is_none() || r.profile_icon_id == 0) && r.game_name.is_some() {
            let mut name = r.game_name.clone().unwrap_or_default();
            if let Some(t) = &r.tag_line {
                name.push('#');
                name.push_str(t);
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
        let key = format!(
            "{puuid}|{}",
            filter
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        if let Some(c) = self.cached_career(&key) {
            return c;
        }
        // single-flight：同 key 并发只回源一次
        let gate = self.inflight.key(&key);
        let _hold = gate.lock().await;
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
                            queue_name: Some(m.queue_name.clone()).filter(|s| !s.is_empty()),
                            time_short: m.short_time.clone(),
                            game_creation: (m.game_creation > 0).then_some(m.game_creation),
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
        let gate = self.inflight.key("champ_index");
        let _hold = gate.lock().await;
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
