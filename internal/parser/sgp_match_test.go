package parser

import "testing"

// sgpSummaryFixture SGP match-history-query SUMMARY 形态夹具（Akari 契约）：
// {games:[{metadata, json:{…, participants:[扁平统计]}}]}，本人排第二验证按 puuid 选人。
const sgpSummaryFixture = `{"games":[
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
]}`

func TestParseSGPSummaries(t *testing.T) {
	const self = "5e65c58d-5b4a-5936-9104-806bb8443eef"
	sums, err := ParseSGPSummaries([]byte(sgpSummaryFixture), self)
	if err != nil {
		t.Fatal(err)
	}
	if len(sums) != 2 {
		t.Fatalf("summaries = %d", len(sums))
	}

	// ── 第 1 局（排位 420）：按 puuid 选中本人（第二位），非首位 ──
	s := sums[0]
	if s.GameID != 900001 || s.QueueID != 420 {
		t.Fatalf("game = %+v", s)
	}
	if s.ChampionID != 22 || s.Kills != 6 || s.Deaths != 1 || s.Assists != 8 {
		t.Fatalf("本人参赛者未命中: %+v", s)
	}
	if s.Spell1ID != 7 || s.Spell2ID != 6 {
		t.Fatalf("spells = %d/%d", s.Spell1ID, s.Spell2ID)
	}
	if s.RuneID != 8112 {
		t.Fatalf("rune (perks 基石) = %d", s.RuneID)
	}
	if s.KDA != "14.00" || !s.Win {
		t.Fatalf("kda/win = %q %v", s.KDA, s.Win)
	}
	if len(s.Items) != 7 || s.Items[0] != 10 || s.Items[6] != 3340 {
		t.Fatalf("items = %v", s.Items)
	}
	if s.CS != 220 || s.Gold != 14200 || s.TotalDamage != 18500 {
		t.Fatalf("cs/gold/dmg = %d/%d/%d", s.CS, s.Gold, s.TotalDamage)
	}
	if s.QueueName == "" || s.Arena {
		t.Fatalf("queue = %+v", s)
	}

	// ── 第 2 局（竞技场 1700）：placement + 海克斯 + 重赛标记 ──
	a := sums[1]
	if a.GameID != 900002 || !a.Arena || a.Placement != 2 {
		t.Fatalf("arena = %+v", a)
	}
	if len(a.AugmentIDs) != 2 || a.AugmentIDs[0] != 111 || a.AugmentIDs[1] != 222 {
		t.Fatalf("augments = %v", a.AugmentIDs)
	}
	if !a.Remake {
		t.Fatalf("teamEarlySurrendered 应判重赛: %+v", a)
	}
}

func TestParseSGPSummaries_FallbackFirstParticipant(t *testing.T) {
	// selfPuuid 不在对局内 → 回退首位参与者（与 LCU 通路语义一致）
	sums, err := ParseSGPSummaries([]byte(sgpSummaryFixture), "nobody")
	if err != nil {
		t.Fatal(err)
	}
	if len(sums) != 2 || sums[0].ChampionID != 103 {
		t.Fatalf("fallback = %+v", sums)
	}
}

func TestParseSGPSummaries_EmptyAndBad(t *testing.T) {
	if sums, err := ParseSGPSummaries([]byte(`{"games":[]}`), "x"); err != nil || len(sums) != 0 {
		t.Fatalf("empty games: %v %v", sums, err)
	}
	if _, err := ParseSGPSummaries([]byte(`{bad json`), "x"); err == nil {
		t.Fatal("bad json should error")
	}
}
