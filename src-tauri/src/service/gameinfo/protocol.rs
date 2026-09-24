//! LCU 协议层中间态：Gameflow / Lobby / ChampSelect / SummonerRaw 等原始结构解析。

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::lcu::ConnStatus;
use crate::parser::{lookup_queue, queue_info_for};

use super::model::RecentMatch;
use super::util::{is_self_match, split_name};
use crate::util::{opt_id, opt_name};

/* ── 内部中间态 ── */

#[derive(Debug, Clone, Default)]
pub(super) struct PlayerRef {
    pub(super) puuid: Option<String>,
    pub(super) summoner_id: Option<String>,
    pub(super) game_name: Option<String>,
    pub(super) tag_line: Option<String>,
    pub(super) profile_icon_id: i32,
    pub(super) champion_id: i32,
    pub(super) is_self: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Career {
    pub(super) recent: Vec<RecentMatch>,
    pub(super) hidden: bool,
}

#[derive(Deserialize, Default)]
pub(super) struct ChampEntry {
    #[serde(default)]
    pub(super) id: i32,
    #[serde(default)]
    pub(super) alias: String,
    #[serde(default)]
    pub(super) name: String,
}

#[derive(Deserialize, Default, Clone)]
pub(super) struct GsParticipant {
    #[serde(default)]
    pub(super) puuid: String,
    #[serde(
        default,
        rename = "summonerId",
        deserialize_with = "crate::util::de_flex_str"
    )]
    pub(super) summoner_id: String,
    #[serde(default, rename = "gameName")]
    pub(super) game_name: String,
    #[serde(default, rename = "tagLine")]
    pub(super) tag_line: String,
    #[serde(default, rename = "summonerName")]
    pub(super) summoner_name: String,
    #[serde(default, rename = "profileIconId")]
    pub(super) profile_icon_id: i32,
    #[serde(default, rename = "championId")]
    pub(super) champion_id: i32,
}

#[derive(Deserialize, Default, Clone)]
pub(super) struct GsPick {
    #[serde(default)]
    pub(super) puuid: String,
    #[serde(default, rename = "championId")]
    pub(super) champion_id: i32,
}

#[derive(Deserialize, Default)]
pub(super) struct QueueMeta {
    #[serde(default)]
    pub(super) id: i32,
    #[serde(default)]
    pub(super) name: String,
    #[serde(default, rename = "numPlayersPerTeam")]
    pub(super) num_players_per_team: i32,
}

#[derive(Deserialize, Default)]
pub(super) struct GameData {
    #[serde(default)]
    pub(super) queue: QueueMeta,
    #[serde(default, rename = "queueId")]
    pub(super) queue_id: i32,
    #[serde(default, rename = "teamOne")]
    pub(super) team_one: Vec<GsParticipant>,
    #[serde(default, rename = "teamTwo")]
    pub(super) team_two: Vec<GsParticipant>,
    #[serde(default, rename = "playerChampionSelections")]
    pub(super) picks: Vec<GsPick>,
}

#[derive(Deserialize, Default)]
pub(super) struct IdOnly {
    #[serde(default)]
    pub(super) id: i32,
}

#[derive(Deserialize, Default)]
pub(super) struct GameflowSession {
    #[serde(default, rename = "gameData")]
    pub(super) game_data: GameData,
    #[serde(default)]
    pub(super) queue: IdOnly,
    #[serde(default, rename = "queueId")]
    pub(super) queue_id: i32,
}

