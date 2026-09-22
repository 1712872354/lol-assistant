package gameinfo

import (
	"errors"
	"fmt"
	"testing"

	"github.com/1712872354/lol-assistant/internal/lcu"
	"github.com/1712872354/lol-assistant/internal/liveclient"
	"github.com/1712872354/lol-assistant/internal/parser"
	"github.com/1712872354/lol-assistant/service/history"
)

/* ── fakes ─────────────────────────────────────────── */

type fakeLCU struct {
	routes map[string]string
}

func (f *fakeLCU) Get(path string) (int, []byte, error) {
	if body, ok := f.routes[path]; ok {
		return 200, []byte(body), nil
	}
	return 404, nil, nil
}

type fakeHist struct {
	ranked    map[string]history.RankedInfo
	matches   map[string][]parser.MatchSummary
	pages     map[string][][]parser.MatchSummary // 多页形态（优先于 matches；HasMore 逐页递进）
	matchErr  map[string]bool
	summoners map[string]history.SummonerResult
}

func (f *fakeHist) GetPlayersRanked(ids []string) ([]history.RankedInfo, error) {
	var out []history.RankedInfo
	for _, id := range ids {
		if r, ok := f.ranked[id]; ok {
			out = append(out, r)
		}
	}
	return out, nil
}

func (f *fakeHist) GetMatches(puuid string, page int) (history.MatchPage, error) {
	if f.matchErr[puuid] {
		return history.MatchPage{}, errors.New("career hidden")
	}
	if f.pages != nil {
		pgs := f.pages[puuid]
		if page >= len(pgs) {
			return history.MatchPage{Puuid: puuid, Page: page, HasMore: false}, nil
		}
		return history.MatchPage{Puuid: puuid, Page: page, Summaries: pgs[page], HasMore: page < len(pgs)-1}, nil
	}
	return history.MatchPage{Summaries: f.matches[puuid]}, nil
}

func (f *fakeHist) SearchSummoner(name string) (history.SummonerResult, error) {
	if r, ok := f.summoners[name]; ok {
		return r, nil
	}
	return history.SummonerResult{}, errors.New("not found")
}

type fakeLive struct {
	players []liveclient.Player
	active  string
	err     error
}

func (f *fakeLive) PlayerList() ([]liveclient.Player, error) {
	if f.err != nil {
		return nil, f.err
	}
	return f.players, nil
}

func (f *fakeLive) ActivePlayerName() (string, error) {
	if f.err != nil {
		return "", f.err
	}
	return f.active, nil
}

func newTestService(cli lcuAPI, hist histAPI, live liveAPI) *Service {
	return &Service{
		cliFn: func() (lcuAPI, error) { return cli, nil },
		selfFn: func() lcu.ConnStatus {
			return lcu.ConnStatus{State: lcu.StateConnected, Puuid: "PSELF"}
		},
		hist:        hist,
		live:        live,
		concurrency: 2,
	}
}

const phasePath = lcu.PathGameflowPhase

/* ── Lobby / Matchmaking / ReadyCheck ──────────────── */

