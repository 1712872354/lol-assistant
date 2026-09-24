//! LCU 战绩解析（对齐 Go internal/parser/match.go）。
//! JSON 字段契约：列表 `{games:{games:[...],gameCount}}`，明细为单个 game 对象。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::queue::queue_info_for;
use crate::util::{flex_value, opt_id};

/* ─── 输出视图模型 ─────────────────────────────────────────────── */

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    pub game_id: i64,
    pub queue_id: i32,
    pub queue_name: String,
    pub queue_short: String,
    pub arena: bool,
    pub game_creation: i64,
    pub game_duration: i32,
    pub short_time: String,
    pub duration: String,
    pub champion_id: i32,
    pub champ_level: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub rune_id: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub kda: String,
    pub win: bool,
    pub remake: bool,
    pub placement: i32,
    pub items: Vec<i32>,
    pub gold: i32,
    pub total_damage: i32,
    pub augment_ids: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRow {
    pub participant_id: i32,
    pub team_id: i32,
    pub placement: i32,
    pub puuid: Option<String>,
    pub summoner_id: Option<String>,
    pub name: String,
    pub profile_icon_id: i32,
    pub champion_id: i32,
    pub champ_level: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub rune_id: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub kda: String,
    pub items: Vec<i32>,
    pub gold: i32,
    pub total_damage: i32,
    pub win: bool,
    pub remake: bool,
    pub augment_ids: Vec<i32>,
    pub tier_short: String,
    pub dmg_ratio: f64,
    pub match_rating: f64,
    pub rating_rank: i32,
    pub kill_participation: i32,
    pub is_self: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub team_id: i32,
    pub placement: i32,
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub gold: i32,
    pub damage: i32,
    pub players: Vec<PlayerRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetail {
    pub game_id: i64,
    pub queue_id: i32,
    pub queue_name: String,
    pub arena: bool,
    pub game_creation: i64,
    pub time: String,
    pub game_duration: i32,
    pub duration: String,
    pub duration_min: String,
    pub remake: bool,
    pub self_puuid: String,
    pub teams: Vec<TeamSummary>,
}

/* ─── LCU 原始 JSON 结构 ───────────────────────────────────────── */

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LcuIdentity {
    #[serde(rename = "participantId")]
    pub(crate) participant_id: i32,
    #[serde(default)]
    pub(crate) player: LcuIdentityPlayer,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct LcuIdentityPlayer {
    #[serde(default)]
    pub(crate) puuid: Value,
    #[serde(default, rename = "summonerId")]
    pub(crate) summoner_id: Value,
    #[serde(default, rename = "gameName")]
    game_name: String,
    #[serde(default, rename = "tagLine")]
    tag_line: String,
    #[serde(default, rename = "summonerName")]
    summoner_name: String,
    #[serde(default, rename = "displayName")]
    display_name: String,
    #[serde(default, rename = "profileIcon")]
    profile_icon: i32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct LcuStats {
    #[serde(default)]
    pub(crate) win: bool,
    #[serde(default)]
    pub(crate) kills: i32,
    #[serde(default)]
    pub(crate) deaths: i32,
    #[serde(default)]
    pub(crate) assists: i32,
    #[serde(default, rename = "champLevel")]
    pub(crate) champ_level: i32,
    #[serde(default, rename = "item0")]
    pub(crate) item0: i32,
    #[serde(default, rename = "item1")]
    pub(crate) item1: i32,
    #[serde(default, rename = "item2")]
    pub(crate) item2: i32,
    #[serde(default, rename = "item3")]
    pub(crate) item3: i32,
    #[serde(default, rename = "item4")]
    pub(crate) item4: i32,
    #[serde(default, rename = "item5")]
    pub(crate) item5: i32,
    #[serde(default, rename = "item6")]
    pub(crate) item6: i32,
    #[serde(default, rename = "perk0")]
    pub(crate) perk0: i32,
    #[serde(default, rename = "goldEarned")]
    pub(crate) gold_earned: i32,
    #[serde(default, rename = "totalDamageDealtToChampions")]
    pub(crate) total_damage_dealt_to_champions: i32,
    #[serde(default, rename = "gameEndedInEarlySurrender")]
    pub(crate) game_ended_in_early_surrender: bool,
    #[serde(default, rename = "teamEarlySurrendered")]
    pub(crate) team_early_surrendered: bool,
    #[serde(default, rename = "subteamPlacement")]
    pub(crate) subteam_placement: i32,
    #[serde(default, rename = "highestAchievedSeasonTier")]
    pub(crate) highest_achieved_season_tier: String,
    #[serde(default)]
    pub(crate) augments: Vec<i32>,
    #[serde(default, rename = "playerAugment1")]
    pub(crate) player_augment1: i32,
    #[serde(default, rename = "playerAugment2")]
    pub(crate) player_augment2: i32,
    #[serde(default, rename = "playerAugment3")]
    pub(crate) player_augment3: i32,
    #[serde(default, rename = "playerAugment4")]
    pub(crate) player_augment4: i32,
    #[serde(default, rename = "playerAugment5")]
    pub(crate) player_augment5: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LcuParticipant {
    #[serde(rename = "participantId")]
    pub(crate) participant_id: i32,
    #[serde(default, rename = "teamID")]
    pub(crate) team_id: i32,
    #[serde(default, rename = "teamId")]
    pub(crate) team_id_alt: i32,
    #[serde(default, rename = "championId")]
    pub(crate) champion_id: i32,
    #[serde(default, rename = "spell1Id")]
    pub(crate) spell1_id: i32,
    #[serde(default, rename = "spell2Id")]
    pub(crate) spell2_id: i32,
    #[serde(default)]
    pub(crate) stats: LcuStats,
}

impl LcuParticipant {
    pub(crate) fn team(&self) -> i32 {
        if self.team_id != 0 {
            self.team_id
        } else {
            self.team_id_alt
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LcuGame {
    #[serde(default, rename = "gameId")]
    pub(crate) game_id: i64,
    #[serde(default, rename = "gameCreation")]
    pub(crate) game_creation: i64,
    #[serde(default, rename = "gameDuration")]
    pub(crate) game_duration: i32,
    #[serde(default, rename = "queueId")]
    pub(crate) queue_id: i32,
    #[serde(default, rename = "mapId")]
    #[allow(dead_code)]
    pub(crate) map_id: i32,
    #[serde(default, rename = "gameEndedInEarlySurrender")]
    pub(crate) game_ended_in_early_surrender: bool,
    #[serde(default)]
    pub(crate) participants: Vec<LcuParticipant>,
    #[serde(default, rename = "participantIdentities")]
    pub(crate) participant_identities: Vec<LcuIdentity>,
}

/* ─── 解析入口 ─────────────────────────────────────────────────── */

/// 解析战绩列表 → (摘要, 权威总场数)。
/// 总场数来自 API `gameCount`；缺失或非正数时返回 None（未知），不得用本页条数冒充。
pub fn parse_match_summaries(
    data: &[u8],
    self_puuid: &str,
) -> Result<(Vec<MatchSummary>, Option<i32>), crate::error::AppError> {
    let v: Value = serde_json::from_slice(data)
        .map_err(|e| crate::error::AppError::Parse(format!("解析战绩列表失败: {e}")))?;
    let games = v
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();
    let game_count = v
        .get("games")
        .and_then(|g| g.get("gameCount"))
        .and_then(|g| g.as_i64());

    let mut summaries = Vec::with_capacity(games.len());
    for raw in &games {
        let Ok(g) = serde_json::from_value::<LcuGame>(raw.clone()) else {
            continue;
        };
        if g.participants.is_empty() {
            continue;
        }
        let p = pick_participant(&g, self_puuid);
        summaries.push(build_summary(&g, &p));
    }

    let count = game_count.filter(|&c| c > 0).map(|c| c as i32);
    Ok((summaries, count))
}

pub fn parse_match_detail(
    data: &[u8],
    self_puuid: &str,
) -> Result<MatchDetail, crate::error::AppError> {
    let g: LcuGame = serde_json::from_slice(data)
        .map_err(|e| crate::error::AppError::Parse(format!("解析对局明细失败: {e}")))?;
    if g.participants.is_empty() {
        return Err(crate::error::AppError::Parse("对局明细无参与者数据".into()));
    }

    let qi = queue_info_for(g.queue_id);
    let id_by_id: HashMap<i32, &LcuIdentity> = g
        .participant_identities
        .iter()
        .map(|ident| (ident.participant_id, ident))
        .collect();
    let remake = is_remake(&g);

    let (mut rows, self_key) = build_rows(&g, &id_by_id, remake, self_puuid, &qi);
    normalize_damage(&qi, &mut rows);
    rank_rows(&mut rows);
    let (order, mut buckets) = group_teams(rows, &qi, self_key);
    let teams = summarize_teams(&order, &mut buckets);

    Ok(MatchDetail {
        game_id: g.game_id,
        queue_id: g.queue_id,
        queue_name: qi.name,
        arena: qi.arena,
        game_creation: g.game_creation,
        time: format_time(g.game_creation),
        game_duration: g.game_duration,
        duration: format_duration(g.game_duration),
        duration_min: format!("{}分", g.game_duration / 60),
        remake,
        self_puuid: self_puuid.to_string(),
        teams,
    })
}

/// 由参赛者构建明细行并标记本人行；返回 (行集, 本人所在分组键)。
fn build_rows(
    g: &LcuGame,
    id_by_id: &HashMap<i32, &LcuIdentity>,
    remake: bool,
    self_puuid: &str,
    qi: &super::QueueInfo,
) -> (Vec<PlayerRow>, String) {
    let mut rows: Vec<PlayerRow> = Vec::with_capacity(g.participants.len());
    let mut self_key = String::new();

    for p in &g.participants {
        let s = &p.stats;
        let mut row = PlayerRow {
            participant_id: p.participant_id,
            team_id: p.team(),
            placement: s.subteam_placement,
            puuid: None,
            summoner_id: None,
            name: String::new(),
            profile_icon_id: 0,
            champion_id: p.champion_id,
            champ_level: s.champ_level,
            spell1_id: p.spell1_id,
            spell2_id: p.spell2_id,
            rune_id: s.perk0,
            kills: s.kills,
            deaths: s.deaths,
            assists: s.assists,
            kda: kda_string(s.kills, s.deaths, s.assists),
            items: vec![
                s.item0, s.item1, s.item2, s.item3, s.item4, s.item5, s.item6,
            ],
            gold: s.gold_earned,
            total_damage: s.total_damage_dealt_to_champions,
            win: s.win,
            remake,
            augment_ids: augment_ids(s),
            tier_short: tier_cn(&s.highest_achieved_season_tier),
            dmg_ratio: 0.0,
            match_rating: rating(
                s.kills,
                s.deaths,
                s.assists,
                s.total_damage_dealt_to_champions,
                s.gold_earned,
                s.win,
            ),
            rating_rank: 0,
            kill_participation: 0,
            is_self: false,
        };
        if let Some(ident) = id_by_id.get(&p.participant_id) {
            row.puuid = opt_id(flex_value(&ident.player.puuid));
            row.summoner_id = opt_id(flex_value(&ident.player.summoner_id));
            row.profile_icon_id = ident.player.profile_icon;
            row.name = identity_name(ident, p.participant_id);
        } else {
            row.name = format!("玩家 {}", p.participant_id);
        }
        if !self_puuid.is_empty()
            && row
                .puuid
                .as_deref()
                .is_some_and(|p| p.eq_ignore_ascii_case(self_puuid))
        {
            row.is_self = true;
            self_key = group_key(qi, row.team_id, row.placement);
        }
        rows.push(row);
    }
    (rows, self_key)
}

/// 行级派生：队伍内伤害占比（dmg_ratio）与参团率（kill_pct）。
fn normalize_damage(qi: &super::QueueInfo, rows: &mut [PlayerRow]) {
    #[derive(Default, Clone)]
    struct GroupSum {
        dmg: i64,
        kills: i64,
        n: i64,
    }
    let mut sums: HashMap<String, GroupSum> = HashMap::new();
    for r in rows.iter() {
        let k = group_key(qi, r.team_id, r.placement);
        let st = sums.entry(k).or_default();
        st.dmg += r.total_damage as i64;
        st.kills += r.kills as i64;
        st.n += 1;
    }
    for r in rows.iter_mut() {
        let k = group_key(qi, r.team_id, r.placement);
        let st = sums.get(&k).cloned().unwrap_or_default();
        let avg = st.dmg as f64 / (st.n as f64).max(1.0);
        if avg > 0.0 {
            r.dmg_ratio = ((r.total_damage as f64 / avg) * 10.0).round() / 10.0;
        }
        if st.kills > 0 {
            r.kill_participation =
                (((r.kills + r.assists) as f64 / st.kills as f64) * 100.0).round() as i32;
        }
    }
}

/// 全局评分排名（跨全部行；评分并列按总伤害破平）。
fn rank_rows(rows: &mut [PlayerRow]) {
    let mut rank_order: Vec<usize> = (0..rows.len()).collect();
    rank_order.sort_by(|&a, &b| {
        let ra = rows[a].match_rating;
        let rb = rows[b].match_rating;
        if (ra - rb).abs() > f64::EPSILON {
            rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            rows[b].total_damage.cmp(&rows[a].total_damage)
        }
    });
    for (rank, &idx) in rank_order.iter().enumerate() {
        rows[idx].rating_rank = (rank + 1) as i32;
    }
}

/// 明细分组桶（队伍 / 竞技场子队）。
#[derive(Default)]
struct Bucket {
    team_id: i32,
    placement: i32,
    win: bool,
    rows: Vec<PlayerRow>,
}

/// 按队伍/子队分桶并定序：本人队最前；竞技场按 placement、其余按 teamID 升序。
/// 竞技场 placement=1 的分组强制判胜；self_key 为空时以首组兜底。
fn group_teams(
    rows: Vec<PlayerRow>,
    qi: &super::QueueInfo,
    mut self_key: String,
) -> (Vec<String>, HashMap<String, Bucket>) {
    let mut order: Vec<String> = Vec::new();
    let mut buckets: HashMap<String, Bucket> = HashMap::new();
    for r in rows {
        let k = group_key(qi, r.team_id, r.placement);
        let b = buckets.entry(k.clone()).or_insert_with(|| {
            order.push(k.clone());
            Bucket {
                team_id: r.team_id,
                placement: r.placement,
                win: false,
                rows: Vec::new(),
            }
        });
        b.win = b.win || r.win;
        b.rows.push(r);
    }
    if self_key.is_empty() {
        if let Some(first) = order.first() {
            if let Some(b) = buckets.get(first) {
                self_key = group_key(qi, b.team_id, b.placement);
            }
        }
    }
    // 竞技场按名次 1 判定胜利
    if qi.arena {
        for b in buckets.values_mut() {
            if b.placement == 1 {
                b.win = true;
            }
        }
    }

    // 排序：本人队伍最前，其余按 placement 与 teamID 升序
    order.sort_by(|a, b| {
        if a == &self_key {
            return std::cmp::Ordering::Less;
        }
        if b == &self_key {
            return std::cmp::Ordering::Greater;
        }
        let ba = &buckets[a];
        let bb = &buckets[b];
        if ba.placement != bb.placement {
            ba.placement.cmp(&bb.placement)
        } else {
            ba.team_id.cmp(&bb.team_id)
        }
    });
    (order, buckets)
}

/// 汇总各分组为队伍视图：行按评分降序；击杀/死亡/助攻/经济/伤害求和。
fn summarize_teams(order: &[String], buckets: &mut HashMap<String, Bucket>) -> Vec<TeamSummary> {
    let mut teams = Vec::with_capacity(order.len());
    for k in order {
        let Some(b) = buckets.get_mut(k) else {
            continue;
        };
        b.rows.sort_by(|x, y| {
            y.match_rating
                .partial_cmp(&x.match_rating)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut ts = TeamSummary {
            team_id: b.team_id,
            placement: b.placement,
            win: b.win,
            kills: 0,
            deaths: 0,
            assists: 0,
            gold: 0,
            damage: 0,
            players: b.rows.clone(),
        };
        for r in &b.rows {
            ts.kills += r.kills;
            ts.deaths += r.deaths;
            ts.assists += r.assists;
            ts.gold += r.gold;
            ts.damage += r.total_damage;
        }
        teams.push(ts);
    }
    teams
}

/* ─── 纯函数工具 ───────────────────────────────────────────────── */

/// 评分权重：KDA 部分 / 伤害万分位 / 经济万分位 / 胜利加成
pub const W_KDA: f64 = 1.1;
pub const W_DAMAGE: f64 = 1.2;
pub const W_GOLD: f64 = 0.6;
pub const WIN_BONUS: f64 = 1.5;

pub fn rating(kills: i32, deaths: i32, assists: i32, damage: i32, gold: i32, win: bool) -> f64 {
    let mut kda_part = (kills + assists) as f64 / (deaths as f64).max(1.0);
    if kda_part > 10.0 {
        kda_part = 10.0;
    }
    let mut r =
        kda_part * W_KDA + damage as f64 / 10000.0 * W_DAMAGE + gold as f64 / 10000.0 * W_GOLD;
    if win {
        r += 1.5;
    }
    (r * 10.0).round() / 10.0
}

pub fn kda_string(kills: i32, deaths: i32, assists: i32) -> String {
    if deaths == 0 {
        return "Perfect".into();
    }
    format!("{:.2}", (kills + assists) as f64 / deaths as f64)
}

pub fn tier_cn(tier: &str) -> String {
    match tier.trim().to_uppercase().as_str() {
        "" | "UNRANKED" | "NONE" => String::new(),
        "IRON" => "黑铁".into(),
        "BRONZE" => "青铜".into(),
        "SILVER" => "白银".into(),
        "GOLD" => "黄金".into(),
        "PLATINUM" => "白金".into(),
        "EMERALD" => "翡翠".into(),
        "DIAMOND" => "钻石".into(),
        "MASTER" => "大师".into(),
        "GRANDMASTER" => "宗师".into(),
        "CHALLENGER" => "王者".into(),
        other => other.to_string(),
    }
}

pub fn format_time(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%Y/%-m/%-d %H:%M:%S").to_string())
        .unwrap_or_default()
}

pub fn format_short_time(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

pub fn format_duration(secs: i32) -> String {
    let secs = secs.max(0);
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/* ─── 内部辅助 ─────────────────────────────────────────────────── */

pub(crate) fn pick_participant(g: &LcuGame, self_puuid: &str) -> LcuParticipant {
    if !self_puuid.is_empty() && !g.participant_identities.is_empty() {
        let mut id_by_id: HashMap<i32, &LcuIdentity> = HashMap::new();
        for ident in &g.participant_identities {
            id_by_id.insert(ident.participant_id, ident);
        }
        for p in &g.participants {
            if let Some(ident) = id_by_id.get(&p.participant_id) {
                if flex_value(&ident.player.puuid).eq_ignore_ascii_case(self_puuid) {
                    return p.clone();
                }
            }
        }
    }
    g.participants[0].clone()
}

/// 重赛（无效对局）判定——**唯一口径**，列表 / 详情 / SGP 适配三处共用。
/// 局级 `gameEndedInEarlySurrender` 为真，或任一参赛者带投降类标志
/// （`gameEndedInEarlySurrender` / `teamEarlySurrendered`）即判重赛。
pub(crate) fn is_remake(g: &LcuGame) -> bool {
    g.game_ended_in_early_surrender
        || g.participants
            .iter()
            .any(|p| p.stats.game_ended_in_early_surrender || p.stats.team_early_surrendered)
}

pub(crate) fn build_summary(g: &LcuGame, p: &LcuParticipant) -> MatchSummary {
    let s = &p.stats;
    let qi = queue_info_for(g.queue_id);
    MatchSummary {
        game_id: g.game_id,
        queue_id: g.queue_id,
        queue_name: qi.name.clone(),
        queue_short: qi.short.clone(),
        arena: qi.arena,
        game_creation: g.game_creation,
        game_duration: g.game_duration,
        short_time: format_short_time(g.game_creation),
        duration: format_duration(g.game_duration),
        champion_id: p.champion_id,
        champ_level: s.champ_level,
        spell1_id: p.spell1_id,
        spell2_id: p.spell2_id,
        rune_id: s.perk0,
        kills: s.kills,
        deaths: s.deaths,
        assists: s.assists,
        kda: kda_string(s.kills, s.deaths, s.assists),
        win: s.win,
        remake: is_remake(g),
        placement: s.subteam_placement,
        items: vec![
            s.item0, s.item1, s.item2, s.item3, s.item4, s.item5, s.item6,
        ],
        gold: s.gold_earned,
        total_damage: s.total_damage_dealt_to_champions,
        augment_ids: augment_ids(s),
    }
}

fn augment_ids(s: &LcuStats) -> Vec<i32> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(5);
    let push = |id: i32, seen: &mut std::collections::HashSet<i32>, out: &mut Vec<i32>| {
        if id != 0 && seen.insert(id) && out.len() < 5 {
            out.push(id);
        }
    };
    for &id in &s.augments {
        push(id, &mut seen, &mut out);
    }
    for id in [
        s.player_augment1,
        s.player_augment2,
        s.player_augment3,
        s.player_augment4,
        s.player_augment5,
    ] {
        push(id, &mut seen, &mut out);
    }
    out
}

fn identity_name(ident: &LcuIdentity, participant_id: i32) -> String {
    let p = &ident.player;
    let mut name = p.game_name.clone();
    if name.is_empty() {
        name = p.summoner_name.clone();
    }
    if name.is_empty() {
        name = p.display_name.clone();
    }
    if name.is_empty() {
        return format!("玩家 {participant_id}");
    }
    if !p.tag_line.is_empty() {
        format!("{name}#{}", p.tag_line)
    } else {
        name
    }
}

fn group_key(qi: &super::QueueInfo, team_id: i32, placement: i32) -> String {
    if qi.arena && placement > 0 {
        format!("p{placement}")
    } else {
        format!("t{team_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HISTORY_FIXTURE: &str = r#"{"games":{"games":[
      {"gameId":7000000001,"gameCreation":1705329000000,"gameDuration":924,"queueId":2400,
       "participantIdentities":[
         {"participantId":1,"player":{"puuid":"OTHER","gameName":"路人","tagLine":"0001","profileIcon":7}},
         {"participantId":2,"player":{"puuid":"PSELF","gameName":"歪比巴卜小宝贝","tagLine":"60021","summonerId":"SID-3","profileIcon":42}}],
       "participants":[
         {"participantId":1,"teamID":100,"championId":22,"spell1Id":4,"spell2Id":12,
          "stats":{"win":true,"kills":9,"deaths":1,"assists":20,"champLevel":16,"perk0":8112,
                   "totalMinionsKilled":10,"neutralMinionsKilled":3,"goldEarned":13000,
                   "totalDamageDealtToChampions":25000}},
         {"participantId":2,"teamId":200,"championId":53,"spell1Id":4,"spell2Id":6,
          "stats":{"win":false,"kills":4,"deaths":11,"assists":20,"champLevel":16,
                   "item0":3157,"item1":3020,"item6":3340,"perk0":8351,
                   "totalMinionsKilled":5,"neutralMinionsKilled":2,"goldEarned":11767,
                   "totalDamageDealtToChampions":17324,"totalHeal":900,
                   "augments":[7010,0,7010],"playerAugment1":7018,"playerAugment2":0,
                   "highestAchievedSeasonTier":"GOLD"}}]},
      {"gameId":7000000002,"gameCreation":1705329000000,"gameDuration":600,"queueId":1700,
       "participants":[
         {"participantId":1,"championId":1,"spell1Id":4,"spell2Id":6,
          "stats":{"win":false,"kills":0,"deaths":0,"assists":0,"champLevel":9,
                   "gameEndedInEarlySurrender":true,"subteamPlacement":5}}]}],
      "gameCount":45}}"#;

    const DETAIL_FIXTURE: &str = r#"{"gameId":8000000001,"gameCreation":1705329000000,"gameDuration":924,
      "queueId":420,
      "participantIdentities":[
        {"participantId":1,"player":{"puuid":"PWIN1","gameName":"A","tagLine":"111","summonerId":"S1","profileIcon":11}},
        {"participantId":2,"player":{"puuid":"PWIN2","gameName":"B","tagLine":"222","summonerId":"S2","profileIcon":12}},
        {"participantId":3,"player":{"puuid":"PSELF","gameName":"歪比","tagLine":"60021","summonerId":3333,"profileIcon":42}},
        {"participantId":4,"player":{"puuid":"PLOSE2","gameName":"D","tagLine":"444","summonerId":"S4","profileIcon":14}}],
      "participants":[
        {"participantId":1,"teamID":100,"championId":22,"spell1Id":4,"spell2Id":12,
         "stats":{"win":true,"kills":10,"deaths":2,"assists":8,"champLevel":16,"perk0":8112,
                  "item0":6672,"totalMinionsKilled":20,"neutralMinionsKilled":4,"goldEarned":14000,
                  "totalDamageDealtToChampions":25000,"highestAchievedSeasonTier":"GOLD"}},
        {"participantId":2,"teamID":100,"championId":18,"spell1Id":4,"spell2Id":7,
         "stats":{"win":true,"kills":5,"deaths":5,"assists":5,"champLevel":15,"perk0":8005,
                  "totalMinionsKilled":15,"goldEarned":12000,"totalDamageDealtToChampions":25000}},
        {"participantId":3,"teamId":200,"championId":53,"spell1Id":4,"spell2Id":6,
         "stats":{"win":false,"kills":4,"deaths":11,"assists":20,"champLevel":16,"perk0":8351,
                  "item0":3157,"item1":3020,"item6":3340,"totalMinionsKilled":5,
                  "neutralMinionsKilled":2,"goldEarned":11767,"totalDamageDealtToChampions":30000}},
        {"participantId":4,"teamId":200,"championId":99,"spell1Id":4,"spell2Id":14,
         "stats":{"win":false,"kills":6,"deaths":9,"assists":15,"champLevel":15,"perk0":8229,
                  "goldEarned":10000,"totalDamageDealtToChampions":10000}}]}"#;

    const DETAIL_ARENA_FIXTURE: &str = r#"{"gameId":9000000001,"gameCreation":1705329000000,"gameDuration":780,
      "queueId":1700,
      "participantIdentities":[
        {"participantId":1,"player":{"puuid":"PSELF","gameName":"歪比","tagLine":"60021"}},
        {"participantId":2,"player":{"puuid":"MATE","gameName":"队友","tagLine":"0002"}},
        {"participantId":3,"player":{"puuid":"WIN1","gameName":"甲","tagLine":"0003"}},
        {"participantId":4,"player":{"puuid":"WIN2","gameName":"乙","tagLine":"0004"}}],
      "participants":[
        {"participantId":1,"teamID":100,"championId":1,"stats":{"win":false,"kills":2,"deaths":6,"assists":3,"subteamPlacement":2,"goldEarned":9000,"totalDamageDealtToChampions":12000}},
        {"participantId":2,"teamID":100,"championId":2,"stats":{"win":false,"kills":3,"deaths":5,"assists":4,"subteamPlacement":2,"goldEarned":9500,"totalDamageDealtToChampions":14000}},
        {"participantId":3,"teamID":100,"championId":3,"stats":{"win":true,"kills":8,"deaths":2,"assists":6,"subteamPlacement":1,"goldEarned":13000,"totalDamageDealtToChampions":22000}},
        {"participantId":4,"teamID":100,"championId":4,"stats":{"win":true,"kills":7,"deaths":3,"assists":7,"subteamPlacement":1,"goldEarned":12500,"totalDamageDealtToChampions":20000}}]}"#;

    #[test]
    fn rating_zero_death_capped() {
        let got = rating(30, 0, 30, 50000, 15000, true);
        let base: f64 = 10.0 * 1.1 + 50000.0 / 10000.0 * 1.2 + 15000.0 / 10000.0 * 0.6 + 1.5;
        let want: f64 = (base * 10.0).round() / 10.0;
        assert!((got - want).abs() < 1e-9, "got={got} want={want}");
    }

    #[test]
    fn rating_win_bonus_exactly_1_5() {
        let win = rating(5, 5, 5, 20000, 10000, true);
        let lose = rating(5, 5, 5, 20000, 10000, false);
        assert!(((win - lose) - 1.5).abs() < 1e-9);
    }

    #[test]
    fn kda_string_cases() {
        assert_eq!(kda_string(10, 2, 8), "9.00");
        assert_eq!(kda_string(4, 0, 12), "Perfect");
    }

    #[test]
    fn tier_cn_cases() {
        assert_eq!(tier_cn("GOLD"), "黄金");
        assert_eq!(tier_cn("gold"), "黄金");
        assert_eq!(tier_cn("IRON"), "黑铁");
        assert_eq!(tier_cn("CHALLENGER"), "王者");
        assert_eq!(tier_cn(""), "");
        assert_eq!(tier_cn("UNRANKED"), "");
        assert_eq!(tier_cn("WEIRD"), "WEIRD");
    }

    #[test]
    fn formatters() {
        assert_eq!(format_duration(1530), "25:30");
        assert_eq!(format_duration(3725), "62:05");
        assert_eq!(format_duration(-1), "00:00");
        assert_eq!(format_time(0), "");
        assert!(!format_short_time(1705329000000).is_empty());
    }

    #[test]
    fn parse_match_summaries_self_by_puuid_and_fallback() {
        let (sums, count) = parse_match_summaries(HISTORY_FIXTURE.as_bytes(), "PSELF").unwrap();
        assert_eq!(count, Some(45), "权威总场数来自 gameCount");
        assert_eq!(sums.len(), 2);

        let s = &sums[0];
        assert_eq!(s.champion_id, 53, "self participant not matched by puuid");
        assert_eq!(s.queue_short, "海斗");
        assert_eq!(s.queue_name, "海克斯大乱斗");
        assert_eq!(s.kda, "2.18");
        assert!(!s.win);
        assert!(!s.remake);
        assert_eq!(s.duration, "15:24");
        assert_eq!(s.items[0], 3157);
        assert_eq!(s.items[6], 3340);
        assert_eq!(s.gold, 11767);
        assert_eq!(s.augment_ids, vec![7010, 7018]);

        let r = &sums[1];
        assert!(r.remake);
        assert_eq!(r.placement, 5);
        assert_eq!(r.kda, "Perfect");
        assert!(r.arena);

        let (firsts, _) = parse_match_summaries(HISTORY_FIXTURE.as_bytes(), "").unwrap();
        assert_eq!(firsts[0].champion_id, 22, "fallback should pick first");
    }

    #[test]
    fn parse_match_detail_classic_teams() {
        let d = parse_match_detail(DETAIL_FIXTURE.as_bytes(), "PSELF").unwrap();
        assert_eq!(d.game_id, 8000000001);
        assert_eq!(d.queue_name, "排位单双排");
        assert!(!d.arena);
        assert_eq!(d.duration, "15:24");
        assert_eq!(d.duration_min, "15分");
        assert!(!d.remake);
        assert_eq!(d.teams.len(), 2);

        let self_team = &d.teams[0];
        assert_eq!(self_team.team_id, 200);
        assert!(!self_team.win);
        assert_eq!(self_team.kills, 10);
        assert_eq!(self_team.damage, 40000);
        assert_eq!(self_team.gold, 21767);
        assert_eq!(self_team.players.len(), 2);

        let me = &self_team.players[0];
        assert!(me.is_self);
        assert_eq!(me.name, "歪比#60021");
        assert_eq!(me.summoner_id.as_deref(), Some("3333"));
        assert!(
            self_team.players[0].match_rating > self_team.players[1].match_rating,
            "team players not sorted by rating desc"
        );
        assert_eq!(me.dmg_ratio, 1.5);
        assert_eq!(self_team.players[1].dmg_ratio, 0.5);
        assert_eq!(self_team.deaths, 20);
        assert_eq!(self_team.assists, 35);
        assert_eq!(me.kill_participation, 240);
        assert_eq!(self_team.players[1].kill_participation, 210);

        let mut rank_set = std::collections::HashSet::new();
        for team in &d.teams {
            for p in &team.players {
                rank_set.insert(p.rating_rank);
            }
        }
        assert_eq!(rank_set.len(), 4);
        assert!(rank_set.contains(&1));
        assert!(rank_set.contains(&4));

        let mvp = &d.teams[1].players[0];
        assert_eq!(mvp.rating_rank, 1);
        assert_eq!(mvp.tier_short, "黄金");
        assert_eq!(d.teams[1].team_id, 100);
        assert!(d.teams[1].win);
    }

    #[test]
    fn parse_match_detail_arena_subteams() {
        let d = parse_match_detail(DETAIL_ARENA_FIXTURE.as_bytes(), "PSELF").unwrap();
        assert!(d.arena);
        assert_eq!(d.queue_name, "斗魂竞技场");
        assert_eq!(d.teams.len(), 2);
        assert_eq!(d.teams[0].placement, 2);
        assert!(!d.teams[0].win);
        assert_eq!(d.teams[1].placement, 1);
        assert!(d.teams[1].win);

        let mut self_found = false;
        for p in &d.teams[0].players {
            if p.is_self {
                self_found = true;
                assert_eq!(p.dmg_ratio, 0.9, "self dmgRatio");
                assert_eq!(p.kill_participation, 100, "self killParticipation");
            } else {
                assert_eq!(p.dmg_ratio, 1.1, "mate dmgRatio");
                assert_eq!(p.kill_participation, 140, "mate killParticipation");
            }
        }
        assert!(self_found, "self row missing in own subteam");
    }

    #[test]
    fn parse_match_detail_invalid_input() {
        assert!(parse_match_detail("{}".as_bytes(), "").is_err());
        assert!(parse_match_detail(b"not-json", "").is_err());
    }

    /// C2 回归：仅参赛者级 `teamEarlySurrendered=true` 时，列表与详情的 remake 必须同判。
    #[test]
    fn remake_consistent_between_summary_and_detail() {
        // 局级 gameEndedInEarlySurrender 不出现（默认 false），仅参赛者级 teamEarlySurrendered=true
        let game = r#"{"gameId":42,"gameCreation":1705329000000,"gameDuration":300,"queueId":420,
          "participantIdentities":[
            {"participantId":1,"player":{"puuid":"PSELF","gameName":"甲","tagLine":"111"}},
            {"participantId":2,"player":{"puuid":"OTHER","gameName":"乙","tagLine":"222"}}],
          "participants":[
            {"participantId":1,"teamID":100,"championId":1,
             "stats":{"win":false,"kills":0,"deaths":0,"assists":0,
                      "gameEndedInEarlySurrender":false,"teamEarlySurrendered":true}},
            {"participantId":2,"teamID":200,"championId":2,
             "stats":{"win":true,"kills":1,"deaths":0,"assists":0,
                      "gameEndedInEarlySurrender":false,"teamEarlySurrendered":false}}]}"#;

        let list = format!(r#"{{"games":{{"games":[{game}],"gameCount":1}}}}"#);
        let (sums, _) = parse_match_summaries(list.as_bytes(), "PSELF").unwrap();
        let d = parse_match_detail(game.as_bytes(), "PSELF").unwrap();

        assert!(d.remake, "detail：teamEarlySurrendered 应判重赛");
        assert!(
            sums[0].remake,
            "summary：teamEarlySurrendered 应判重赛（此前漏判）"
        );
        assert_eq!(sums[0].remake, d.remake, "列表与详情 remake 口径必须一致");
    }

    /// C2 回归：局级 gameEndedInEarlySurrender=true 时两口径同样一致。
    #[test]
    fn remake_game_level_flag_consistent() {
        let game = r#"{"gameId":43,"gameCreation":1705329000000,"gameDuration":300,"queueId":420,
          "gameEndedInEarlySurrender":true,
          "participantIdentities":[
            {"participantId":1,"player":{"puuid":"PSELF","gameName":"甲","tagLine":"111"}}],
          "participants":[
            {"participantId":1,"teamID":100,"championId":1,
             "stats":{"win":false,"kills":0,"deaths":0,"assists":0}}]}"#;

        let list = format!(r#"{{"games":{{"games":[{game}],"gameCount":1}}}}"#);
        let (sums, _) = parse_match_summaries(list.as_bytes(), "PSELF").unwrap();
        let d = parse_match_detail(game.as_bytes(), "PSELF").unwrap();

        assert!(sums[0].remake && d.remake);
        assert_eq!(sums[0].remake, d.remake);
    }
}
