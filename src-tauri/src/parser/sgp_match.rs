//! SGP 战绩解析：match-history-query SUMMARY → 与 LCU 同构的 MatchSummary。
//! 形态（League Akari 契约）：`{games:[{metadata, json:{扁平 participants}}]}`。

use serde::Deserialize;
use serde_json::Value;

use super::lcu_match::{build_summary, pick_participant, MatchSummary};

/* ─── SGP 原始 JSON 结构 ───────────────────────────────────────── */

#[derive(Debug, Clone, Default, Deserialize)]
struct SgpPerks {
    #[serde(default)]
    styles: Vec<SgpStyle>,
}

#[derive(Debug, Clone, Deserialize)]
struct SgpStyle {
    #[serde(default)]
    selections: Vec<SgpSelection>,
}

#[derive(Debug, Clone, Deserialize)]
struct SgpSelection {
    #[serde(default)]
    perk: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct SgpParticipant {
    #[serde(default)]
    puuid: Value,
    #[serde(default, rename = "teamId")]
    team_id: i32,
    #[serde(default, rename = "championId")]
    champion_id: i32,
    #[serde(default, rename = "champLevel")]
    champ_level: i32,
    #[serde(default, rename = "spell1Id")]
    spell1_id: i32,
    #[serde(default, rename = "spell2Id")]
    spell2_id: i32,
    #[serde(default)]
    kills: i32,
    #[serde(default)]
    deaths: i32,
    #[serde(default)]
    assists: i32,
    #[serde(default)]
    win: bool,
    #[serde(default, rename = "item0")]
    item0: i32,
    #[serde(default, rename = "item1")]
    item1: i32,
    #[serde(default, rename = "item2")]
    item2: i32,
    #[serde(default, rename = "item3")]
    item3: i32,
    #[serde(default, rename = "item4")]
    item4: i32,
    #[serde(default, rename = "item5")]
    item5: i32,
    #[serde(default, rename = "item6")]
    item6: i32,
    #[serde(default, rename = "playerAugment1")]
    player_augment1: i32,
    #[serde(default, rename = "playerAugment2")]
    player_augment2: i32,
    #[serde(default, rename = "playerAugment3")]
    player_augment3: i32,
    #[serde(default, rename = "playerAugment4")]
    player_augment4: i32,
    #[serde(default, rename = "playerAugment5")]
    player_augment5: i32,
    #[serde(default, rename = "playerAugment6")]
    player_augment6: i32,
    #[serde(default, rename = "subteamPlacement")]
    subteam_placement: i32,
    #[serde(default, rename = "gameEndedInEarlySurrender")]
    game_ended_in_early_surrender: bool,
    #[serde(default, rename = "teamEarlySurrendered")]
    team_early_surrendered: bool,
    #[serde(default, rename = "totalMinionsKilled")]
    total_minions_killed: i32,
    #[serde(default, rename = "neutralMinionsKilled")]
    neutral_minions_killed: i32,
    #[serde(default, rename = "goldEarned")]
    gold_earned: i32,
    #[serde(default, rename = "totalDamageDealtToChampions")]
    total_damage_dealt_to_champions: i32,
    #[serde(default, rename = "totalHeal")]
    total_heal: i32,
    #[serde(default)]
    perks: SgpPerks,
}

impl SgpParticipant {
    fn perk0(&self) -> i32 {
        self.perks
            .styles
            .first()
            .and_then(|s| s.selections.first())
            .map(|s| s.perk)
            .unwrap_or(0)
    }

    fn augments(&self) -> Vec<i32> {
        vec![
            self.player_augment1,
            self.player_augment2,
            self.player_augment3,
            self.player_augment4,
            self.player_augment5,
            self.player_augment6,
        ]
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SgpGameJson {
    #[serde(default, rename = "gameId")]
    game_id: i64,
    #[serde(default, rename = "gameCreation")]
    game_creation: i64,
    #[serde(default, rename = "gameDuration")]
    game_duration: i32,
    #[serde(default, rename = "queueId")]
    queue_id: i32,
    #[serde(default, rename = "mapId")]
    map_id: i32,
    #[serde(default)]
    participants: Vec<SgpParticipant>,
}

#[derive(Debug, Clone, Deserialize)]
struct SgpGameSummary {
    json: SgpGameJson,
}

/* ─── 解析入口 ─────────────────────────────────────────────────── */

pub fn parse_sgp_summaries(data: &[u8], self_puuid: &str) -> Result<Vec<MatchSummary>, String> {
    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default)]
        games: Vec<SgpGameSummary>,
    }
    let wrap: Wrap = serde_json::from_slice(data).map_err(|e| format!("解析战绩列表失败: {e}"))?;