func TestGetGameflowState_Lobby(t *testing.T) {
	cli := &fakeLCU{routes: map[string]string{
		phasePath: `"Lobby"`,
		lcu.PathLobby: `{
			"gameQueueConfig": {"queueId": 420},
			"members": [
				{"puuid":"PSELF","summonerId":1001,"gameName":"我","tagLine":"CN1","profileIconId":7,"team":1},
				{"puuid":"P2","summonerId":"1002","gameName":"队友","tagLine":"CN2","profileIconId":8,"team":1},
				{"puuid":"P3","summonerId":1003,"gameName":"敌1","tagLine":"CN3","profileIconId":9,"team":2}
			]
		}`,
	}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Phase != "Lobby" || st.Teams[0].PhaseLabel != "房间内" {
		t.Fatalf("phase=%s label=%s", st.Phase, st.Teams[0].PhaseLabel)
	}
	if want := parser.QueueInfoFor(420).Name; st.QueueLabel != want {
		t.Fatalf("queue=%q want %q", st.QueueLabel, want)
	}
	if st.Teams[0].PlayerCount != 2 || st.Teams[1].PlayerCount != 1 {
		t.Fatalf("counts ally=%d enemy=%d", st.Teams[0].PlayerCount, st.Teams[1].PlayerCount)
	}
	if !st.Teams[0].Slots[0].IsSelf || st.Teams[0].Slots[0].GameName != "我" {
		t.Fatalf("self slot=%+v", st.Teams[0].Slots[0])
	}
	// summonerId number/string 双形态
	if st.Teams[0].Slots[0].SummonerID != "1001" || st.Teams[0].Slots[1].SummonerID != "1002" {
		t.Fatalf("summoner ids %q %q", st.Teams[0].Slots[0].SummonerID, st.Teams[0].Slots[1].SummonerID)
	}
	if len(st.Teams[0].Slots) != 5 || len(st.Teams[1].Slots) != 5 {
		t.Fatal("slots must be 5 per team")
	}
}

/* ── ChampSelect ──────────────────────────────────── */

func TestGetGameflowState_ChampSelect(t *testing.T) {
	cli := &fakeLCU{routes: map[string]string{
		phasePath: `"ChampSelect"`,
		lcu.PathChampSelectSession: `{
			"myTeam": [
				{"puuid":"PSELF","summonerId":1,"championId":103},
				{"puuid":"P2","summonerId":2,"championId":0}
			],
			"theirTeam": [
				{"puuid":"P9","summonerId":9,"championId":22},
				{"puuid":"","summonerId":0,"championId":0}
			]
		}`,
		lcu.PathGameflowSession:                    `{"queueId": 420}`,
		fmt.Sprintf(lcu.PathSummonerByPuuid, "P2"): `{"puuid":"P2","gameName":"队友","tagLine":"T2","profileIconId":33,"summonerId":"2"}`,
		fmt.Sprintf(lcu.PathSummonerByPuuid, "P9"): `{"puuid":"P9","gameName":"敌人","tagLine":"T9","profileIconId":44,"summonerId":"9"}`,
	}}
	// PathSummonerByPuuid 含 %s 格式化，路由键需与 fmt.Sprintf 结果一致
	svc := newTestService(cli, &fakeHist{}, &fakeLive{})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Teams[0].PhaseLabel != "选人中" {
		t.Fatalf("label=%s", st.Teams[0].PhaseLabel)
	}
	// 盲选无 id 条目被跳过 → enemy 仅 1 槽
	if st.Teams[0].PlayerCount != 2 || st.Teams[1].PlayerCount != 1 {
		t.Fatalf("counts ally=%d enemy=%d", st.Teams[0].PlayerCount, st.Teams[1].PlayerCount)
	}
	if st.Teams[0].Slots[0].ChampionID != 103 || st.Teams[1].Slots[0].ChampionID != 22 {
		t.Fatalf("champs ally=%d enemy=%d", st.Teams[0].Slots[0].ChampionID, st.Teams[1].Slots[0].ChampionID)
	}
}

/* ── Live Client（游戏中/结算/重连） ───────────────── */

func TestGetGameflowState_Live(t *testing.T) {
	cli := &fakeLCU{routes: map[string]string{
		phasePath:                                  `"InProgress"`,
		lcu.PathGameflowSession:                    `{"queueId": 420}`,
		lcu.PathGDChampionSummary:                  `[{"id":1,"alias":"Annie","name":"黑暗之女"},{"id":22,"alias":"Ashe","name":"寒冰射手"}]`,
		fmt.Sprintf(lcu.PathSummonerByPuuid, "PA"): `{"puuid":"PA","gameName":"我","tagLine":"CN1","profileIconId":7,"summonerId":"1001"}`,
	}}
	live := &fakeLive{
		active: "我#CN1",
		players: []liveclient.Player{
			{SummonerName: "我#CN1", Puuid: "PA", ChampionName: "Annie", Team: 100},
			{RiotIdGameName: "队友", RiotIdTagLine: "CN2", Puuid: "PB", ChampionName: "Ashe", Team: liveclient.TeamBlue},
			{RiotIdGameName: "敌人", RiotIdTagLine: "CN9", Puuid: "PC", ChampionName: "Annie", Team: liveclient.TeamRed},
		},
	}
	svc := newTestService(cli, &fakeHist{
		summoners: map[string]history.SummonerResult{
			"队友#CN2": {Puuid: "PB", GameName: "队友", TagLine: "CN2", ProfileIconID: 8, SummonerID: "1002"},
			"敌人#CN9": {Puuid: "PC", GameName: "敌人", TagLine: "CN9", ProfileIconID: 9, SummonerID: "1003"},
		},
	}, live)
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Teams[0].PhaseLabel != "游戏中" {
		t.Fatalf("label=%s", st.Teams[0].PhaseLabel)
	}
	if st.Teams[0].PlayerCount != 2 || st.Teams[1].PlayerCount != 1 {
		t.Fatalf("counts ally=%d enemy=%d", st.Teams[0].PlayerCount, st.Teams[1].PlayerCount)
	}
	if !st.Teams[0].Slots[0].IsSelf {
		t.Fatal("self not marked")
	}
	if st.Teams[0].Slots[0].ChampionID != 1 || st.Teams[1].Slots[0].ChampionID != 1 {
		t.Fatal("champion index mapping failed")
	}
	// 名字反查补 puuid/icon
	if st.Teams[0].Slots[1].ProfileIconID != 8 || st.Teams[0].Slots[1].SummonerID != "1002" {
		t.Fatalf("identity fill: %+v", st.Teams[0].Slots[1])
	}
}

func TestGetGameflowState_LiveUnavailable(t *testing.T) {
	cli := &fakeLCU{routes: map[string]string{phasePath: `"InProgress"`}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{err: errors.New("connection refused")})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Teams[0].PlayerCount != 0 || st.Teams[1].PlayerCount != 0 {
		t.Fatal("must degrade to empty slots")
	}
}

/* ── None 空态 ────────────────────────────────────── */

func TestGetGameflowState_None(t *testing.T) {
	svc := newTestService(&fakeLCU{}, &fakeHist{}, &fakeLive{})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Phase != "None" || st.Teams[0].PhaseLabel != "大厅中" {
		t.Fatalf("phase=%s label=%s", st.Phase, st.Teams[0].PhaseLabel)
	}
	if st.Teams[0].PlayerCount != 0 {
		t.Fatal("none phase must be empty")
	}
}

/* ── 补数：段位 / 近况 / 生涯隐藏 ─────────────────── */

func TestBuildSlots_EnrichAndHiddenCareer(t *testing.T) {
	cli := &fakeLCU{routes: map[string]string{phasePath: `"Lobby"`}}
	hist := &fakeHist{
		ranked: map[string]history.RankedInfo{
			"1001": {SummonerID: "1001", Puuid: "PA", Solo: "黄金 IV 45", Flex: "未定级"},
		},
		matches: map[string][]parser.MatchSummary{
			"PA": {
				{Win: true, Kills: 5, Deaths: 1, Assists: 5, QueueShort: "海斗", ShortTime: "09-21", ChampionID: 1},
				{Win: false, Kills: 1, Deaths: 4, Assists: 2, QueueShort: "海斗", ShortTime: "09-20", ChampionID: 2},
			},
		},
		matchErr: map[string]bool{"PB": true},
	}
	svc := newTestService(cli, hist, &fakeLive{})
	refs := []playerRef{
		{puuid: "PA", summonerID: "1001", gameName: "正常"},
		{puuid: "PB", summonerID: "1002", gameName: "隐藏"},
	}
	slots := svc.buildSlots(cli, refs, nil)

	a := slots[0]
	if a.Solo != "黄金 IV 45" || a.Flex != "" {
		t.Fatalf("rank: %+v", a)
	}
	// 胜率 50.0；avgKda=(5+5)/1 + (1+2)/4 = 10.75 / 2 = 5.38（按 (K+A)/max(D,1) 均值）
	if a.WinRate != 50.0 || a.WinRateSample != 2 {
		t.Fatalf("winrate=%v sample=%d", a.WinRate, a.WinRateSample)
	}
	if a.AvgKda != 5.38 {
		t.Fatalf("avgKda=%v", a.AvgKda)
	}
	if a.Rating != 7.9 { // 50/20 + 5.38 = 7.88 → 7.9
		t.Fatalf("rating=%v", a.Rating)
	}
	b := slots[1]
	if !b.HiddenCareer || len(b.Recent) != 0 {
		t.Fatalf("hidden career: %+v", b)
	}
	// 5 槽补齐
	if len(slots) != 5 || slots[4].Filled {
		t.Fatalf("pad slots: %+v", slots)
	}
}

/* ── 回归：选人头像/名字补齐（2026-09-22 Bug①） ───── */

func TestGetGameflowState_ChampSelect_SessionNames(t *testing.T) {
	// 新版本选人条目自带 gameName/tagLine/profileIconId：无需任何反查
	cli := &fakeLCU{routes: map[string]string{
		phasePath: `"ChampSelect"`,
		lcu.PathChampSelectSession: `{
			"myTeam": [
				{"puuid":"PSELF","summonerId":1,"championId":103,"gameName":"我","tagLine":"CN1","profileIconId":7},
				{"puuid":"P2","summonerId":2,"championId":0,"gameName":"队友","tagLine":"CN2"}
			],
			"theirTeam": [{"puuid":"","summonerId":0,"championId":0}]
		}`,
	}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	a := st.Teams[0].Slots[0]
	if a.GameName != "我" || a.ProfileIconID != 7 || !a.IsSelf {
		t.Fatalf("session names: %+v", a)
	}
	if b := st.Teams[0].Slots[1]; b.GameName != "队友" || b.TagLine != "CN2" {
		t.Fatalf("teammate: %+v", b)
	}
}

func TestGetGameflowState_ChampSelect_RosterEnrich(t *testing.T) {
	// 选人条目缺名/头像 → gameflow 花名册按 puuid 对齐补齐
	cli := &fakeLCU{routes: map[string]string{
		phasePath:                  `"ChampSelect"`,
		lcu.PathChampSelectSession: `{"myTeam":[{"puuid":"P2","summonerId":2,"championId":0}],"theirTeam":[]}`,
		lcu.PathGameflowSession: `{"gameData":{"teamOne":[
			{"puuid":"PSELF","summonerId":1,"gameName":"我","tagLine":"CN1","profileIconId":7},
			{"puuid":"P2","summonerId":2,"gameName":"队友","tagLine":"T2","profileIconId":33}]}}`,
	}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	b := st.Teams[0].Slots[0]
	if b.GameName != "队友" || b.ProfileIconID != 33 {
		t.Fatalf("roster enrich: %+v", b)
	}
}

func TestGetGameflowState_ChampSelect_SummonerIDFallback(t *testing.T) {
	// by-puuid 未命中 → by-summonerId 兜底 → displayName "name#tag" 拆分
	cli := &fakeLCU{routes: map[string]string{
		phasePath:                                `"ChampSelect"`,
		lcu.PathChampSelectSession:               `{"myTeam":[{"puuid":"PX","summonerId":777,"championId":0}],"theirTeam":[]}`,
		fmt.Sprintf(lcu.PathSummonerByID, "777"): `{"puuid":"PX","displayName":"补位#T7","profileIconId":66,"summonerId":777}`,
	}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	a := st.Teams[0].Slots[0]
	if a.GameName != "补位" || a.TagLine != "T7" || a.ProfileIconID != 66 || a.SummonerID != "777" {
		t.Fatalf("by-summonerId fallback: %+v", a)
	}
}

/* ── 回归：游戏中双队数据（2026-09-22 Bug②） ─────── */

func TestGetGameflowState_InGameRoster(t *testing.T) {
	// Live Client 不可达也要有 5/5 双队 + 自带头像/名（GameStart 早期即有数据）
	roster := `{"gameData":{"queueId":420,
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
			{"puuid":"E5","summonerId":2005,"gameName":"敌5","tagLine":"E5"}]}}`
	cli := &fakeLCU{routes: map[string]string{
		phasePath:               `"GameStart"`,
		lcu.PathGameflowSession: roster,
	}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{err: errors.New("live down")})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Phase != "GameStart" || st.Teams[0].PhaseLabel != "游戏启动" {
		t.Fatalf("phase=%s label=%s", st.Phase, st.Teams[0].PhaseLabel)
	}
	if st.Teams[0].PlayerCount != 5 || st.Teams[1].PlayerCount != 5 {
		t.Fatalf("counts ally=%d enemy=%d（回归：游戏中敌方 0 人）", st.Teams[0].PlayerCount, st.Teams[1].PlayerCount)
	}
	if want := parser.QueueInfoFor(420).Name; st.QueueLabel != want {
		t.Fatalf("queue=%q want %q（回归：跟随对局药丸缺失）", st.QueueLabel, want)
	}
	if a := st.Teams[0].Slots[0]; !a.IsSelf || a.GameName != "我" || a.ProfileIconID != 7 || a.ChampionID != 1 {
		t.Fatalf("ally self slot: %+v", a)
	}
	if e := st.Teams[1].Slots[0]; e.GameName != "敌1" || e.SummonerID != "2001" {
		t.Fatalf("enemy slot: %+v", e)
	}
}

func TestGameflowRoster_SelfInTeamTwo(t *testing.T) {
	cli := &fakeLCU{routes: map[string]string{
		lcu.PathGameflowSession: `{"gameData":{
			"teamOne":[{"puuid":"A1","summonerId":1,"gameName":"甲","tagLine":"T1"}],
			"teamTwo":[{"puuid":"PSELF","summonerId":2,"gameName":"我","tagLine":"CN1"}]}}`,
	}}
	sess, ok := fetchGameflowSession(cli)
	if !ok {
		t.Fatal("session parse failed")
	}
	ally, enemy := sess.roster(lcu.ConnStatus{Puuid: "PSELF"})
	if len(ally) != 1 || len(enemy) != 1 || ally[0].puuid != "PSELF" || enemy[0].puuid != "A1" {
		t.Fatalf("flip on self-in-teamTwo: ally=%+v enemy=%+v", ally, enemy)
	}
}

func TestGetGameflowState_LiveTeamUnusable(t *testing.T) {
	// team 字段不可用（全 TeamNone）时不允许 10 人挤我方 → 对半分兜底
	players := make([]liveclient.Player, 10)
	for i := range players {
		players[i] = liveclient.Player{
			RiotIdGameName: fmt.Sprintf("P%d", i),
			RiotIdTagLine:  "T",
			Puuid:          fmt.Sprintf("Q%d", i),
		}
	}
	players[0] = liveclient.Player{RiotIdGameName: "我", RiotIdTagLine: "CN1", Puuid: "Q0"}
	cli := &fakeLCU{routes: map[string]string{phasePath: `"InProgress"`}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{active: "我#CN1", players: players})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Teams[0].PlayerCount != 5 || st.Teams[1].PlayerCount != 5 {
		t.Fatalf("half-split fallback: ally=%d enemy=%d", st.Teams[0].PlayerCount, st.Teams[1].PlayerCount)
	}
	if !st.Teams[0].Slots[0].IsSelf {
		t.Fatal("self not marked in fallback split")
	}
}

/* ── 回归：teamTwo 漏第 5 人 + 队列真实位置（2026-09-22 Bug③） ───── */

// 实测形状（2026-09-22 探针 _gameflow_probe.txt）：条目无 gameName/tagLine、summonerName 恒空串、
// summonerId 是纯数字；队列在 gameData.queue{id,name,numPlayersPerTeam}；teamTwo 只有 4 条，
// 漏网者在 playerChampionSelections（10 人全）。
func TestGetGameflowState_InGameRosterHole(t *testing.T) {
	const missing = "84be6377-c593-537f-9b0c-709a1febab04"
	sess := `{"gameData":{
		"queue":{"id":2400,"name":"海克斯大乱斗 ","numPlayersPerTeam":5},
		"teamOne":[
			{"puuid":"PSELF","summonerId":17993441068,"summonerName":"","profileIconId":7063,"championId":157},
			{"puuid":"A2","summonerId":17775323153,"summonerName":"","profileIconId":6589,"championId":11},
			{"puuid":"A3","summonerId":17059462600,"summonerName":"","profileIconId":3543,"championId":99},
			{"puuid":"A4","summonerId":17506727465,"summonerName":"","profileIconId":6841,"championId":111},
			{"puuid":"A5","summonerId":16262047083,"summonerName":"","profileIconId":745,"championId":777}],
		"teamTwo":[
			{"puuid":"E1","summonerId":17004661252,"summonerName":"","profileIconId":4745,"championId":154},
			{"puuid":"E2","summonerId":4102901763434272,"summonerName":"","profileIconId":3542,"championId":45},
			{"puuid":"E3","summonerId":16067475272,"summonerName":"","profileIconId":3796,"championId":876},
			{"puuid":"E4","summonerId":18101421026,"summonerName":"","profileIconId":4568,"championId":203}],
		"playerChampionSelections":[
			{"puuid":"A4","championId":111},{"puuid":"A2","championId":11},{"puuid":"A5","championId":777},
			{"puuid":"PSELF","championId":157},{"puuid":"A3","championId":99},{"puuid":"E3","championId":876},
			{"puuid":"` + missing + `","championId":112},{"puuid":"E1","championId":154},
			{"puuid":"E2","championId":45},{"puuid":"E4","championId":203}]}}`
	cli := &fakeLCU{routes: map[string]string{
		phasePath:               `"InProgress"`,
		lcu.PathGameflowSession: sess,
		fmt.Sprintf(lcu.PathSummonerByPuuid, missing): `{"puuid":"` + missing + `","gameName":"漏网者","tagLine":"CN5","profileIconId":999,"summonerId":"5555"}`,
	}}
	svc := newTestService(cli, &fakeHist{}, &fakeLive{err: errors.New("live down")})
	st, err := svc.GetGameflowState(nil)
	if err != nil {
		t.Fatal(err)
	}
	if st.Teams[0].PlayerCount != 5 || st.Teams[1].PlayerCount != 5 {
		t.Fatalf("counts ally=%d enemy=%d（回归：teamTwo 漏第 5 人未补回）", st.Teams[0].PlayerCount, st.Teams[1].PlayerCount)
	}
	if want := "海克斯大乱斗"; st.QueueLabel != want {
		t.Fatalf("queue=%q want %q（回归：跟随对局药丸缺失，实测队列在 gameData.queue.id）", st.QueueLabel, want)
	}
	if a := st.Teams[0].Slots[0]; !a.IsSelf || a.ChampionID != 157 || a.ProfileIconID != 7063 || a.SummonerID != "17993441068" {
		t.Fatalf("ally self slot: %+v", a)
	}
	// 漏网者补回敌方第 5 槽并经 by-puuid 回查补齐名字/头像/summonerId
	e := st.Teams[1].Slots[4]
	if !e.Filled || e.GameName != "漏网者" || e.ChampionID != 112 || e.ProfileIconID != 999 || e.SummonerID != "5555" {
		t.Fatalf("enemy 5th slot: %+v", e)
	}
	// 超大 summonerId 数字形态不丢精度
	if got := st.Teams[1].Slots[1].SummonerID; got != "4102901763434272" {
		t.Fatalf("huge summonerId: %q", got)
	}
}

func TestGameflowSession_QueueName(t *testing.T) {
	parse := func(raw string) gameflowSession {
		s, ok := fetchGameflowSession(&fakeLCU{routes: map[string]string{lcu.PathGameflowSession: raw}})
		if !ok {
			t.Fatal("session parse failed")
		}
		return s
	}
	// 实测位置：gameData.queue.id（本地表已收录 → 表内名）
	if got := parse(`{"gameData":{"queue":{"id":2400,"name":"海克斯大乱斗 "}}}`).queueName(); got != "海克斯大乱斗" {
		t.Fatalf("known id: %q", got)
	}
	// 本地表未收录 → LCU 官方名（TrimSpace 去尾空格）
	if got := parse(`{"gameData":{"queue":{"id":86783593,"name":"末日人机 "}}}`).queueName(); got != "末日人机" {
		t.Fatalf("official fallback: %q", got)
	}
	// 历史形态兜底
	if got := parse(`{"gameData":{"queueId":440}}`).queueName(); got != parser.QueueInfoFor(440).Name {
		t.Fatalf("legacy gameData.queueId: %q", got)
	}
	if got := parse(`{"queue":{"id":420}}`).queueName(); got != parser.QueueInfoFor(420).Name {
		t.Fatalf("legacy queue.id: %q", got)
	}
	if got := parse(`{"queueId":450}`).queueName(); got != parser.QueueInfoFor(450).Name {
		t.Fatalf("legacy top queueId: %q", got)
	}
	if got := parse(`{}`).queueName(); got != "" {
		t.Fatalf("empty: %q", got)
	}
}

/* ── 回归：近 20 场 + 对局类型口径（2026-09-22 迭代） ───────────── */

// mkSum 近况夹具（qid 区分队列；short 供断言行归属）
func mkSum(qid int, short string, win bool, k, d, a int) parser.MatchSummary {
	return parser.MatchSummary{
		GameID: int64(k*1000 + d*100 + a), QueueID: qid, QueueShort: short,
		Win: win, Kills: k, Deaths: d, Assists: a, ChampionID: 1,
	}
}

// mixedPages 3 页混合队列：page0=10 海斗+5 单双，page1=12 海斗+3 单双，page2=10 海斗（不应触及）
func mixedPages() [][]parser.MatchSummary {
	var p0, p1, p2 []parser.MatchSummary
	for i := 0; i < 10; i++ {
		p0 = append(p0, mkSum(2400, "海斗", i%2 == 0, 5, 2, 3))
	}
	for i := 0; i < 5; i++ {
		p0 = append(p0, mkSum(420, "单双", true, 1, 1, 1))
	}
	for i := 0; i < 12; i++ {
		p1 = append(p1, mkSum(2400, "海斗", true, 2, 2, 2))
	}
	for i := 0; i < 3; i++ {
		p1 = append(p1, mkSum(420, "单双", false, 0, 3, 0))
	}
	for i := 0; i < 10; i++ {
		p2 = append(p2, mkSum(2400, "海斗", true, 9, 0, 9))
	}
	return [][]parser.MatchSummary{p0, p1, p2}
}

func TestBuildSlots_CareerFilter20(t *testing.T) {
	cli := &fakeLCU{routes: map[string]string{phasePath: `"Lobby"`}}
	hist := &fakeHist{pages: map[string][][]parser.MatchSummary{"PA": mixedPages()}}
	svc := newTestService(cli, hist, &fakeLive{})
	refs := []playerRef{{puuid: "PA", summonerID: "1001", gameName: "甲"}}

	// 队列口径 [2400]：多页凑满 20 场海斗（page0 的 10 + page1 的 12 → 截断 20），单双场次全部滤掉
	a := svc.buildSlots(cli, refs, []int{2400})[0]
	if len(a.Recent) != 20 || a.WinRateSample != 20 {
		t.Fatalf("filtered recent=%d sample=%d（应满 20 场海斗）", len(a.Recent), a.WinRateSample)
	}
	for i, r := range a.Recent {
		if r.QueueShort != "海斗" {
			t.Fatalf("recent[%d] 混入 %q", i, r.QueueShort)
		}
	}

	// 空口径 = 全部：同为 20 场，但混合两队列且保持时间序（page0 在前）
	b := svc.buildSlots(cli, []playerRef{{puuid: "PA", summonerID: "1001", gameName: "甲"}}, nil)[0]
	if len(b.Recent) != 20 || b.WinRateSample != 20 {
		t.Fatalf("all recent=%d sample=%d", len(b.Recent), b.WinRateSample)
	}
	if b.Recent[0].QueueShort != "海斗" || b.Recent[14].QueueShort != "单双" || b.Recent[15].QueueShort != "海斗" {
		t.Fatalf("时间序/归属错误: %q %q %q", b.Recent[0].QueueShort, b.Recent[14].QueueShort, b.Recent[15].QueueShort)
	}
}

func TestGetGameflowState_FollowQueueFilter(t *testing.T) {
	// queueFilter=[-1]（跟随）→ 按当前对局队列（gameData.queue.id=2400）过滤近况
	sess := `{"gameData":{"queue":{"id":2400,"name":"海克斯大乱斗 ","numPlayersPerTeam":5},
		"teamOne":[{"puuid":"PA","summonerId":1001,"profileIconId":7,"championId":1}],
		"teamTwo":[{"puuid":"E1","summonerId":2001,"profileIconId":8,"championId":2}]}}`
	cli := &fakeLCU{routes: map[string]string{
		phasePath:               `"InProgress"`,
		lcu.PathGameflowSession: sess,
	}}
	hist := &fakeHist{
		pages: map[string][][]parser.MatchSummary{
			"PA": mixedPages(),
			"E1": {{mkSum(420, "单双", true, 3, 3, 3)}}, // 敌方只有单双 → 跟随海斗口径下应为空
		},
	}
	svc := newTestService(cli, hist, &fakeLive{err: errors.New("live down")})
	st, err := svc.GetGameflowState([]int{-1})
	if err != nil {
		t.Fatal(err)
	}
	if st.QueueID != 2400 {
		t.Fatalf("view queueId=%d want 2400", st.QueueID)
	}
	a := st.Teams[0].Slots[0]
	if len(a.Recent) != 20 {
		t.Fatalf("ally recent=%d want 20", len(a.Recent))
	}
	for _, r := range a.Recent {
		if r.QueueShort != "海斗" {
			t.Fatalf("跟随口径混入 %q", r.QueueShort)
		}
	}
	// 敌方仅单双场次：跟随海斗 → 0 场（非生涯隐藏，显示"暂无近战数据"）
	e := st.Teams[1].Slots[0]
	if len(e.Recent) != 0 || e.HiddenCareer {
		t.Fatalf("enemy recent=%d hidden=%v", len(e.Recent), e.HiddenCareer)
	}
}

/* ── PhaseLabelCN 全映射 ──────────────────────────── */

func TestPhaseLabelCN(t *testing.T) {
	cases := map[string]string{
		"":                "大厅中",
		"None":            "大厅中",
		"Lobby":           "房间内",
		"Matchmaking":     "匹配中",
		"ReadyCheck":      "接受对局",
		"ChampSelect":     "选人中",
		"GameStart":       "游戏启动",
		"InProgress":      "游戏中",
		"WaitingForStats": "结算中",
		"PreEndOfGame":    "结算中",
		"EndOfGame":       "对局结束",
		"Reconnect":       "重新连接",
		"Weird":           "大厅中",
	}
	for in, want := range cases {
		if got := PhaseLabelCN(in); got != want {
			t.Errorf("PhaseLabelCN(%q)=%q want %q", in, got, want)
		}
	}
}

/* ── 设置热更新：近况场数 / 并发三挡 ──────────────────────────── */

func TestSetCareerLimitAndConcurrency(t *testing.T) {
	svc := newTestService(&fakeLCU{}, &fakeHist{}, &fakeLive{})

	svc.SetCareerLimit(10)
	if svc.currentCareerLimit() != 10 {
		t.Fatalf("careerLimit = %d, want 10", svc.currentCareerLimit())
	}
	svc.SetCareerLimit(3) // 非法值不生效
	if svc.currentCareerLimit() != 10 {
		t.Fatalf("careerLimit after invalid set = %d", svc.currentCareerLimit())
	}

	for _, n := range []int{2, 5, 10} {
		svc.SetConcurrency(n)
		if svc.currentConcurrency() != n {
			t.Fatalf("concurrency = %d, want %d", svc.currentConcurrency(), n)
		}
	}
	svc.SetConcurrency(8) // 旧挡位忽略
	if svc.currentConcurrency() != 10 {
		t.Fatalf("concurrency after invalid set = %d", svc.currentConcurrency())
	}
}

func TestLoadCareerRespectsLimit(t *testing.T) {
	sums := make([]parser.MatchSummary, 30)
	for i := range sums {
		sums[i] = parser.MatchSummary{QueueID: 420, QueueShort: "排位", Win: true, ChampionID: 1}
	}
	h := &fakeHist{matches: map[string][]parser.MatchSummary{"P1": sums}}
	svc := newTestService(&fakeLCU{}, h, &fakeLive{})
	svc.SetCareerLimit(10)
	c := svc.loadCareer("P1", nil)
	if len(c.recent) != 10 {
		t.Fatalf("recent = %d, want 10", len(c.recent))
	}
	svc.SetCareerLimit(30)
	c = svc.loadCareer("P1", nil)
	if len(c.recent) != 30 {
		t.Fatalf("recent = %d, want 30", len(c.recent))
	}
}
