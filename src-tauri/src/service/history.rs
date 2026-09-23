//! 历史战绩服务（对齐 Go service/history）。
//! SGP 优先、LCU 兜底；资源代理 base64 + 内存缓存。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::lcu::endpoints::{
    PATH_ENTITLEMENTS_TOKEN, PATH_GD_AUGMENTS, PATH_GD_CHAMPION_ICON, PATH_GD_ITEMS,
    PATH_GD_ITEM_ICON, PATH_GD_PERKS, PATH_GD_PROFILE_ICON, PATH_GD_SPELLS,
    PATH_LEAGUE_SESSION_TOKEN, PATH_MATCH_GAME_DETAIL, PATH_MATCH_HISTORY,
    PATH_RANKED_STATS_BY_SUMMONER, PATH_SUMMONERS_BY_NAME, PATH_SUMMONER_BY_ID,
    PATH_SUMMONER_BY_PUUID,
};
#[cfg(test)]
use crate::lcu::ConnStatus;
use crate::lcu::State;
use crate::parser::{
    parse_match_detail, parse_match_summaries, parse_sgp_summaries, tier_cn, MatchDetail,
    MatchSummary,
};
use crate::service::http::{path_escape, query_escape, BoxFut, LcuHttp};

pub const ERR_NOT_CONNECTED: &str = "LCU 未连接，请先启动英雄联盟客户端并登录";

pub const ASSET_CHAMPION: &str = "champion";
pub const ASSET_PROFILE: &str = "profile";
pub const ASSET_ITEM: &str = "item";
pub const ASSET_SPELL: &str = "spell";
pub const ASSET_PERK: &str = "perk";
pub const ASSET_AUGMENT: &str = "augment";

const UNRANKED: &str = "未定级";
const SGP_TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
const SGP_TOKEN_ERR_TTL: Duration = Duration::from_secs(15);
const INDEX_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_BYTE_ENTRIES: usize = 1024;

/* ── 输出视图模型 ─────────────────────────────────────────── */

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummonerResult {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
    pub display_name: String,
    pub profile_icon_id: i32,
    pub summoner_level: i64,
    pub summoner_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchPage {
    pub puuid: String,
    pub page: i32,
    pub page_size: i32,
    pub game_count: i32,
    pub total_pages: i32,
    pub has_more: bool,
    pub summaries: Vec<MatchSummary>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RankedInfo {
    pub summoner_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub puuid: String,
    pub solo: String,
    pub flex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetResult {
    pub kind: String,
    pub id: i32,
    pub mime: String,
    pub data: String,
}

/* ── SGP 拉取注入 ─────────────────────────────────────────── */

pub type SgpRankedFetch = Arc<
    dyn Fn(String, String, String) -> BoxFut<'static, Result<crate::sgp::RankedStats, String>>
        + Send
        + Sync,
>;
pub type SgpMatchFetch = Arc<
    dyn Fn(String, String, String, i32, i32) -> BoxFut<'static, Result<Vec<u8>, String>>
        + Send
        + Sync,
>;

fn default_sgp_ranked() -> SgpRankedFetch {
    Arc::new(|host, puuid, token| {
        Box::pin(async move { crate::sgp::fetch_ranked_stats(&host, &puuid, &token).await })
    })
}

fn default_sgp_match() -> SgpMatchFetch {
    Arc::new(|host, puuid, token, start, count| {
        Box::pin(async move {
            crate::sgp::fetch_match_history(&host, &puuid, &token, start, count).await
        })
    })
}

#[derive(Clone, Default)]
struct SgpTokens {
    session: String,
    entitlements: String,
}

impl SgpTokens {
    fn ranked(&self) -> Vec<String> {
        non_empty(&self.session, &self.entitlements)
    }
    fn matched(&self) -> Vec<String> {
        non_empty(&self.entitlements, &self.session)
    }
}

fn non_empty(a: &str, b: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !a.is_empty() {
        out.push(a.to_string());
    }
    if !b.is_empty() && b != a {
        out.push(b.to_string());
    }
    out
}

struct TokenCache {
    toks: SgpTokens,
    at: Option<Instant>,
    ttl: Duration,
}

impl TokenCache {
    fn new() -> Self {
        Self {
            toks: SgpTokens::default(),
            at: None,
            ttl: Duration::ZERO,
        }
    }

    fn valid(&self) -> bool {
        self.ttl > Duration::ZERO && self.at.map(|at| at.elapsed() < self.ttl).unwrap_or(false)
    }
}

/* ── 资源缓存 ─────────────────────────────────────────────── */

struct AssetCacheInner {
    bytes: HashMap<String, (String, String)>,
    indexes: HashMap<String, (Instant, HashMap<i32, String>)>,
}

#[derive(Clone)]
struct AssetCache {
    inner: Arc<Mutex<AssetCacheInner>>,
}

impl AssetCache {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AssetCacheInner {
                bytes: HashMap::new(),
                indexes: HashMap::new(),
            })),
        }
    }

    fn get_bytes(&self, key: &str) -> Option<(String, String)> {
        self.inner.lock().unwrap().bytes.get(key).cloned()
    }

    fn put_bytes(&self, key: &str, mime: String, data: String) {
        let mut g = self.inner.lock().unwrap();
        if g.bytes.len() >= MAX_BYTE_ENTRIES {
            g.bytes.clear();
        }
        g.bytes.insert(key.to_string(), (mime, data));
    }

    fn get_index(&self, path: &str) -> Option<HashMap<i32, String>> {
        let g = self.inner.lock().unwrap();
        g.indexes
            .get(path)
            .filter(|(at, _)| at.elapsed() < INDEX_TTL)
            .map(|(_, m)| m.clone())
    }

    fn put_index(&self, path: &str, paths: HashMap<i32, String>) {
        self.inner
            .lock()
            .unwrap()
            .indexes
            .insert(path.to_string(), (Instant::now(), paths));
    }
}

/* ── 服务 ─────────────────────────────────────────────────── */

pub struct HistoryService {
    http: Arc<dyn LcuHttp>,
    page_size: Mutex<i32>,
    sgp_enabled: AtomicBool,
    assets: AssetCache,
    tok: Mutex<TokenCache>,
    sgp_ranked: SgpRankedFetch,
    sgp_match: SgpMatchFetch,
    #[cfg(test)]
    tok_err_ttl: Mutex<Duration>,
}

impl HistoryService {
    pub fn new(http: Arc<dyn LcuHttp>, page_size: i32) -> Self {
        let page_size = if (5..=50).contains(&page_size) {
            page_size
        } else {
            20
        };
        Self {
            http,
            page_size: Mutex::new(page_size),
            sgp_enabled: AtomicBool::new(true),
            assets: AssetCache::new(),
            tok: Mutex::new(TokenCache::new()),
            sgp_ranked: default_sgp_ranked(),
            sgp_match: default_sgp_match(),
            #[cfg(test)]
            tok_err_ttl: Mutex::new(SGP_TOKEN_ERR_TTL),
        }
    }

    pub fn set_page_size(&self, n: i32) {
        if !(5..=50).contains(&n) {
            return;
        }
        *self.page_size.lock().unwrap() = n;
    }

    pub fn current_page_size(&self) -> i32 {
        *self.page_size.lock().unwrap()
    }