    let mut summaries = Vec::with_capacity(wrap.games.len());
    for sg in &wrap.games {
        if sg.json.participants.is_empty() {
            continue;
        }
        let g = adapt_sgp_game(&sg.json);
        let p = pick_participant(&g, self_puuid);
        summaries.push(build_summary(&g, &p));
    }
    Ok(summaries)
}

/* ─── SGP → LCU 形态适配 ───────────────────────────────────────── */

fn adapt_sgp_game(sj: &SgpGameJson) -> super::lcu_match::LcuGame {
    use super::lcu_match::{LcuGame, LcuIdentity, LcuIdentityPlayer, LcuParticipant, LcuStats};
    let mut g = LcuGame {
        game_id: sj.game_id,
        game_creation: sj.game_creation,
        game_duration: sj.game_duration,
        queue_id: sj.queue_id,
        map_id: sj.map_id,
        game_ended_in_early_surrender: false,
        participants: Vec::new(),
        participant_identities: Vec::new(),
    };
    for (i, sp) in sj.participants.iter().enumerate() {
        let pid = (i + 1) as i32;
        let lp = LcuParticipant {
            participant_id: pid,
            team_id: sp.team_id,
            team_id_alt: 0,
            champion_id: sp.champion_id,
            spell1_id: sp.spell1_id,
            spell2_id: sp.spell2_id,
            stats: LcuStats {
                win: sp.win,
                kills: sp.kills,
                deaths: sp.deaths,
                assists: sp.assists,
                champ_level: sp.champ_level,
                item0: sp.item0,
                item1: sp.item1,
                item2: sp.item2,
                item3: sp.item3,
                item4: sp.item4,
                item5: sp.item5,
                item6: sp.item6,
                perk0: sp.perk0(),
                total_minions_killed: sp.total_minions_killed,
                neutral_minions_killed: sp.neutral_minions_killed,
                gold_earned: sp.gold_earned,
                total_damage_dealt_to_champions: sp.total_damage_dealt_to_champions,
                total_heal: sp.total_heal,
                game_ended_in_early_surrender: sp.game_ended_in_early_surrender
                    || sp.team_early_surrendered,
                team_early_surrendered: sp.team_early_surrendered,
                subteam_placement: sp.subteam_placement,
                highest_achieved_season_tier: String::new(),
                augments: sp.augments(),
                player_augment1: 0,
                player_augment2: 0,
                player_augment3: 0,
                player_augment4: 0,
                player_augment5: 0,
            },
        };
        g.participants.push(lp);
        let mut ident = LcuIdentity {
            participant_id: pid,
            player: LcuIdentityPlayer::default(),
        };
        ident.player.puuid = sp.puuid.clone();
        g.participant_identities.push(ident);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    const SGP_FIXTURE: &str = r#"{"games":[
    	{"metadata":{"participants":["5e65c58d-5b4a-5936-9104-806bb8443eef","other-puuid-0000000000000"]},
    	 "json":{"gameId":900001,"gameCreation":1758000000000,"gameDuration":1234,"queueId":420,"mapId":11,
    		"participants":[
    			{"puuid":"other-puuid-0000000000000","teamId":200,"championId":103,"champLevel":15,
    			 "spell1Id":4,"spell2Id":14,"kills":9,"deaths":2,"assists":7,"win":false,
    			 "item0":1,"item1":2,"item2":3,"item3":4,"item4":5,"item5":6,"item6":3340,
    			 "totalMinionsKilled":180,"neutralMinionsKilled":20,"goldEarned":13000,
    			 "totalDamageDealtToChampions":21000,"totalHeal":900,
    			 "perks":{"statPerks":{},"styles":[{"style":8000,"selections":[{"perk":8005},{"perk":9111}]}]}},
    			{"puuid":"5e65c58d-5b4a-5936-9104-806bb8443eef","teamId":100,"championId":22,"champLevel":16,
    			 "spell1Id":7,"spell2Id":6,"kills":6,"deaths":1,"assists":8,"win":true,
    			 "item0":10,"item1":0,"item2":30,"item3":0,"item4":0,"item5":0,"item6":3340,
    			 "totalMinionsKilled":205,"neutralMinionsKilled":15,"goldEarned":14200,
    			 "totalDamageDealtToChampions":18500,"totalHeal":420,
    			 "perks":{"statPerks":{},"styles":[{"style":8100,"selections":[{"perk":8112},{"perk":8138}]}]}}
    		]}},
    	{"metadata":{"participants":["5e65c58d-5b4a-5936-9104-806bb8443eef"]},
    	 "json":{"gameId":900002,"gameCreation":1757000000000,"gameDuration":900,"queueId":1700,"mapId":30,
    		"participants":[
    			{"puuid":"5e65c58d-5b4a-5936-9104-806bb8443eef","teamId":0,"championId":86,"champLevel":11,
    			 "spell1Id":4,"spell2Id":6,"kills":3,"deaths":4,"assists":2,"win":false,"subteamPlacement":2,
    			 "item0":1,"item1":2,"item2":0,"item3":0,"item4":0,"item5":0,"item6":0,
    			 "playerAugment1":111,"playerAugment2":0,"playerAugment3":222,"playerAugment4":0,
    			 "playerAugment5":0,"playerAugment6":0,
    			 "totalMinionsKilled":40,"neutralMinionsKilled":5,"goldEarned":7000,
    			 "totalDamageDealtToChampions":9000,"totalHeal":100,"gameEndedInEarlySurrender":false,
    			 "teamEarlySurrendered":true,
    			 "perks":{"statPerks":{},"styles":[{"style":8000,"selections":[{"perk":8010}]}]}}
    		]}}
    ]}"#;

    #[test]
    fn parse_sgp_summaries_basic() {
        const SELF: &str = "5e65c58d-5b4a-5936-9104-806bb8443eef";
        let sums = parse_sgp_summaries(SGP_FIXTURE.as_bytes(), SELF).unwrap();
        assert_eq!(sums.len(), 2);

        let s = &sums[0];
        assert_eq!(s.game_id, 900001);
        assert_eq!(s.queue_id, 420);
        assert_eq!(s.champion_id, 22, "本人参赛者未命中");
        assert_eq!(s.kills, 6);
        assert_eq!(s.deaths, 1);
        assert_eq!(s.assists, 8);
        assert_eq!(s.spell1_id, 7);
        assert_eq!(s.spell2_id, 6);
        assert_eq!(s.rune_id, 8112, "perks 基石");
        assert_eq!(s.kda, "14.00");
        assert!(s.win);
        assert_eq!(s.items.len(), 7);
        assert_eq!(s.items[0], 10);
        assert_eq!(s.items[6], 3340);
        assert_eq!(s.cs, 220);
        assert_eq!(s.gold, 14200);
        assert_eq!(s.total_damage, 18500);
        assert!(!s.queue_name.is_empty());
        assert!(!s.arena);

        let a = &sums[1];
        assert_eq!(a.game_id, 900002);
        assert!(a.arena);
        assert_eq!(a.placement, 2);
        assert_eq!(a.augment_ids, vec![111, 222]);
        assert!(a.remake, "teamEarlySurrendered 应判重赛");
    }

    #[test]
    fn parse_sgp_summaries_fallback_first() {
        let sums = parse_sgp_summaries(SGP_FIXTURE.as_bytes(), "nobody").unwrap();
        assert_eq!(sums.len(), 2);
        assert_eq!(sums[0].champion_id, 103, "fallback 首位参与者");
    }

    #[test]
    fn parse_sgp_summaries_empty_and_bad() {
        let sums = parse_sgp_summaries(r#"{"games":[]}"#.as_bytes(), "x").unwrap();
        assert_eq!(sums.len(), 0);
        assert!(parse_sgp_summaries(b"{bad json", "x").is_err());
    }
}
