use super::*;
use crate::lcu::State;
use crate::service::http::FakeHttp;
use std::collections::HashSet;
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
    ) -> BoxFut<'a, Result<Vec<RankedInfo>, crate::error::AppError>> {
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
    ) -> BoxFut<'a, Result<MatchPage, crate::error::AppError>> {
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
    ) -> BoxFut<'a, Result<SummonerResult, crate::error::AppError>> {
        Box::pin(async move {
            self.summoners
                .get(&name)
                .cloned()
                .ok_or_else(|| crate::error::AppError::Msg("not found".into()))
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
    fn player_list<'a>(&'a self) -> BoxFut<'a, Result<Vec<Player>, crate::error::AppError>> {
        Box::pin(async move {
            match &self.err {
                Some(e) => Err(e.clone().into()),
                None => Ok(self.players.clone()),
            }
        })
    }

    fn active_player_name<'a>(&'a self) -> BoxFut<'a, Result<String, crate::error::AppError>> {
        Box::pin(async move {
            match &self.err {
                Some(e) => Err(e.clone().into()),
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
    assert_eq!(
        resolve_queue_filter(None, 2400),
        vec![2400],
        "None=跟随当前"
    );
    assert!(resolve_queue_filter(None, 0).is_empty());
    assert!(
        resolve_queue_filter(Some(&[]), 2400).is_empty(),
        "Some(空)=全部"
    );
    assert!(resolve_queue_filter(None, 0).is_empty());
    assert_eq!(resolve_queue_filter(Some(&[420]), 2400), vec![420]);
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.phase, "Lobby");
    assert_eq!(st.queue_label, queue_info_for(420).name);
    assert_eq!(st.teams[0].player_count, 2);
    assert_eq!(st.teams[1].player_count, 1);
    assert!(st.teams[0].slots[0].is_self);
    assert_eq!(st.teams[0].slots[0].game_name.as_deref(), Some("我"));
    assert_eq!(st.teams[0].slots[0].summoner_id.as_deref(), Some("1001"));
    assert_eq!(st.teams[0].slots[1].summoner_id.as_deref(), Some("1002"));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.teams[0].player_count, 2);
    assert_eq!(st.teams[1].player_count, 1);
    assert_eq!(st.teams[0].slots[0].champion_id, Some(103));
    assert_eq!(st.teams[1].slots[0].champion_id, Some(22));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.teams[0].player_count, 5);
    assert_eq!(st.teams[0].slots[4].game_name.as_deref(), Some("队5"));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.teams[0].player_count, 2);
    assert_eq!(st.teams[1].player_count, 1);
    assert!(st.teams[0].slots[0].is_self);
    assert_eq!(st.teams[0].slots[0].champion_id, Some(1));
    assert_eq!(st.teams[1].slots[0].champion_id, Some(1));
    assert_eq!(st.teams[0].slots[1].profile_icon_id, Some(8));
    assert_eq!(st.teams[0].slots[1].summoner_id.as_deref(), Some("1002"));
}