    pub fn set_sgp_enabled(&self, on: bool) {
        self.sgp_enabled.store(on, Ordering::Relaxed);
    }

    pub fn sgp_enabled(&self) -> bool {
        self.sgp_enabled.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub fn set_sgp_ranked_fetch(&mut self, f: SgpRankedFetch) {
        self.sgp_ranked = f;
    }

    #[cfg(test)]
    pub fn set_sgp_match_fetch(&mut self, f: SgpMatchFetch) {
        self.sgp_match = f;
    }

    #[cfg(test)]
    pub fn set_tok_err_ttl(&self, d: Duration) {
        *self.tok_err_ttl.lock().unwrap() = d;
    }

    async fn ensure_connected(&self) -> Result<(), String> {
        let st = self.http.status().await;
        if st.state != State::Connected {
            return Err(ERR_NOT_CONNECTED.to_string());
        }
        Ok(())
    }

    /* ── 召唤师查询 ── */

    pub async fn search_summoner(&self, name: &str) -> Result<SummonerResult, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("请输入召唤师昵称或 Riot ID（昵称#TAG）".into());
        }
        self.ensure_connected().await?;
        let path = format!("{PATH_SUMMONERS_BY_NAME}?name={}", query_escape(name));
        let (status, body) = self.http.get(&path).await?;
        if status == 404 {
            return Err(format!("未找到召唤师「{name}」，请检查昵称#TAG"));
        }
        if !(200..300).contains(&status) {
            return Err(format!("查询召唤师失败: HTTP {status}"));
        }
        let raw: SummonerRaw =
            serde_json::from_slice(&body).map_err(|e| format!("解析召唤师数据失败: {e}"))?;
        if raw.puuid.is_empty() {
            return Err(format!("未找到召唤师「{name}」，请检查昵称#TAG"));
        }
        Ok(SummonerResult {
            display_name: display_name_of(&raw.game_name, &raw.tag_line, &raw.display_name),
            puuid: raw.puuid,
            game_name: raw.game_name,
            tag_line: raw.tag_line,
            profile_icon_id: raw.profile_icon_id,
            summoner_level: raw.summoner_level,
            summoner_id: raw.summoner_id,
        })
    }

    pub async fn get_self_summoner(&self) -> Result<SummonerResult, String> {
        let st = self.http.status().await;
        if st.state != State::Connected {
            return Err("客户端未登录，无法获取当前召唤师".into());
        }
        let puuid = st
            .puuid
            .clone()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| "客户端未登录，无法获取当前召唤师".to_string())?;
        let game_name = st.game_name.clone().unwrap_or_default();
        let tag_line = st.tag_line.clone().unwrap_or_default();
        let mut res = SummonerResult {
            puuid: puuid.clone(),
            display_name: display_name_of(&game_name, &tag_line, ""),
            profile_icon_id: st.profile_icon_id.unwrap_or(0) as i32,
            summoner_level: st.summoner_level.unwrap_or(0),
            game_name,
            tag_line,
            summoner_id: String::new(),
        };
        let path = PATH_SUMMONER_BY_PUUID.replace("%s", &path_escape(&puuid));
        if let Ok((status, body)) = self.http.get(&path).await {
            if (200..300).contains(&status) {
                if let Ok(raw) = serde_json::from_slice::<SummonerRaw>(&body) {
                    res.summoner_id = raw.summoner_id;
                    if raw.profile_icon_id > 0 {
                        res.profile_icon_id = raw.profile_icon_id;
                    }
                    if raw.summoner_level > 0 {
                        res.summoner_level = raw.summoner_level;
                    }
                }
            }
        }
        Ok(res)
    }

    /* ── 战绩列表 ── */

    pub async fn get_matches(&self, puuid: &str, page: i32) -> Result<MatchPage, String> {
        let puuid = puuid.trim();
        if puuid.is_empty() {
            return Err("缺少召唤师 puuid".into());
        }
        let page = page.clamp(0, 1_000_000);
        let page_size = self.current_page_size();
        let beg = page * page_size;
        let end = beg + page_size - 1;

        if let Some(mp) = self.get_matches_sgp(puuid, page, beg, page_size).await {
            return Ok(mp);
        }

        self.ensure_connected().await?;
        let path = format!(
            "{}?begIndex={beg}&endIndex={end}",
            PATH_MATCH_HISTORY.replace("%s", &path_escape(puuid))
        );
        let (status, body) = self.http.get(&path).await?;
        if status == 404 {
            return Err("未找到战绩数据（该账号近期无对局记录）".into());
        }
        if !(200..300).contains(&status) {
            return Err(format!(
                "获取战绩失败: HTTP {status}（国服查询他人战绩可能受限）"
            ));
        }

        let (summaries, game_count) = parse_match_summaries(&body, puuid)?;
        let total_pages = if game_count > 0 {
            (game_count + page_size - 1) / page_size
        } else {
            1
        };
        let mut has_more = (beg + summaries.len() as i32) < game_count;
        if game_count <= 0 {
            has_more = (summaries.len() as i32) >= page_size;
        }
        Ok(MatchPage {
            puuid: puuid.to_string(),
            page,
            page_size,
            game_count,
            total_pages,
            has_more,
            summaries,
        })
    }

    async fn get_matches_sgp(
        &self,
        puuid: &str,
        page: i32,
        beg: i32,
        page_size: i32,
    ) -> Option<MatchPage> {
        if !self.sgp_enabled() {
            return None;
        }
        self.ensure_connected().await.ok()?;
        let st = self.http.status().await;
        let platform = st.platform_id.clone().unwrap_or_default();
        if platform.is_empty() {
            return None;
        }
        let host = crate::sgp::host(&platform);
        if host.is_empty() {
            return None;
        }
        let toks = self.fetch_sgp_tokens().await;
        let toks = toks.matched();
        if toks.is_empty() {
            return None;
        }

        let mut body: Option<Vec<u8>> = None;
        for tok in toks {
            if let Ok(b) =
                (self.sgp_match)(host.clone(), puuid.to_string(), tok, beg, page_size).await
            {
                body = Some(b);
                break;
            }
        }
        let body = body?;
        let summaries = parse_sgp_summaries(&body, puuid).ok()?;
        if summaries.is_empty() && page == 0 {
            return None;
        }
        let has_more = (summaries.len() as i32) >= page_size;
        let mut total_pages = page + 1;
        if has_more {
            total_pages += 1;
        }
        Some(MatchPage {
            puuid: puuid.to_string(),
            page,
            page_size,
            game_count: beg + summaries.len() as i32,
            total_pages,
            has_more,
            summaries,
        })
    }

    /* ── 对局明细 ── */

    pub async fn get_match_detail(
        &self,
        game_id: i64,
        self_puuid: &str,
    ) -> Result<MatchDetail, String> {
        if game_id <= 0 {
            return Err("无效对局 ID".into());
        }
        self.ensure_connected().await?;
        let path = PATH_MATCH_GAME_DETAIL.replace("%d", &game_id.to_string());
        let (status, body) = self.http.get(&path).await?;
        if !(200..300).contains(&status) {
            return Err(format!("获取对局明细失败: HTTP {status}"));
        }
        parse_match_detail(&body, self_puuid.trim())
    }

    /* ── 段位 ── */

    pub async fn get_players_ranked(&self, ids: &[String]) -> Result<Vec<RankedInfo>, String> {
        self.ensure_connected().await?;
        let mut seen = std::collections::HashSet::new();
        let mut uniq = Vec::new();
        for id in ids {
            let id = id.trim();
            if id.is_empty() || !seen.insert(id.to_string()) {
                continue;
            }
            uniq.push(id.to_string());
            if uniq.len() >= 40 {
                break;
            }
        }
        if uniq.is_empty() {
            return Ok(Vec::new());
        }

        let mut buckets: HashMap<String, Vec<String>> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for id in uniq {
            let canonical = if is_puuid(&id) {
                id.clone()
            } else {
                self.resolve_puuid(&id).await.unwrap_or_else(|| id.clone())
            };
            if !buckets.contains_key(&canonical) {
                order.push(canonical.clone());
            }
            buckets.entry(canonical).or_default().push(id);
        }

        let sgp_on = self.sgp_enabled();
        let st = self.http.status().await;
        let platform = st.platform_id.clone().unwrap_or_default();
        let mut sgp_host = String::new();
        let mut toks = SgpTokens::default();
        if sgp_on && !platform.is_empty() {
            sgp_host = crate::sgp::host(&platform);
            if !sgp_host.is_empty() {
                toks = self.fetch_sgp_tokens().await;
                if toks.ranked().is_empty() {
                    sgp_host.clear();
                }
            }
        }

        let mut results: HashMap<String, (String, String, bool)> = HashMap::new();
        for canonical in &order {
            let mut solo = UNRANKED.to_string();
            let mut flex = UNRANKED.to_string();
            let mut filled = false;
            let mut ok = false;

            if !sgp_host.is_empty() && is_puuid(canonical) {
                for tok in toks.ranked() {
                    if let Ok(stats) =
                        (self.sgp_ranked)(sgp_host.clone(), canonical.clone(), tok).await
                    {
                        let (s, f) = sgp_ranked_display(&stats);
                        solo = s;
                        flex = f;
                        filled = true;
                        ok = true;
                        break;
                    }
                }
            }
            if !filled {
                if let Some((s, f)) = self.fetch_ranked_lcu(canonical).await {
                    solo = s;
                    flex = f;
                    ok = true;
                }
            }
            results.insert(canonical.clone(), (solo, flex, ok));
        }

        let mut out = Vec::new();
        for canonical in order {
            let Some((solo, flex, ok)) = results.get(&canonical) else {
                continue;
            };
            if !ok {
                continue;
            }
            let puuid = if is_puuid(&canonical) {
                canonical.clone()
            } else {
                String::new()
            };
            if let Some(input_ids) = buckets.get(&canonical) {
                for input_id in input_ids {
                    out.push(RankedInfo {
                        summoner_id: input_id.clone(),
                        puuid: puuid.clone(),
                        solo: solo.clone(),
                        flex: flex.clone(),
                    });
                }
            }
        }
        Ok(out)
    }

    async fn resolve_puuid(&self, summoner_id: &str) -> Option<String> {
        let path = PATH_SUMMONER_BY_ID.replace("%s", &path_escape(summoner_id));
        let (status, body) = self.http.get(&path).await.ok()?;
        if !(200..300).contains(&status) {
            return None;
        }
        let v: Value = serde_json::from_slice(&body).ok()?;
        let p = v["puuid"].as_str()?.trim().to_string();
        (!p.is_empty()).then_some(p)
    }

    async fn fetch_ranked_lcu(&self, id: &str) -> Option<(String, String)> {
        let path = PATH_RANKED_STATS_BY_SUMMONER.replace("%s", &path_escape(id));
        let (status, body) = self.http.get(&path).await.ok()?;
        if !(200..300).contains(&status) {
            return None;
        }
        let v: Value = serde_json::from_slice(&body).ok()?;

        let find = |queue: &str| -> (String, String, i32) {
            if let Some(e) = v.get("queueMap").and_then(|m| m.get(queue)) {
                let tier = e["tier"].as_str().unwrap_or("").to_string();
                let div = e["division"].as_str().unwrap_or("").to_string();
                if !tier.is_empty() || !div.is_empty() {
                    return (tier, div, e["leaguePoints"].as_i64().unwrap_or(0) as i32);
                }
            }
            for key in ["queues", "leagues"] {
                if let Some(arr) = v.get(key).and_then(|a| a.as_array()) {
                    for e in arr {
                        if e["queueType"].as_str() == Some(queue) {
                            let tier = e["tier"].as_str().unwrap_or("").to_string();
                            let div = e["division"].as_str().unwrap_or("").to_string();
                            if !tier.is_empty() || !div.is_empty() {
                                return (tier, div, e["leaguePoints"].as_i64().unwrap_or(0) as i32);
                            }
                        }
                    }
                }
            }
            (String::new(), String::new(), 0)
        };

        let (mut solo_t, mut solo_d, mut solo_lp) = find("RANKED_SOLO_5x5");
        let (flex_t, flex_d, flex_lp) = find("RANKED_FLEX_SR");

        if solo_t.is_empty() {
            if let Some(e) = v.get("highestRankedEntry") {
                let qt = e["queueType"].as_str().unwrap_or("");
                if qt.is_empty() || qt == "RANKED_SOLO_5x5" {
                    solo_t = e["tier"].as_str().unwrap_or("").to_string();
                    solo_d = e["division"].as_str().unwrap_or("").to_string();
                    solo_lp = e["leaguePoints"].as_i64().unwrap_or(0) as i32;
                }
            }
        }
        if solo_t.is_empty() {
            if let Some(t) = v["highestCurrentSeasonReachedTier"].as_str() {
                if !t.is_empty() {
                    solo_t = t.to_string();
                    solo_d = v["highestCurrentSeasonReachedDivision"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                }
            }
        }

        Some((
            ranked_display(&solo_t, &solo_d, solo_lp),
            ranked_display(&flex_t, &flex_d, flex_lp),
        ))
    }

    async fn fetch_sgp_tokens(&self) -> SgpTokens {
        {
            let cache = self.tok.lock().unwrap();
            if cache.valid() {
                return cache.toks.clone();
            }
        }

        let mut t = SgpTokens::default();
        if let Some(body) = self.get_ok(PATH_LEAGUE_SESSION_TOKEN).await {
            t.session = crate::sgp::parse_token_body(&body, "");
        }
        if let Some(body) = self.get_ok(PATH_ENTITLEMENTS_TOKEN).await {
            t.entitlements = crate::sgp::parse_token_body(&body, "accessToken");
        }

        let mut cache = self.tok.lock().unwrap();
        let has_any = !t.session.is_empty() || !t.entitlements.is_empty();
        cache.toks = t.clone();
        cache.at = Some(Instant::now());
        cache.ttl = if has_any {
            SGP_TOKEN_TTL
        } else {
            #[cfg(test)]
            {
                *self.tok_err_ttl.lock().unwrap()
            }
            #[cfg(not(test))]
            {
                SGP_TOKEN_ERR_TTL
            }
        };
        t
    }

    async fn get_ok(&self, path: &str) -> Option<Vec<u8>> {
        let (status, body) = self.http.get(path).await.ok()?;
        if (200..300).contains(&status) && !body.is_empty() {
            Some(body)
        } else {
            None
        }
    }

    /* ── 资源代理 ── */

    pub async fn get_asset(&self, kind: &str, id: i32) -> Result<AssetResult, String> {
        if id <= 0 {
            return Err(format!("资源不可用: 无效资源 id {id}"));
        }
        self.ensure_connected().await?;
        let key = format!("{kind}:{id}");
        if let Some((mime, data)) = self.assets.get_bytes(&key) {
            return Ok(AssetResult {
                kind: kind.to_string(),
                id,
                mime,
                data,
            });
        }

        let mut path = self.resolve_path(kind, id).await;
        if path.is_empty() {
            return Err(format!("资源不可用: {kind} {id} 无图标映射"));
        }

        let mut body = self.fetch_asset_bytes(&path).await;
        if kind == ASSET_ITEM && body.as_ref().map(|b| b.is_empty()).unwrap_or(true) {
            let idx = self.lookup_index(PATH_GD_ITEMS).await;
            if let Some(p) = idx.get(&id) {
                let alt = normalize_asset_path(p);
                if !alt.is_empty() {
                    if let Some(b) = self.fetch_asset_bytes(&alt).await {
                        path = alt;
                        body = Some(b);
                    }
                }
            }
        }

        let body = body
            .filter(|b| !b.is_empty())
            .ok_or_else(|| format!("资源不可用: {kind} {id}"))?;
        let mime = mime_by_ext(&path);
        let data = base64::engine::general_purpose::STANDARD.encode(&body);
        self.assets.put_bytes(&key, mime.clone(), data.clone());
        Ok(AssetResult {
            kind: kind.to_string(),
            id,
            mime,
            data,
        })
    }

    async fn resolve_path(&self, kind: &str, id: i32) -> String {
        match kind {
            ASSET_CHAMPION => PATH_GD_CHAMPION_ICON.replace("%d", &id.to_string()),
            ASSET_PROFILE => PATH_GD_PROFILE_ICON.replace("%d", &id.to_string()),
            ASSET_ITEM => PATH_GD_ITEM_ICON.replace("%d", &id.to_string()),
            ASSET_SPELL => {
                let idx = self.lookup_index(PATH_GD_SPELLS).await;
                normalize_asset_path(idx.get(&id).map(|s| s.as_str()).unwrap_or(""))
            }
            ASSET_PERK => {
                let idx = self.lookup_index(PATH_GD_PERKS).await;
                normalize_asset_path(idx.get(&id).map(|s| s.as_str()).unwrap_or(""))
            }
            ASSET_AUGMENT => {
                let idx = self.lookup_index(PATH_GD_AUGMENTS).await;
                normalize_asset_path(idx.get(&id).map(|s| s.as_str()).unwrap_or(""))
            }
            _ => String::new(),
        }
    }

    async fn fetch_asset_bytes(&self, path: &str) -> Option<Vec<u8>> {
        let (status, body) = self.http.get(path).await.ok()?;
        if (200..300).contains(&status) && !body.is_empty() {
            Some(body)
        } else {
            None
        }
    }

    async fn lookup_index(&self, json_path: &str) -> HashMap<i32, String> {
        if let Some(paths) = self.assets.get_index(json_path) {
            return paths;
        }
        let mut paths = HashMap::new();
        if let Some(body) = self.get_ok(json_path).await {
            if let Ok(arr) = serde_json::from_slice::<Value>(&body) {
                if let Some(list) = arr.as_array() {
                    for e in list {
                        let id = e["id"].as_i64().unwrap_or(0) as i32;
                        let p = e["iconPath"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                e["augmentSmallImagePath"]
                                    .as_str()
                                    .filter(|s| !s.is_empty())
                            })
                            .or_else(|| {
                                e["augmentLargeImagePath"]
                                    .as_str()
                                    .filter(|s| !s.is_empty())
                            })
                            .unwrap_or("");
                        if id > 0 && !p.is_empty() {
                            paths.insert(id, p.to_string());
                        }
                    }
                }
            }
        }
        self.assets.put_index(json_path, paths.clone());
        paths
    }
}

