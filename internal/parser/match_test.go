package parser

import (
	"math"
	"testing"
	"time"
)

/* ─── 队列映射 ─────────────────────────────────────────────────── */

func TestQueueInfoFor_CNQueues(t *testing.T) {
	cases := []struct {
		id         int
		name, want string
		arena      bool
	}{
		{2400, "海克斯大乱斗", "海斗", false},
		{2450, "经典海斗", "海斗", false},
		{420, "排位单双排", "单双", false},
		{440, "排位灵活组排", "灵活", false},
		{450, "极地大乱斗", "大乱斗", false},
		{1700, "斗魂竞技场", "竞技场", true},
		{1710, "斗魂竞技场", "竞技场", true},
	}
	for _, c := range cases {
		qi := QueueInfoFor(c.id)
		if qi.Name != c.name || qi.Short != c.want || qi.Arena != c.arena {
			t.Errorf("QueueInfoFor(%d) = {%s %s arena=%v}, want {%s %s arena=%v}",
				c.id, qi.Name, qi.Short, qi.Arena, c.name, c.want, c.arena)
		}
	}
}

func TestQueueInfoFor_UnknownFallback(t *testing.T) {
	qi := QueueInfoFor(999999)
	if qi.ID != 999999 || qi.Short != "999999" {
		t.Fatalf("unknown fallback = %+v", qi)
	}
}

/* ─── 纯函数 ───────────────────────────────────────────────────── */

func TestRating_ZeroDeathCapped(t *testing.T) {
	// kdaPart 封顶 10：30/0/30 不应爆分
	got := Rating(30, 0, 30, 50000, 15000, true)
	want := math.Round((10*1.1+50000.0/10000*1.2+15000.0/10000*0.6+1.5)*10) / 10
	if got != want {
		t.Fatalf("Rating(30,0,30,...) = %v, want %v", got, want)
	}
}

func TestRating_WinBonusExactly1_5(t *testing.T) {
	win := Rating(5, 5, 5, 20000, 10000, true)
	lose := Rating(5, 5, 5, 20000, 10000, false)
	if math.Abs((win-lose)-1.5) > 1e-9 {
		t.Fatalf("win bonus = %v, want 1.5", win-lose)
	}
}

func TestKDAString(t *testing.T) {
	if KDAString(10, 2, 8) != "9.00" {
		t.Fatalf("KDAString normal = %s", KDAString(10, 2, 8))
	}
	if KDAString(4, 0, 12) != "Perfect" {
		t.Fatalf("KDAString perfect = %s", KDAString(4, 0, 12))
	}
}