impl GameflowSession {
    pub(super) fn queue_id(&self) -> i32 {
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

    pub(super) fn queue_name(&self) -> String {
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

    pub(super) fn roster(&self, self_st: &ConnStatus) -> (Vec<PlayerRef>, Vec<PlayerRef>) {
        let to_refs = |list: &[GsParticipant]| -> Vec<PlayerRef> {
            let mut out = Vec::with_capacity(list.len());
            for p in list {
                let mut g = opt_name(p.game_name.clone());
                let mut t = opt_name(p.tag_line.clone());
                if g.is_none() {
                    let (a, b) = split_name(&p.summoner_name);
                    g = opt_name(a);
                    t = opt_name(b);
                }
                let sid = opt_id(p.summoner_id.clone());
                let pu = opt_id(p.puuid.clone());
                if pu.is_none() && g.is_none() && sid.is_none() {
                    continue;
                }
                let is_self = is_self_match(
                    pu.as_deref().unwrap_or(""),
                    g.as_deref().unwrap_or(""),
                    t.as_deref().unwrap_or(""),
                    self_st,
                );
                out.push(PlayerRef {
                    puuid: pu,
                    summoner_id: sid,
                    is_self,
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
            if let Some(pu) = &r.puuid {
                side.insert(pu.to_lowercase(), 0);
            }
        }
        for r in &two {
            if let Some(pu) = &r.puuid {
                side.insert(pu.to_lowercase(), 1);
            }
        }

        for r in one.iter_mut() {
            if r.champion_id == 0 {
                if let Some(pu) = &r.puuid {
                    r.champion_id = champ_by.get(&pu.to_lowercase()).copied().unwrap_or(0);
                }
            }
        }
        for r in two.iter_mut() {
            if r.champion_id == 0 {
                if let Some(pu) = &r.puuid {
                    r.champion_id = champ_by.get(&pu.to_lowercase()).copied().unwrap_or(0);
                }
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
                puuid: opt_id(pk.puuid.clone()),
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

#[derive(Deserialize, Default)]
pub(super) struct CsPlayer {
    #[serde(default)]
    pub(super) puuid: String,
    #[serde(
        default,
        rename = "summonerId",
        deserialize_with = "crate::util::de_flex_str"
    )]
    pub(super) summoner_id: String,
    #[serde(default, rename = "championId")]
    pub(super) champion_id: i32,
    #[serde(default, rename = "gameName")]
    pub(super) game_name: String,
    #[serde(default, rename = "tagLine")]
    pub(super) tag_line: String,
    #[serde(default, rename = "profileIconId")]
    pub(super) profile_icon_id: i32,
}

#[derive(Deserialize, Default)]
pub(super) struct ChampSelectBody {
    #[serde(default, rename = "myTeam")]
    pub(super) my_team: Vec<CsPlayer>,
    #[serde(default, rename = "theirTeam")]
    pub(super) their_team: Vec<CsPlayer>,
}

#[derive(Deserialize, Default)]
pub(super) struct LobbyBody {
    #[serde(default, rename = "gameQueueConfig")]
    pub(super) game_queue_config: IdQueue,
    #[serde(default)]
    pub(super) members: Vec<LobbyMember>,
}

#[derive(Deserialize, Default)]
pub(super) struct IdQueue {
    #[serde(default, rename = "queueId")]
    pub(super) queue_id: i32,
}

#[derive(Deserialize, Default)]
pub(super) struct LobbyMember {
    #[serde(default)]
    pub(super) puuid: String,
    #[serde(
        default,
        rename = "summonerId",
        deserialize_with = "crate::util::de_flex_str"
    )]
    pub(super) summoner_id: String,
    #[serde(default, rename = "gameName")]
    pub(super) game_name: String,
    #[serde(default, rename = "tagLine")]
    pub(super) tag_line: String,
    #[serde(default, rename = "profileIconId")]
    pub(super) profile_icon_id: i32,
    #[serde(default)]
    pub(super) team: i32,
}

#[derive(Deserialize, Default)]
pub(super) struct SummonerRaw {
    #[serde(default)]
    pub(super) puuid: String,
    #[serde(default, rename = "gameName")]
    pub(super) game_name: String,
    #[serde(default, rename = "tagLine")]
    pub(super) tag_line: String,
    #[serde(default, rename = "displayName")]
    pub(super) display_name: String,
    #[serde(default, rename = "profileIconId")]
    pub(super) profile_icon_id: i32,
    #[serde(
        default,
        rename = "summonerId",
        deserialize_with = "crate::util::de_flex_str"
    )]
    pub(super) summoner_id: String,
}

impl SummonerRaw {
    pub(super) fn apply(&self, r: &mut PlayerRef) {
        if r.game_name.is_none() {
            if !self.game_name.is_empty() {
                r.game_name = opt_name(self.game_name.clone());
                r.tag_line = opt_name(self.tag_line.clone());
            } else if !self.display_name.is_empty() {
                let (g, t) = split_name(&self.display_name);
                r.game_name = opt_name(g);
                r.tag_line = opt_name(t);
            }
        }
        if r.profile_icon_id == 0 && self.profile_icon_id != 0 {
            r.profile_icon_id = self.profile_icon_id;
        }
        if r.summoner_id.is_none() {
            r.summoner_id = opt_id(self.summoner_id.clone());
        }
        if r.puuid.is_none() {
            r.puuid = opt_id(self.puuid.clone());
        }
    }

    pub(super) fn valid(&self) -> bool {
        !(self.puuid.is_empty()
            && self.game_name.is_empty()
            && self.display_name.is_empty()
            && self.profile_icon_id == 0)
    }
}