/* ── 纯函数 ───────────────────────────────────────────────── */

fn display_name_of(game_name: &str, tag_line: &str, fallback: &str) -> String {
    let name = if game_name.is_empty() {
        fallback
    } else {
        game_name
    };
    if name.is_empty() {
        return String::new();
    }
    if !tag_line.is_empty() {
        format!("{name}#{tag_line}")
    } else {
        name.to_string()
    }
}

pub fn is_puuid(id: &str) -> bool {
    id.len() >= 32 && id.contains('-')
}

pub fn ranked_display(tier: &str, division: &str, lp: i32) -> String {
    let cn = tier_cn(tier);
    if cn.is_empty() {
        return UNRANKED.to_string();
    }
    let mut parts = vec![cn];
    if !division.is_empty() {
        parts.push(division.to_string());
    }
    if lp > 0 {
        parts.push(lp.to_string());
    }
    parts.join(" ")
}

pub fn sgp_ranked_display(stats: &crate::sgp::RankedStats) -> (String, String) {
    let find = |queue: &str| -> (String, String, i32) {
        for q in &stats.queues {
            if q.queue_type == queue && !q.tier.is_empty() {
                return (q.tier.clone(), q.div().to_string(), q.league_points);
            }
        }
        (String::new(), String::new(), 0)
    };
    let (st, sd, sl) = find("RANKED_SOLO_5x5");
    let (ft, fd, fl) = find("RANKED_FLEX_SR");
    (ranked_display(&st, &sd, sl), ranked_display(&ft, &fd, fl))
}