func TestTierCN(t *testing.T) {
	cases := map[string]string{"GOLD": "黄金", "gold": "黄金", "IRON": "黑铁",
		"CHALLENGER": "王者", "": "", "UNRANKED": "", "WEIRD": "WEIRD"}
	for in, want := range cases {
		if got := TierCN(in); got != want {
			t.Errorf("TierCN(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestFormatters(t *testing.T) {
	if FormatDuration(1530) != "25:30" || FormatDuration(3725) != "62:05" || FormatDuration(-1) != "00:00" {
		t.Fatalf("FormatDuration mismatch")
	}
	const ms = int64(1705329000000)
	if FormatTime(ms) != time.UnixMilli(ms).Format("2006/1/2 15:04:05") {
		t.Fatalf("FormatTime = %s", FormatTime(ms))
	}
	if FormatShortTime(ms) != time.UnixMilli(ms).Format("01-02 15:04") || FormatTime(0) != "" {
		t.Fatalf("time formatters mismatch: %s / %q", FormatShortTime(ms), FormatTime(0))
	}
}

/* ─── 战绩列表解析 ─────────────────────────────────────────────── */

// historyFixture 两个对局：①海斗（identities 中本人排第二，验证按 puuid 匹配 + 海克斯合并去重）
// ②竞技场重赛（无 identities，验证首参与者回退 + remake 判定）
const historyFixture = `{"games":{"games":[
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
  "gameCount":45}}`

func TestParseMatchSummaries_SelfByPuuidAndFallback(t *testing.T) {
	sums, count, err := ParseMatchSummaries([]byte(historyFixture), "PSELF")
	if err != nil {
		t.Fatal(err)
	}
	if count != 45 || len(sums) != 2 {
		t.Fatalf("count=%d len=%d, want 45/2", count, len(sums))
	}

	s := sums[0]
	if s.ChampionID != 53 {
		t.Fatalf("self participant not matched by puuid: championId=%d", s.ChampionID)
	}
	if s.QueueShort != "海斗" || s.QueueName != "海克斯大乱斗" {
		t.Fatalf("queue mapping = %s/%s", s.QueueShort, s.QueueName)
	}
	if s.KDA != "2.18" || s.Win || s.Remake {
		t.Fatalf("stats mismatch: kda=%s win=%v remake=%v", s.KDA, s.Win, s.Remake)
	}
	if s.Duration != "15:24" || s.ShortTime != time.UnixMilli(1705329000000).Format("01-02 15:04") {
		t.Fatalf("time mismatch: %s / %s", s.Duration, s.ShortTime)
	}
	if s.Items[0] != 3157 || s.Items[6] != 3340 || s.CS != 7 || s.Gold != 11767 {
		t.Fatalf("items/cs/gold mismatch: %+v", s.Items)
	}
	if len(s.AugmentIDs) != 2 || s.AugmentIDs[0] != 7010 || s.AugmentIDs[1] != 7018 {
		t.Fatalf("augment dedupe failed: %v", s.AugmentIDs)
	}

	// 无 identities 的重赛场：回退首位参与者 + remake + 名次
	r := sums[1]
	if !r.Remake || r.Placement != 5 || r.KDA != "Perfect" || !r.Arena {
		t.Fatalf("arena remake row mismatch: %+v", r)
	}

	// 空 puuid → 回退首位参与者
	firsts, _, err := ParseMatchSummaries([]byte(historyFixture), "")
	if err != nil {
		t.Fatal(err)
	}
	if firsts[0].ChampionID != 22 {
		t.Fatalf("fallback should pick first participant, got %d", firsts[0].ChampionID)
	}
}

/* ─── 对局明细解析 ─────────────────────────────────────────────── */

// detailFixture 经典 4 人局（逻辑等价 10 人）：本人在败方 teamId=200
const detailFixture = `{"gameId":8000000001,"gameCreation":1705329000000,"gameDuration":924,
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
              "goldEarned":10000,"totalDamageDealtToChampions":10000}}]}`

func TestParseMatchDetail_ClassicTeams(t *testing.T) {
	d, err := ParseMatchDetail([]byte(detailFixture), "PSELF")
	if err != nil {
		t.Fatal(err)
	}
	if d.GameID != 8000000001 || d.QueueName != "排位单双排" || d.Arena {
		t.Fatalf("detail header mismatch: %+v", d)
	}
	if d.Duration != "15:24" || d.DurationMin != "15分" || d.Remake {
		t.Fatalf("duration/remake mismatch: %s %s remake=%v", d.Duration, d.DurationMin, d.Remake)
	}
	if len(d.Teams) != 2 {
		t.Fatalf("teams = %d, want 2", len(d.Teams))
	}

	selfTeam := d.Teams[0]
	if selfTeam.TeamID != 200 || selfTeam.Win {
		t.Fatalf("self team should be losing teamId=200: %+v", selfTeam)
	}
	if selfTeam.Kills != 10 || selfTeam.Damage != 40000 || selfTeam.Gold != 21767 {
		t.Fatalf("self team sums mismatch: %+v", selfTeam)
	}
	if len(selfTeam.Players) != 2 {
		t.Fatalf("self team players = %d", len(selfTeam.Players))
	}

	self := selfTeam.Players[0]
	if !self.IsSelf || self.Name != "歪比#60021" || self.SummonerID != "3333" {
		t.Fatalf("self row mismatch: %+v", self)
	}
	// 队内按评分降序：本人 6.7 > 队友 4.4
	if selfTeam.Players[0].Rating <= selfTeam.Players[1].Rating {
		t.Fatalf("team players not sorted by rating desc")
	}
	// 伤转：本组均伤 20000 → 1.5 / 0.5
	if self.DmgRatio != 1.5 || selfTeam.Players[1].DmgRatio != 0.5 {
		t.Fatalf("dmgRatio mismatch: %v / %v", self.DmgRatio, selfTeam.Players[1].DmgRatio)
	}
	// 队伍汇总 D/A：本方 11+9=20 / 20+15=35
	if selfTeam.Deaths != 20 || selfTeam.Assists != 35 {
		t.Fatalf("self team D/A mismatch: %d/%d", selfTeam.Deaths, selfTeam.Assists)
	}
	// 参团率：本组总击杀 4+6=10 → 本人 (4+20)/10=240%，队友 (6+15)/10=210%（fixture 高助攻，校验公式）
	if self.KillPct != 240 || selfTeam.Players[1].KillPct != 210 {
		t.Fatalf("killPct mismatch: %d / %d", self.KillPct, selfTeam.Players[1].KillPct)
	}

	// 全局名次为 1..N 排列；胜方 MVP 名次第 1
	rankSet := map[int]bool{}
	for _, team := range d.Teams {
		for _, p := range team.Players {
			rankSet[p.RatingRank] = true
		}
	}
	if len(rankSet) != 4 || !rankSet[1] || !rankSet[4] {
		t.Fatalf("rating ranks not a 1..N permutation: %v", rankSet)
	}
	mvp := d.Teams[1].Players[0]
	if mvp.RatingRank != 1 || mvp.TierShort != "黄金" {
		t.Fatalf("winner MVP mismatch: rank=%d tier=%s", mvp.RatingRank, mvp.TierShort)
	}
	if d.Teams[1].TeamID != 100 || !d.Teams[1].Win {
		t.Fatalf("other team mismatch: %+v", d.Teams[1])
	}
}

// detailArenaFixture 竞技场：同 teamId 不同 subteamPlacement → 按小队分组，本人小队置顶
const detailArenaFixture = `{"gameId":9000000001,"gameCreation":1705329000000,"gameDuration":780,
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
    {"participantId":4,"teamID":100,"championId":4,"stats":{"win":true,"kills":7,"deaths":3,"assists":7,"subteamPlacement":1,"goldEarned":12500,"totalDamageDealtToChampions":20000}}]}`

func TestParseMatchDetail_ArenaSubteams(t *testing.T) {
	d, err := ParseMatchDetail([]byte(detailArenaFixture), "PSELF")
	if err != nil {
		t.Fatal(err)
	}
	if !d.Arena || d.QueueName != "斗魂竞技场" {
		t.Fatalf("arena flag/name mismatch: %+v", d)
	}
	if len(d.Teams) != 2 {
		t.Fatalf("arena teams = %d, want 2 subteams", len(d.Teams))
	}
	if d.Teams[0].Placement != 2 || d.Teams[0].Win {
		t.Fatalf("self subteam should be placement 2 / lose: %+v", d.Teams[0])
	}
	if d.Teams[1].Placement != 1 || !d.Teams[1].Win {
		t.Fatalf("placement-1 subteam should win: %+v", d.Teams[1])
	}
	// 队内按评分降序（本人评分低于队友时不强制置首）；本人行必须存在于本方小队
	// 伤转：本组均伤 13000 → 队友 14000/13000≈1.1，本人 12000/13000≈0.9
	var selfFound bool
	for _, p := range d.Teams[0].Players {
		if p.IsSelf {
			selfFound = true
			if p.DmgRatio != 0.9 {
				t.Fatalf("self dmgRatio = %v, want 0.9", p.DmgRatio)
			}
			// 参团率：本组总击杀 2+3=5 → (2+3)/5=100%
			if p.KillPct != 100 {
				t.Fatalf("self killPct = %d, want 100", p.KillPct)
			}
		} else {
			if p.DmgRatio != 1.1 {
				t.Fatalf("mate dmgRatio = %v, want 1.1", p.DmgRatio)
			}
			// 参团率：(3+4)/5=140%
			if p.KillPct != 140 {
				t.Fatalf("mate killPct = %d, want 140", p.KillPct)
			}
		}
	}
	if !selfFound {
		t.Fatal("self row missing in own subteam")
	}
}

func TestParseMatchDetail_InvalidInput(t *testing.T) {
	if _, err := ParseMatchDetail([]byte(`{}`), ""); err == nil {
		t.Fatal("empty participants should error")
	}
	if _, err := ParseMatchDetail([]byte(`not-json`), ""); err == nil {
		t.Fatal("invalid json should error")
	}
}
