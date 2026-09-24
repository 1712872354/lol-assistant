//! 纯函数工具：名字/标签拆分、段位选取、队伍汇总、花名册补齐等。

use std::collections::{HashMap, HashSet};

use crate::lcu::ConnStatus;
use crate::service::history::RankedInfo;

use super::model::{PlayerSlot, RecentMatch, TeamView};
use super::protocol::PlayerRef;

/* ── 纯函数 ── */

pub(super) fn split_name(name: &str) -> (String, String) {
    let name = name.trim();
    if let Some(i) = name.find('#') {
        (name[..i].to_string(), name[i + 1..].to_string())
    } else {
        (name.to_string(), String::new())
    }
}

pub(super) fn tag_equal(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return true;
    }
    a.eq_ignore_ascii_case(b)
}

pub(super) fn is_self_match(puuid: &str, game_name: &str, tag: &str, self_st: &ConnStatus) -> bool {
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

pub(super) fn pick_rank(rk: &RankedInfo) -> (String, String) {
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

pub(super) fn career_stats(recent: &[RecentMatch]) -> (f64, i32, f64, f64) {
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

pub(super) fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

pub(super) fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

pub(super) fn sum_team(
    key: &str,
    label: &str,
    side_text: &str,
    badge: &str,
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
        wr_sum += s.win_rate.unwrap_or(0.0);
        r_sum += s.player_score.unwrap_or(0.0);
    }
    let mut tv = TeamView {
        key: key.into(),
        label: label.into(),
        side_text: side_text.into(),
        badge: badge.into(),
        player_count: n,
        win_rate: 0.0,
        team_score: 0,
        slots,
    };
    if n > 0 {
        tv.win_rate = round1(wr_sum / n as f64);
        tv.team_score = (r_sum / n as f64 * 10.0).round() as i32;
    }
    tv
}

/// 近况队列口径解析：None=跟随当前对局；Some(空)=全部；Some(ids)=限定 id。
pub(super) fn resolve_queue_filter(
    queue_filter: Option<&[i32]>,
    current_queue_id: i32,
) -> Vec<i32> {
    match queue_filter {
        Some(ids) => ids.to_vec(),
        None => {
            if current_queue_id > 0 {
                vec![current_queue_id]
            } else {
                Vec::new()
            }
        }
    }
}

pub(super) fn queue_matches(queue_id: i32, filter: &[i32]) -> bool {
    if filter.is_empty() {
        return true;
    }
    filter.contains(&queue_id)
}

pub(super) fn enrich_from_roster(refs: &mut [PlayerRef], teams: &[Vec<PlayerRef>]) {
    let mut idx: HashMap<String, PlayerRef> = HashMap::new();
    for list in teams {
        for r in list {
            if let Some(pu) = &r.puuid {
                idx.insert(format!("p:{pu}"), r.clone());
            }
            if let Some(sid) = &r.summoner_id {
                idx.insert(format!("s:{sid}"), r.clone());
            }
        }
    }
    for r in refs.iter_mut() {
        let mut src = None;
        if let Some(pu) = &r.puuid {
            src = idx.get(&format!("p:{pu}")).cloned();
        }
        if src.is_none() {
            if let Some(sid) = &r.summoner_id {
                src = idx.get(&format!("s:{sid}")).cloned();
            }
        }
        let Some(src) = src else { continue };
        if r.game_name.is_none() {
            r.game_name = src.game_name.clone();
            r.tag_line = src.tag_line.clone();
        }
        if r.profile_icon_id == 0 {
            r.profile_icon_id = src.profile_icon_id;
        }
        if r.summoner_id.is_none() {
            r.summoner_id = src.summoner_id.clone();
        }
        if r.puuid.is_none() {
            r.puuid = src.puuid.clone();
        }
        if r.champion_id == 0 {
            r.champion_id = src.champion_id;
        }
    }
}

pub(super) fn backfill_from_roster(
    mut refs: Vec<PlayerRef>,
    src: Vec<PlayerRef>,
) -> Vec<PlayerRef> {
    enrich_from_roster(&mut refs, std::slice::from_ref(&src));
    if src.is_empty() {
        return refs;
    }
    let mut have_p = HashSet::new();
    let mut have_s = HashSet::new();
    for r in &refs {
        if let Some(pu) = &r.puuid {
            have_p.insert(pu.clone());
        }
        if let Some(sid) = &r.summoner_id {
            have_s.insert(sid.clone());
        }
    }
    for r in src {
        if let Some(pu) = &r.puuid {
            if have_p.contains(pu) {
                continue;
            }
        }
        if let Some(sid) = &r.summoner_id {
            if have_s.contains(sid) {
                continue;
            }
        }
        if let Some(pu) = &r.puuid {
            have_p.insert(pu.clone());
        }
        if let Some(sid) = &r.summoner_id {
            have_s.insert(sid.clone());
        }
        refs.push(r);
    }
    refs
}