pub fn normalize_asset_path(p: &str) -> String {
    let p = p.trim();
    if p.is_empty() {
        return String::new();
    }
    if p.starts_with("http://") || p.starts_with("https://") {
        return p.to_string();
    }
    if p.starts_with("/lol-game-data/assets") {
        return p.to_string();
    }
    if let Some(rest) = p.strip_prefix("/lol-game-data/") {
        return format!("/lol-game-data/assets/{rest}");
    }
    if let Some(rest) = p.strip_prefix("assets/") {
        return format!("/lol-game-data/assets/{rest}");
    }
    if p.starts_with("/fe/") {
        return p.to_string();
    }
    format!("/lol-game-data/assets/{}", p.trim_start_matches('/'))
}

pub fn mime_by_ext(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png".into()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".into()
    } else if lower.ends_with(".svg") {
        "image/svg+xml".into()
    } else if lower.ends_with(".webp") {
        "image/webp".into()
    } else if lower.ends_with(".gif") {
        "image/gif".into()
    } else {
        "application/octet-stream".into()
    }
}

fn flex_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

#[derive(Deserialize)]
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
    #[serde(default, rename = "summonerLevel")]
    summoner_level: i64,
    #[serde(default, rename = "summonerId", deserialize_with = "de_flex_str")]
    summoner_id: String,
}