#[tokio::test]
async fn live_unavailable_empty() {
    let http = routes_from(&[(PATH_GAMEFLOW_PHASE, r#""InProgress""#)]);
    let live = FakeLive {
        err: Some("connection refused".into()),
        ..Default::default()
    };
    let svc = new_test(http, FakeHist::default(), live);
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.teams[0].player_count, 0);
    assert_eq!(st.teams[1].player_count, 0);
}

#[tokio::test]
async fn none_empty_view() {
    let http = FakeHttp::new(connected_self());
    let svc = new_test(http, FakeHist::default(), FakeLive::default());
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.phase, "None");
    assert_eq!(st.teams[0].player_count, 0);
}

#[tokio::test]
async fn offline_errors() {
    let http = FakeHttp::new(ConnStatus {
        state: State::Disconnected,
        ..Default::default()
    });
    let svc = new_test(http, FakeHist::default(), FakeLive::default());
    let err = svc.get_gameflow_state(Some(vec![])).await.unwrap_err();
    assert!(err.to_string().contains("未连接"), "err={err}");
}

#[tokio::test]
async fn build_slots_enrich_and_hidden() {
    let http = routes_from(&[]);
    let mut hist = FakeHist::default();
    hist.ranked.insert(
        "1001".into(),
        RankedInfo {
            query_id: "1001".into(),
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
            puuid: Some("PA".into()),
            summoner_id: Some("1001".into()),
            game_name: Some("正常".into()),
            ..Default::default()
        },
        PlayerRef {
            puuid: Some("PB".into()),
            summoner_id: Some("1002".into()),
            game_name: Some("隐藏".into()),
            ..Default::default()
        },
    ];
    let slots = svc.build_slots(&refs, &[]).await;
    let a = &slots[0];
    assert_eq!(a.solo.as_deref(), Some("黄金 IV 45"));
    assert_eq!(a.flex, None);
    assert_eq!(a.win_rate, Some(50.0));
    assert_eq!(a.win_rate_sample, Some(2));
    assert_eq!(a.avg_kda, Some(5.38));
    assert_eq!(a.player_score, Some(7.9));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    let a = &st.teams[0].slots[0];
    assert_eq!(a.game_name.as_deref(), Some("我"));
    assert_eq!(a.profile_icon_id, Some(7));
    assert!(a.is_self);
    let b = &st.teams[0].slots[1];
    assert_eq!(b.game_name.as_deref(), Some("队友"));
    assert_eq!(b.tag_line.as_deref(), Some("CN2"));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    let b = &st.teams[0].slots[0];
    assert_eq!(b.game_name.as_deref(), Some("队友"));
    assert_eq!(b.profile_icon_id, Some(33));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    let a = &st.teams[0].slots[0];
    assert_eq!(a.game_name.as_deref(), Some("补位"));
    assert_eq!(a.tag_line.as_deref(), Some("T7"));
    assert_eq!(a.profile_icon_id, Some(66));
    assert_eq!(a.summoner_id.as_deref(), Some("777"));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.phase, "GameStart");
    assert_eq!(st.teams[0].player_count, 5);
    assert_eq!(st.teams[1].player_count, 5);
    assert_eq!(st.queue_label, queue_info_for(420).name);
    let a = &st.teams[0].slots[0];
    assert!(a.is_self);
    assert_eq!(a.game_name.as_deref(), Some("我"));
    assert_eq!(a.profile_icon_id, Some(7));
    assert_eq!(a.champion_id, Some(1));
    let e = &st.teams[1].slots[0];
    assert_eq!(e.game_name.as_deref(), Some("敌1"));
    assert_eq!(e.summoner_id.as_deref(), Some("2001"));
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
    assert_eq!(ally[0].puuid.as_deref(), Some("PSELF"));
    assert_eq!(enemy[0].puuid.as_deref(), Some("A1"));
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
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
    let st = svc.get_gameflow_state(Some(vec![])).await.unwrap();
    assert_eq!(st.teams[0].player_count, 5);
    assert_eq!(st.teams[1].player_count, 5, "teamTwo 漏第 5 人未补回");
    assert_eq!(st.queue_label, "海克斯大乱斗");
    let a = &st.teams[0].slots[0];
    assert!(a.is_self);
    assert_eq!(a.champion_id, Some(157));
    assert_eq!(a.profile_icon_id, Some(7063));
    assert_eq!(a.summoner_id.as_deref(), Some("17993441068"));
    let e = &st.teams[1].slots[4];
    assert!(e.filled);
    assert_eq!(e.game_name.as_deref(), Some("漏网者"));
    assert_eq!(e.champion_id, Some(112));
    assert_eq!(e.profile_icon_id, Some(999));
    assert_eq!(e.summoner_id.as_deref(), Some("5555"));
    assert_eq!(
        st.teams[1].slots[1].summoner_id.as_deref(),
        Some("4102901763434272")
    );
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
    let st = svc.get_gameflow_state(None).await.unwrap();
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
        puuid: Some("PA".into()),
        summoner_id: Some("1001".into()),
        game_name: Some("甲".into()),
        profile_icon_id: 1,
        ..Default::default()
    }];
    let slots = svc.build_slots(&refs, &[]).await;
    let b = &slots[0];
    assert_eq!(b.recent.len(), 20);
    assert_eq!(b.win_rate_sample, Some(20));
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
        puuid: Some("P2".into()),
        ..Default::default()
    };
    svc.fill_identity(&mut r).await;
    assert_eq!(r.game_name.as_deref(), Some("队友"));
    assert_eq!(r.profile_icon_id, 33);
    assert_eq!(r.summoner_id.as_deref(), Some("2"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// T3.4 回归：同 key 并发 fetch_career 必须合并为一次回源（single-flight）。
#[tokio::test]
async fn fetch_career_single_flight() {
    use crate::service::gameinfo::HistApi;
    use crate::service::history::{MatchPage, RankedInfo, SummonerResult};
    use crate::service::http::BoxFut;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc as StdArc;

    #[derive(Default)]
    struct SlowHist {
        hits: StdArc<AtomicUsize>,
    }

    impl HistApi for SlowHist {
        fn get_players_ranked<'a>(
            &'a self,
            _ids: Vec<String>,
        ) -> BoxFut<'a, Result<Vec<RankedInfo>, crate::error::AppError>> {
            Box::pin(async move { Ok(Vec::new()) })
        }
        fn get_matches<'a>(
            &'a self,
            _puuid: String,
            _page: i32,
        ) -> BoxFut<'a, Result<MatchPage, crate::error::AppError>> {
            let hits = self.hits.clone();
            Box::pin(async move {
                hits.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                Ok(MatchPage::default())
            })
        }
        fn search_summoner<'a>(
            &'a self,
            _name: String,
        ) -> BoxFut<'a, Result<SummonerResult, crate::error::AppError>> {
            Box::pin(async move { Err("x".into()) })
        }
    }

    let hist = StdArc::new(SlowHist::default());
    let hits = hist.hits.clone();
    let http = StdArc::new(FakeHttp::connected());
    let live = StdArc::new(FakeLive::default());
    let svc = GameinfoService::new(http, hist, live);

    let (a, b) = tokio::join!(svc.fetch_career("PX", &[]), svc.fetch_career("PX", &[]));
    assert_eq!(a.hidden, b.hidden);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "同 key 并发必须合并为一次回源"
    );
}
