package parser

// SGP 战绩解析：match-history-query SUMMARY 响应 → 与 LCU 同构的 MatchSummary。
// SGP 形态（League Akari 契约，与 LCU match-history 不同）：
//
//	{ "games": [ { "metadata": {participants: [puuid…]}, "json": { game 字段 + 扁平 participants } } ] }
//
// 参赛者统计扁平内联（无 stats 子对象），符文经 perks.styles[0].selections[0].perk 取基石，
// 海克斯为 playerAugment1..6。这里做 SGP → lcuGame/lcuParticipant 适配后复用 buildSummary，
// 保证 KDA/物品/队列映射/时间格式与 LCU 通路完全一致。

import (
	"encoding/json"
	"fmt"
)

/* ─── SGP 原始 JSON 结构 ───────────────────────────────────────── */

// sgpPerks match-v5 风格符文树（styles[0]=主系，selections[0]=基石）
type sgpPerks struct {
	Styles []struct {
		Selections []struct {
			Perk int `json:"perk"`
		} `json:"selections"`
	} `json:"styles"`
}

type sgpParticipant struct {
	Puuid                       flexString `json:"puuid"`
	TeamID                      int        `json:"teamId"`
	ChampionID                  int        `json:"championId"`
	ChampLevel                  int        `json:"champLevel"`
	Spell1ID                    int        `json:"spell1Id"`
	Spell2ID                    int        `json:"spell2Id"`
	Kills                       int        `json:"kills"`
	Deaths                      int        `json:"deaths"`
	Assists                     int        `json:"assists"`
	Win                         bool       `json:"win"`
	Item0                       int        `json:"item0"`
	Item1                       int        `json:"item1"`
	Item2                       int        `json:"item2"`
	Item3                       int        `json:"item3"`
	Item4                       int        `json:"item4"`
	Item5                       int        `json:"item5"`
	Item6                       int        `json:"item6"`
	PlayerAugment1              int        `json:"playerAugment1"`
	PlayerAugment2              int        `json:"playerAugment2"`
	PlayerAugment3              int        `json:"playerAugment3"`
	PlayerAugment4              int        `json:"playerAugment4"`
	PlayerAugment5              int        `json:"playerAugment5"`
	PlayerAugment6              int        `json:"playerAugment6"`
	SubteamPlacement            int        `json:"subteamPlacement"`
	GameEndedInEarlySurrender   bool       `json:"gameEndedInEarlySurrender"`
	TeamEarlySurrendered        bool       `json:"teamEarlySurrendered"`
	TotalMinionsKilled          int        `json:"totalMinionsKilled"`
	NeutralMinionsKilled        int        `json:"neutralMinionsKilled"`
	GoldEarned                  int        `json:"goldEarned"`
	TotalDamageDealtToChampions int        `json:"totalDamageDealtToChampions"`
	TotalHeal                   int        `json:"totalHeal"`
	Perks                       sgpPerks   `json:"perks"`
}

// perk0 主系基石（styles[0].selections[0].perk）
func (p sgpParticipant) perk0() int {
	if len(p.Perks.Styles) > 0 && len(p.Perks.Styles[0].Selections) > 0 {
		return p.Perks.Styles[0].Selections[0].Perk
	}
	return 0
}

// augments 海克斯强化 ID（playerAugment1..6，零值由 augmentIDs 统一过滤）
func (p sgpParticipant) augments() []int {
	return []int{
		p.PlayerAugment1, p.PlayerAugment2, p.PlayerAugment3,
		p.PlayerAugment4, p.PlayerAugment5, p.PlayerAugment6,
	}
}

type sgpGameJSON struct {
	GameID       int64            `json:"gameId"`
	GameCreation int64            `json:"gameCreation"`
	GameDuration int              `json:"gameDuration"`
	QueueID      int              `json:"queueId"`
	MapID        int              `json:"mapId"`
	Participants []sgpParticipant `json:"participants"`
}

type sgpGameSummary struct {
	JSON sgpGameJSON `json:"json"`
}

/* ─── 解析入口 ─────────────────────────────────────────────────── */

// ParseSGPSummaries 解析 SGP match-history-query SUMMARY 战绩列表。
// selfPuuid 按 participants[].puuid 匹配本人参赛者，否则回退首个（查询对象恒在结果内）。
// SGP 响应无 gameCount 总数字段，分页边界由调用方按返回条数推断。
func ParseSGPSummaries(data []byte, selfPuuid string) ([]MatchSummary, error) {
	var wrap struct {
		Games []sgpGameSummary `json:"games"`
	}
	if err := json.Unmarshal(data, &wrap); err != nil {
		return nil, fmt.Errorf("解析战绩列表失败: %w", err)
	}

	summaries := make([]MatchSummary, 0, len(wrap.Games))
	for _, sg := range wrap.Games {
		if len(sg.JSON.Participants) == 0 {
			continue
		}
		g := adaptSGPGame(sg.JSON)
		p := pickParticipant(g, selfPuuid)
		summaries = append(summaries, buildSummary(g, p))
	}
	return summaries, nil
}

/* ─── SGP → LCU 形态适配（复用 buildSummary / pickParticipant） ── */

// adaptSGPGame SGP 单局 → lcuGame：参赛者统计压平映射进 lcuStats，
// puuid 写入 identities 使 pickParticipant 按 puuid 命中本人。
func adaptSGPGame(sj sgpGameJSON) lcuGame {
	g := lcuGame{
		GameID:       sj.GameID,
		GameCreation: sj.GameCreation,
		GameDuration: sj.GameDuration,
		QueueID:      sj.QueueID,
		MapID:        sj.MapID,
	}
	for i, sp := range sj.Participants {
		pid := i + 1
		lp := lcuParticipant{
			ParticipantID: pid,
			TeamID:        sp.TeamID,
			ChampionID:    sp.ChampionID,
			Spell1ID:      sp.Spell1ID,
			Spell2ID:      sp.Spell2ID,
			Stats: lcuStats{
				Win:                         sp.Win,
				Kills:                       sp.Kills,
				Deaths:                      sp.Deaths,
				Assists:                     sp.Assists,
				ChampLevel:                  sp.ChampLevel,
				Item0:                       sp.Item0,
				Item1:                       sp.Item1,
				Item2:                       sp.Item2,
				Item3:                       sp.Item3,
				Item4:                       sp.Item4,
				Item5:                       sp.Item5,
				Item6:                       sp.Item6,
				Perk0:                       sp.perk0(),
				TotalMinionsKilled:          sp.TotalMinionsKilled,
				NeutralMinionsKilled:        sp.NeutralMinionsKilled,
				GoldEarned:                  sp.GoldEarned,
				TotalDamageDealtToChampions: sp.TotalDamageDealtToChampions,
				TotalHeal:                   sp.TotalHeal,
				GameEndedInEarlySurrender:   sp.GameEndedInEarlySurrender || sp.TeamEarlySurrendered,
				SubteamPlacement:            sp.SubteamPlacement,
				Augments:                    sp.augments(),
			},
		}
		g.Participants = append(g.Participants, lp)
		ident := lcuIdentity{ParticipantID: pid}
		ident.Player.Puuid = sp.Puuid
		g.ParticipantIdentities = append(g.ParticipantIdentities, ident)
	}
	return g
}