fn de_flex_str<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Value::deserialize(d)?;
    Ok(flex_str(&v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::http::FakeHttp;
    use std::sync::atomic::AtomicU32;

    fn sgp_canned() -> crate::sgp::RankedStats {
        crate::sgp::RankedStats {
            queues: vec![
                crate::sgp::QueueEntry {
                    queue_type: "RANKED_FLEX_SR".into(),
                    tier: "PLATINUM".into(),
                    rank: "IV".into(),
                    league_points: 91,
                    ..Default::default()
                },
                crate::sgp::QueueEntry {
                    queue_type: "RANKED_SOLO_5x5".into(),
                    tier: "GOLD".into(),
                    rank: "IV".into(),
                    league_points: 45,
                    ..Default::default()
                },
                crate::sgp::QueueEntry {
                    queue_type: "JADE_RANKED_SOLO_5x5".into(),
                    tier: "WOOD".into(),
                    rank: "I".into(),
                    ..Default::default()
                },
                crate::sgp::QueueEntry::default(),
            ],
        }
    }

    #[test]
    fn ranked_display_cases() {
        assert_eq!(ranked_display("GOLD", "IV", 45), "黄金 IV 45");
        assert_eq!(ranked_display("", "", 0), "未定级");
        assert_eq!(ranked_display("UNRANKED", "I", 0), "未定级");
        assert_eq!(ranked_display("DIAMOND", "II", 0), "钻石 II");
    }

    #[test]
    fn is_puuid_cases() {
        assert!(is_puuid("5e65c58d-5b4a-5936-9104-806bb8443eef"));
        assert!(!is_puuid("2001"));
        assert!(!is_puuid("16262047083"));
        assert!(!is_puuid("PUUID-1"));
        assert!(!is_puuid(""));
    }

    #[test]
    fn sgp_ranked_display_filters_ghost_queues() {
        let (solo, flex) = sgp_ranked_display(&sgp_canned());
        assert_eq!(solo, "黄金 IV 45");
        assert_eq!(flex, "白金 IV 91");
        let (s, f) = sgp_ranked_display(&crate::sgp::RankedStats { queues: vec![] });
        assert_eq!(s, "未定级");
        assert_eq!(f, "未定级");
    }

    #[test]
    fn normalize_asset_path_cases() {
        assert_eq!(
            normalize_asset_path("/lol-game-data/assets/DATA/x.png"),
            "/lol-game-data/assets/DATA/x.png"
        );
        assert_eq!(
            normalize_asset_path("/lol-game-data/v1/perks/a.png"),
            "/lol-game-data/assets/v1/perks/a.png"
        );
        assert_eq!(
            normalize_asset_path("assets/items/i.png"),
            "/lol-game-data/assets/items/i.png"
        );
        assert_eq!(
            normalize_asset_path("/fe/lol-loot/aug.png"),
            "/fe/lol-loot/aug.png"
        );
        assert_eq!(normalize_asset_path(""), "");
        assert_eq!(
            normalize_asset_path("ASSETS/Items/Icons2D/3157.png"),
            "/lol-game-data/assets/ASSETS/Items/Icons2D/3157.png"
        );
    }

    #[test]
    fn page_size_bounds() {
        let http: Arc<dyn LcuHttp> = Arc::new(FakeHttp::new(ConnStatus::default()));
        let svc = HistoryService::new(http, 20);
        svc.set_page_size(10);
        assert_eq!(svc.current_page_size(), 10);
        svc.set_page_size(3);
        assert_eq!(svc.current_page_size(), 10);
        let svc2 = HistoryService::new(Arc::new(FakeHttp::new(ConnStatus::default())), 99);
        assert_eq!(svc2.current_page_size(), 20);
    }

    #[tokio::test]
    async fn get_matches_empty_puuid_and_offline() {
        let http: Arc<dyn LcuHttp> = Arc::new(FakeHttp::new(ConnStatus::default()));
        let svc = HistoryService::new(http, 20);
        let err = svc.get_matches("", 0).await.unwrap_err();
        assert!(err.contains("puuid"), "err={err}");
        let err = svc.get_matches("X", 0).await.unwrap_err();
        assert!(err.contains("未连接"), "err={err}");
    }

    #[tokio::test]
    async fn get_matches_pagination_and_parse() {
        let fixture = r#"{"games":{"games":[
            {"gameId":1,"gameCreation":1705329000000,"gameDuration":924,"queueId":2400,
             "participants":[{"participantId":1,"championId":53,
             "stats":{"win":true,"kills":4,"deaths":0,"assists":9}}]}],
            "gameCount":45}}"#;
        let last_path: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let http = FakeHttp::connected().with_handler({
            let last_path = last_path.clone();
            let fixture = fixture.to_string();
            move |path: &str| {
                *last_path.lock().unwrap() = path.to_string();
                if path.contains("/lol-match-history/") {
                    Ok((200, fixture.as_bytes().to_vec()))
                } else {
                    Ok((404, vec![]))
                }
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        svc.set_sgp_enabled(false);

        let page = svc.get_matches("PUUID-1", 1).await.unwrap();
        let got = last_path.lock().unwrap().clone();
        assert!(
            got.starts_with("/lol-match-history/v1/products/lol/PUUID-1/matches?"),
            "path={got}"
        );
        assert!(got.contains("begIndex=20&endIndex=39"), "query={got}");
        assert_eq!(page.game_count, 45);
        assert_eq!(page.total_pages, 3);
        assert!(page.has_more);
        assert_eq!(page.page, 1);
        assert_eq!(page.page_size, 20);
        assert_eq!(page.summaries.len(), 1);
        assert_eq!(page.summaries[0].queue_short, "海斗");
        assert_eq!(page.summaries[0].kda, "Perfect");

        let end = svc.get_matches("PUUID-1", 3).await.unwrap();
        assert!(!end.has_more, "beyond end");
    }

    #[tokio::test]
    async fn get_matches_http_errors() {
        let http = FakeHttp::connected().with_handler(|path: &str| {
            if path.contains("HTTP403") {
                Ok((403, vec![]))
            } else if path.contains("HTTP404") {
                Ok((404, vec![]))
            } else {
                Ok((403, vec![]))
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        svc.set_sgp_enabled(false);
        let err = svc.get_matches("X", 0).await.unwrap_err();
        assert!(err.contains("HTTP 403"), "err={err}");
    }

    #[tokio::test]
    async fn search_summoner_found_and_404() {
        let http = FakeHttp::connected().with_handler(|path: &str| {
            if path.starts_with("/lol-summoner/v1/summoners?") {
                Ok((
                    200,
                    br#"{"puuid":"P-1","gameName":"xxx","tagLine":"CN1","profileIconId":29,"summonerLevel":156,"summonerId":999}"#
                        .to_vec(),
                ))
            } else {
                Ok((404, vec![]))
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        let res = svc.search_summoner(" 安静的亚索#CN1 ").await.unwrap();
        assert_eq!(res.puuid, "P-1");
        assert_eq!(res.display_name, "xxx#CN1");
        assert_eq!(res.summoner_id, "999");
        assert_eq!(res.summoner_level, 156);

        let err = svc.search_summoner("  ").await.unwrap_err();
        assert!(err.contains("请输入"));

        let http2 = FakeHttp::connected().with_handler(|_| Ok((404, vec![])));
        let svc2 = HistoryService::new(Arc::new(http2), 20);
        let err = svc2.search_summoner("不存在的人#CN1").await.unwrap_err();
        assert!(err.contains("未找到"), "err={err}");
    }

    #[tokio::test]
    async fn get_self_summoner_enrichment() {
        let http = FakeHttp::connected().with_handler(|path: &str| {
            if path.contains("/by-puuid/PSELF") {
                Ok((
                    200,
                    br#"{"summonerId":"SID-9","profileIconId":42,"summonerLevel":310}"#.to_vec(),
                ))
            } else {
                Ok((404, vec![]))
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        let res = svc.get_self_summoner().await.unwrap();
        assert_eq!(res.display_name, "歪比巴卜小宝贝#60021");
        assert_eq!(res.summoner_id, "SID-9");
        assert_eq!(res.summoner_level, 310);

        let http2 = FakeHttp::new(ConnStatus {
            state: State::Unauthenticated,
            ..Default::default()
        });
        let svc2 = HistoryService::new(Arc::new(http2), 20);
        let err = svc2.get_self_summoner().await.unwrap_err();
        assert!(err.contains("未登录"), "err={err}");
    }

    #[tokio::test]
    async fn get_match_detail_plumbing() {
        let detail = r#"{"gameId":8000000001,"gameCreation":1705329000000,"gameDuration":924,"queueId":420,
            "participantIdentities":[
                {"participantId":1,"player":{"puuid":"PSELF","gameName":"歪比","tagLine":"60021","summonerId":"S1"}},
                {"participantId":2,"player":{"puuid":"P2","gameName":"B","tagLine":"0002","summonerId":"S2"}}],
            "participants":[
                {"participantId":1,"teamID":100,"championId":53,"stats":{"win":false,"kills":4,"deaths":11,"assists":20,"goldEarned":11000,"totalDamageDealtToChampions":17000}},
                {"participantId":2,"teamID":200,"championId":22,"stats":{"win":true,"kills":9,"deaths":3,"assists":12,"goldEarned":14000,"totalDamageDealtToChampions":23000}}]}"#;
        let got_path: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let http = FakeHttp::connected().with_handler({
            let got_path = got_path.clone();
            let detail = detail.to_string();
            move |path: &str| {
                *got_path.lock().unwrap() = path.to_string();
                if path.starts_with("/lol-match-history/v1/games/") {
                    Ok((200, detail.as_bytes().to_vec()))
                } else {
                    Ok((404, vec![]))
                }
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        let d = svc.get_match_detail(8000000001, "PSELF").await.unwrap();
        assert_eq!(
            got_path.lock().unwrap().as_str(),
            "/lol-match-history/v1/games/8000000001"
        );
        assert_eq!(d.game_id, 8000000001);
        assert_eq!(d.teams.len(), 2);
        assert_eq!(d.teams[0].team_id, 100);
        assert!(d.teams[0].players.iter().any(|p| p.is_self));
        assert!(!d.teams[0].win);

        let err = svc.get_match_detail(0, "PSELF").await.unwrap_err();
        assert!(err.contains("无效"));
    }

    #[tokio::test]
    async fn get_players_ranked_best_effort() {
        let http = FakeHttp::connected().with_handler(|path: &str| {
            if path.ends_with("/S1") {
                Ok((
                    200,
                    br#"{"queueMap":{"RANKED_SOLO_5x5":{"tier":"GOLD","division":"IV","leaguePoints":45},"RANKED_FLEX_SR":{"tier":"SILVER","division":"II","leaguePoints":10}}}"#.to_vec(),
                ))
            } else if path.ends_with("/S2") {
                Ok((
                    200,
                    br#"{"queues":[{"queueType":"RANKED_SOLO_5x5","tier":"","division":"","leaguePoints":0}]}"#.to_vec(),
                ))
            } else if path.ends_with("/PUUID-1") {
                Ok((
                    200,
                    br#"{"leagues":[{"queueType":"RANKED_SOLO_5x5","tier":"PLATINUM","division":"II","leaguePoints":12}]}"#.to_vec(),
                ))
            } else if path.ends_with("/PUUID-2") {
                Ok((
                    200,
                    br#"{"highestRankedEntry":{"queueType":"RANKED_SOLO_5x5","tier":"EMERALD","division":"IV","leaguePoints":3}}"#.to_vec(),
                ))
            } else {
                Ok((500, vec![]))
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        svc.set_sgp_enabled(false);
        let ids: Vec<String> = ["S1", "S2", "S3", "", "S1", "PUUID-1", "PUUID-2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let infos = svc.get_players_ranked(&ids).await.unwrap();
        let mut by: HashMap<&str, &RankedInfo> = HashMap::new();
        for r in &infos {
            by.insert(r.summoner_id.as_str(), r);
        }
        assert_eq!(infos.len(), 4, "infos={infos:?}");
        assert_eq!(by["S1"].solo, "黄金 IV 45");
        assert_eq!(by["S1"].flex, "白银 II 10");
        assert_eq!(by["S2"].solo, "未定级");
        assert!(!by.contains_key("S3"), "failed id absent");
        assert_eq!(by["PUUID-1"].solo, "白金 II 12");
        assert_eq!(by["PUUID-2"].solo, "翡翠 IV 3");
    }

    #[tokio::test]
    async fn get_asset_champion_cache() {
        let hits = Arc::new(AtomicU32::new(0));
        let http = FakeHttp::connected().with_handler({
            let hits = hits.clone();
            move |path: &str| {
                if path == "/lol-game-data/assets/v1/champion-icons/42.png" {
                    hits.fetch_add(1, Ordering::SeqCst);
                    Ok((200, b"PNGDATA".to_vec()))
                } else {
                    Ok((404, vec![]))
                }
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        let want = base64::engine::general_purpose::STANDARD.encode(b"PNGDATA");
        for _ in 0..2 {
            let res = svc.get_asset("champion", 42).await.unwrap();
            assert_eq!(res.data, want);
            assert_eq!(res.mime, "image/png");
            assert_eq!(res.kind, "champion");
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1, "cache miss");
    }

    #[tokio::test]
    async fn get_asset_spell_via_index() {
        let index_hits = Arc::new(AtomicU32::new(0));
        let http = FakeHttp::connected().with_handler({
            let index_hits = index_hits.clone();
            move |path: &str| match path {
                "/lol-game-data/assets/v1/summoner-spells.json" => {
                    index_hits.fetch_add(1, Ordering::SeqCst);
                    Ok((
                        200,
                        br#"[{"id":4,"iconPath":"/lol-game-data/assets/DATA/Spells/Icons2D/SummonerFlash.png"}]"#
                            .to_vec(),
                    ))
                }
                "/lol-game-data/assets/DATA/Spells/Icons2D/SummonerFlash.png" => {
                    Ok((200, b"FLASH".to_vec()))
                }
                _ => Ok((404, vec![])),
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        let res = svc.get_asset("spell", 4).await.unwrap();
        assert_eq!(
            res.data,
            base64::engine::general_purpose::STANDARD.encode(b"FLASH")
        );
        assert_eq!(res.mime, "image/png");
        let _ = svc.get_asset("spell", 4).await.unwrap();
        assert_eq!(index_hits.load(Ordering::SeqCst), 1, "index TTL");
        assert!(svc.get_asset("spell", 999).await.is_err());
    }

    #[tokio::test]
    async fn get_asset_item_fallback_and_augment() {
        let http = FakeHttp::connected().with_handler(|path: &str| {
            match path {
            "/lol-game-data/assets/v1/items/icons2d/3157.png" => Ok((404, vec![])),
            "/lol-game-data/assets/v1/items.json" => Ok((
                200,
                br#"[{"id":3157,"iconPath":"/lol-game-data/assets/ASSETS/Items/Icons2D/3157.png"}]"#
                    .to_vec(),
            )),
            "/lol-game-data/assets/ASSETS/Items/Icons2D/3157.png" => Ok((200, b"ITEM".to_vec())),
            "/lol-game-data/assets/v1/cherry-augments.json" => Ok((
                200,
                br#"[{"id":7018,"augmentSmallImagePath":"/fe/lol-loot/aug_7018.png"}]"#.to_vec(),
            )),
            "/fe/lol-loot/aug_7018.png" => Ok((200, b"AUG".to_vec())),
            _ => Ok((404, vec![])),
        }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        let item = svc.get_asset("item", 3157).await.unwrap();
        assert_eq!(
            item.data,
            base64::engine::general_purpose::STANDARD.encode(b"ITEM")
        );
        let aug = svc.get_asset("augment", 7018).await.unwrap();
        assert_eq!(
            aug.data,
            base64::engine::general_purpose::STANDARD.encode(b"AUG")
        );
        assert_eq!(aug.mime, "image/png");
        assert!(svc.get_asset("unknown-kind", 1).await.is_err());
        assert!(svc.get_asset("champion", 0).await.is_err());
    }

    /* ── SGP ── */

    const TEST_PUUID: &str = "5e65c58d-5b4a-5936-9104-806bb8443eef";

    fn sgp_base_http(token_ok: bool) -> FakeHttp {
        FakeHttp::connected().with_handler(move |path: &str| match path {
            "/lol-summoner/v1/summoners/2001" => {
                Ok((200, format!("{{\"puuid\":\"{TEST_PUUID}\"}}").into_bytes()))
            }
            p if p.contains("/ranked-stats/") => Ok((200, br#"{"queueMap":{}}"#.to_vec())),
            "/lol-league-session/v1/league-session-token" if token_ok => {
                Ok((200, br#""test-league-session-token""#.to_vec()))
            }
            "/lol-league-session/v1/league-session-token" => Ok((404, vec![])),
            _ => Ok((404, vec![])),
        })
    }

    #[tokio::test]
    async fn sgp_ranked_dual_key_single_query() {
        let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let mut svc = HistoryService::new(Arc::new(sgp_base_http(true)), 20);
        svc.set_sgp_enabled(true);
        {
            let calls = calls.clone();
            svc.set_sgp_ranked_fetch(Arc::new(move |host, puuid, token| {
                let calls = calls.clone();
                Box::pin(async move {
                    calls
                        .lock()
                        .unwrap()
                        .push(format!("{host}|{puuid}|{token}"));
                    Ok(sgp_canned())
                })
            }));
        }
        let infos = svc
            .get_players_ranked(&["2001".into(), TEST_PUUID.into()])
            .await
            .unwrap();
        assert_eq!(calls.lock().unwrap().len(), 1, "dedup single query");
        let want =
            format!("https://gz100-sgp.lol.qq.com:21019|{TEST_PUUID}|test-league-session-token");
        assert_eq!(calls.lock().unwrap()[0], want);
        let mut by: HashMap<&str, &RankedInfo> = HashMap::new();
        for r in &infos {
            by.insert(r.summoner_id.as_str(), r);
        }
        assert_eq!(infos.len(), 2);
        for key in ["2001", TEST_PUUID] {
            let r = by[key];
            assert_eq!(r.solo, "黄金 IV 45");
            assert_eq!(r.flex, "白金 IV 91");
            assert_eq!(r.puuid, TEST_PUUID);
        }
    }

    #[tokio::test]
    async fn sgp_disabled_skips_fetch() {
        let count = Arc::new(AtomicU32::new(0));
        let mut svc = HistoryService::new(Arc::new(sgp_base_http(true)), 20);
        svc.set_sgp_enabled(false);
        let c2 = count.clone();
        svc.set_sgp_ranked_fetch(Arc::new(move |_, _, _| {
            let c = c2.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(sgp_canned())
            })
        }));
        let infos = svc.get_players_ranked(&["2001".into()]).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0, "disabled no sgp");
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].solo, "未定级", "LCU empty tier");
    }

    #[tokio::test]
    async fn sgp_fail_soft_to_lcu() {
        let count = Arc::new(AtomicU32::new(0));
        let http = FakeHttp::connected().with_handler(|path: &str| {
            if path.contains("/ranked-stats/") {
                Ok((
                    200,
                    br#"{"queues":[{"queueType":"RANKED_SOLO_5x5","tier":"SILVER","division":"II","leaguePoints":7}]}"#.to_vec(),
                ))
            } else if path == "/lol-league-session/v1/league-session-token" {
                Ok((200, br#""t""#.to_vec()))
            } else {
                Ok((404, vec![]))
            }
        });
        let mut svc = HistoryService::new(Arc::new(http), 20);
        svc.set_sgp_enabled(true);
        let c2 = count.clone();
        svc.set_sgp_ranked_fetch(Arc::new(move |_, _, _| {
            let c = c2.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err("sgp timeout".into())
            })
        }));
        let infos = svc.get_players_ranked(&[TEST_PUUID.into()]).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].solo, "白银 II 7", "LCU fallback");
    }

    #[tokio::test]
    async fn sgp_match_priority_entitlements_first() {
        let lcu_hits = Arc::new(AtomicU32::new(0));
        let tokens: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sgp_fixture = r#"{"games":[{"metadata":{"participants":["5e65c58d-5b4a-5936-9104-806bb8443eef"]},
            "json":{"gameId":900001,"gameCreation":1758000000000,"gameDuration":1200,"queueId":420,"mapId":11,
            "participants":[{"puuid":"5e65c58d-5b4a-5936-9104-806bb8443eef","teamId":100,"championId":22,"champLevel":16,
            "spell1Id":7,"spell2Id":6,"kills":6,"deaths":1,"assists":8,"win":true,
            "item0":10,"item1":0,"item2":0,"item3":0,"item4":0,"item5":0,"item6":3340,
            "totalMinionsKilled":205,"neutralMinionsKilled":15,"goldEarned":14200,
            "totalDamageDealtToChampions":18500,"totalHeal":420,
            "perks":{"statPerks":{},"styles":[{"style":8100,"selections":[{"perk":8112}]}]}}]}}]}"#;
        let lcu_fixture = r#"{"games":{"games":[{"gameId":800001,"gameCreation":1758000000000,"gameDuration":900,
            "queueId":450,"mapId":11,
            "participants":[{"participantId":1,"teamID":100,"championId":99,"spell1Id":4,"spell2Id":6,
            "stats":{"win":false,"kills":3,"deaths":1,"assists":5,"champLevel":14}}],
            "participantIdentities":[{"participantId":1,"player":{"puuid":"5e65c58d-5b4a-5936-9104-806bb8443eef"}}]}],"gameCount":40}}"#;

        let http = FakeHttp::connected().with_handler({
            let lcu_hits = lcu_hits.clone();
            let lcu_fixture = lcu_fixture.to_string();
            move |path: &str| match path {
                "/lol-league-session/v1/league-session-token" => {
                    Ok((200, br#""test-league-session-token""#.to_vec()))
                }
                "/entitlements/v1/token" => {
                    Ok((200, br#"{"accessToken":"test-ent-token"}"#.to_vec()))
                }
                p if p.contains("/lol-match-history/") => {
                    lcu_hits.fetch_add(1, Ordering::SeqCst);
                    Ok((200, lcu_fixture.as_bytes().to_vec()))
                }
                _ => Ok((404, vec![])),
            }
        });
        let mut svc = HistoryService::new(Arc::new(http), 20);
        svc.set_sgp_enabled(true);
        let got_tokens = tokens.clone();
        let fixture = sgp_fixture.to_string();
        svc.set_sgp_match_fetch(Arc::new(move |host, puuid, token, start, count| {
            let got_tokens = got_tokens.clone();
            let fixture = fixture.clone();
            Box::pin(async move {
                got_tokens.lock().unwrap().push(token);
                assert_eq!(host, "https://gz100-sgp.lol.qq.com:21019");
                assert_eq!(puuid, TEST_PUUID);
                assert_eq!(start, 0);
                assert_eq!(count, 20);
                Ok(fixture.into_bytes())
            })
        }));

        let page = svc.get_matches(TEST_PUUID, 0).await.unwrap();
        assert_eq!(tokens.lock().unwrap().len(), 1);
        assert_eq!(tokens.lock().unwrap()[0], "test-ent-token");
        assert_eq!(lcu_hits.load(Ordering::SeqCst), 0, "SGP hit skips LCU");
        assert_eq!(page.summaries.len(), 1);
        assert_eq!(page.summaries[0].game_id, 900001);
        assert!(!page.has_more);
    }

    #[tokio::test]
    async fn sgp_match_empty_page0_falls_back_to_lcu() {
        let lcu_hits = Arc::new(AtomicU32::new(0));
        let sgp_hits = Arc::new(AtomicU32::new(0));
        let http = FakeHttp::connected().with_handler({
            let lcu_hits = lcu_hits.clone();
            move |path: &str| match path {
                "/lol-league-session/v1/league-session-token" => Ok((200, br#""t""#.to_vec())),
                "/entitlements/v1/token" => Ok((200, br#"{"accessToken":"e"}"#.to_vec())),
                p if p.contains("/lol-match-history/") => {
                    lcu_hits.fetch_add(1, Ordering::SeqCst);
                    Ok((404, vec![]))
                }
                _ => Ok((404, vec![])),
            }
        });
        let mut svc = HistoryService::new(Arc::new(http), 20);
        svc.set_sgp_enabled(true);
        let sgp_hits2 = sgp_hits.clone();
        svc.set_sgp_match_fetch(Arc::new(move |_, _, _, _, _| {
            let c = sgp_hits2.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(br#"{"games":[]}"#.to_vec())
            })
        }));
        let err = svc.get_matches(TEST_PUUID, 0).await.unwrap_err();
        assert!(err.contains("未找到战绩数据"), "err={err}");
        assert_eq!(sgp_hits.load(Ordering::SeqCst), 1);
        assert_eq!(lcu_hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn sgp_disabled_no_match_call() {
        let lcu_hits = Arc::new(AtomicU32::new(0));
        let sgp_hits = Arc::new(AtomicU32::new(0));
        let lcu_fixture = r#"{"games":{"games":[{"gameId":800001,"gameCreation":1758000000000,"gameDuration":900,
            "queueId":450,"mapId":11,
            "participants":[{"participantId":1,"teamID":100,"championId":99,"spell1Id":4,"spell2Id":6,
            "stats":{"win":false,"kills":3,"deaths":1,"assists":5,"champLevel":14}}],
            "participantIdentities":[{"participantId":1,"player":{"puuid":"5e65c58d-5b4a-5936-9104-806bb8443eef"}}]}],"gameCount":40}}"#;
        let http = FakeHttp::connected().with_handler({
            let lcu_hits = lcu_hits.clone();
            let lcu_fixture = lcu_fixture.to_string();
            move |path: &str| {
                if path.contains("/lol-match-history/") {
                    lcu_hits.fetch_add(1, Ordering::SeqCst);
                    Ok((200, lcu_fixture.as_bytes().to_vec()))
                } else {
                    Ok((404, vec![]))
                }
            }
        });
        let mut svc = HistoryService::new(Arc::new(http), 20);
        svc.set_sgp_enabled(false);
        let sgp_hits2 = sgp_hits.clone();
        svc.set_sgp_match_fetch(Arc::new(move |_, _, _, _, _| {
            let c = sgp_hits2.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(b"".to_vec())
            })
        }));
        let page = svc.get_matches(TEST_PUUID, 0).await.unwrap();
        assert_eq!(sgp_hits.load(Ordering::SeqCst), 0);
        assert_eq!(lcu_hits.load(Ordering::SeqCst), 1);
        assert_eq!(page.summaries[0].game_id, 800001);
    }

    #[tokio::test]
    async fn sgp_tokens_cached_and_failure_not_sticky() {
        let session_hits = Arc::new(AtomicU32::new(0));
        let ent_hits = Arc::new(AtomicU32::new(0));
        let http = FakeHttp::connected().with_handler({
            let s = session_hits.clone();
            let e = ent_hits.clone();
            move |path: &str| match path {
                "/lol-league-session/v1/league-session-token" => {
                    s.fetch_add(1, Ordering::SeqCst);
                    Ok((200, br#""sess-tok""#.to_vec()))
                }
                "/entitlements/v1/token" => {
                    e.fetch_add(1, Ordering::SeqCst);
                    Ok((200, br#"{"accessToken":"ent-tok"}"#.to_vec()))
                }
                _ => Ok((404, vec![])),
            }
        });
        let svc = HistoryService::new(Arc::new(http), 20);
        let t1 = svc.fetch_sgp_tokens().await;
        let t2 = svc.fetch_sgp_tokens().await;
        let t3 = svc.fetch_sgp_tokens().await;
        assert_eq!(t1.session, "sess-tok");
        assert_eq!(t1.entitlements, "ent-tok");
        assert_eq!(t2.session, t1.session);
        assert_eq!(t3.session, t1.session);
        assert_eq!(session_hits.load(Ordering::SeqCst), 1);
        assert_eq!(ent_hits.load(Ordering::SeqCst), 1);

        let hits = Arc::new(AtomicU32::new(0));
        let http2 = FakeHttp::connected().with_handler({
            let h = hits.clone();
            move |path: &str| {
                if path.contains("token") {
                    h.fetch_add(1, Ordering::SeqCst);
                    Ok((500, vec![]))
                } else {
                    Ok((404, vec![]))
                }
            }
        });
        let svc2 = HistoryService::new(Arc::new(http2), 20);
        svc2.set_tok_err_ttl(Duration::ZERO);
        let tok = svc2.fetch_sgp_tokens().await;
        assert!(tok.session.is_empty() && tok.entitlements.is_empty());
        let first = hits.load(Ordering::SeqCst);
        assert_eq!(first, 2, "both token endpoints hit");
        let _ = svc2.fetch_sgp_tokens().await;
        assert!(
            hits.load(Ordering::SeqCst) > first,
            "errTTL expired should retry"
        );
    }
}
