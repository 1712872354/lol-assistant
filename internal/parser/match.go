package parser

// 战绩数据解析层：LCU 战绩 JSON → 前端视图模型（纯函数，可单测）。
// JSON 字段契约对齐 Yuumi match_parser.rs（国服 LCU 实测结构）：
//   - 列表：/lol-match-history/v1/products/lol/{puuid}/matches → {games:{games:[...],gameCount}}
//   - 明细：/lol-match-history/v1/games/{gameId} → 单个 game 对象（含 participantIdentities）
// 输出结构全部使用 camelCase JSON 标签，与前端 lib/types.ts 一一对应。

import (
	"encoding/json"
	"fmt"
	"math"
	"sort"
	"strings"
	"time"
)

/* ─── 输出视图模型 ─────────────────────────────────────────────── */

// MatchSummary 战绩列表卡片数据（左侧列表项）
type MatchSummary struct {
	GameID       int64  `json:"gameId"`
	QueueID      int    `json:"queueId"`
	QueueName    string `json:"queueName"`
	QueueShort   string `json:"queueShort"`
	MapName      string `json:"mapName"`
	Arena        bool   `json:"arena"`
	GameCreation int64  `json:"gameCreation"` // 毫秒时间戳
	GameDuration int    `json:"gameDuration"` // 秒
	Time         string `json:"time"`         // "2026-09-21 20:24"
	ShortTime    string `json:"shortTime"`    // "09-21"
	Duration     string `json:"duration"`     // "15:24"
	ChampionID   int    `json:"championId"`
	ChampLevel   int    `json:"champLevel"`
	Spell1ID     int    `json:"spell1Id"`
	Spell2ID     int    `json:"spell2Id"`
	RuneID       int    `json:"runeId"` // perk0 主系基石
	Kills        int    `json:"kills"`
	Deaths       int    `json:"deaths"`
	Assists      int    `json:"assists"`
	KDA          string `json:"kda"` // "6.83" / "Perfect"
	Win          bool   `json:"win"`
	Remake       bool   `json:"remake"`
	Placement    int    `json:"placement"` // 竞技场名次（subteamPlacement），0=无
	Items        []int  `json:"items"`     // item0..item6（7 格，含饰品）
	CS           int    `json:"cs"`
	Gold         int    `json:"gold"`
	TotalDamage  int    `json:"totalDamage"`
	TotalHeal    int    `json:"totalHeal"`
	AugmentIDs   []int  `json:"augmentIds"` // 海克斯强化（海斗/竞技场）
	TeamID       int    `json:"teamId"`
}

// PlayerRow 明细页单个玩家行（右侧表格）
type PlayerRow struct {
	ParticipantID int     `json:"participantId"`
	TeamID        int     `json:"teamId"`
	Placement     int     `json:"placement"`
	Puuid         string  `json:"puuid"`
	SummonerID    string  `json:"summonerId"`
	Name          string  `json:"name"` // gameName#tagLine
	ProfileIconID int     `json:"profileIconId"`
	ChampionID    int     `json:"championId"`
	ChampLevel    int     `json:"champLevel"`
	Spell1ID      int     `json:"spell1Id"`
	Spell2ID      int     `json:"spell2Id"`
	RuneID        int     `json:"runeId"`
	Kills         int     `json:"kills"`
	Deaths        int     `json:"deaths"`
	Assists       int     `json:"assists"`
	KDA           string  `json:"kda"`
	Items         []int   `json:"items"`
	CS            int     `json:"cs"`
	Gold          int     `json:"gold"`
	TotalDamage   int     `json:"totalDamage"`
	TotalHeal     int     `json:"totalHeal"`
	Win           bool    `json:"win"`
	Remake        bool    `json:"remake"`
	AugmentIDs    []int   `json:"augmentIds"`
	TierShort     string  `json:"tierShort"`  // highestAchievedSeasonTier → 中文（"黄金"），空=未知
	DmgRatio      float64 `json:"dmgRatio"`   // 伤转：个人伤害 / 本组平均伤害
	Rating        float64 `json:"rating"`     // 本工具评分（公式见 Rating）
	RatingRank    int     `json:"ratingRank"` // 全部参与者内评分名次 1..N
	KillPct       int     `json:"killPct"`    // 参团率 %：(K+A)/本组总击杀×100
	IsSelf        bool    `json:"isSelf"`
}

// TeamSummary 队伍（竞技场为小队）汇总；Players 已按评分降序
type TeamSummary struct {
	TeamID    int         `json:"teamId"`
	Placement int         `json:"placement"`
	Win       bool        `json:"win"`
	Kills     int         `json:"kills"`
	Deaths    int         `json:"deaths"`
	Assists   int         `json:"assists"`
	Gold      int         `json:"gold"`
	Damage    int         `json:"damage"`
	Players   []PlayerRow `json:"players"`
}

// MatchDetail 对局明细（Teams[0] 恒为查询对象所在队伍）
type MatchDetail struct {
	GameID        int64         `json:"gameId"`
	QueueID       int           `json:"queueId"`
	QueueName     string        `json:"queueName"`
	MapName       string        `json:"mapName"`
	Arena         bool          `json:"arena"`
	GameCreation  int64         `json:"gameCreation"`
	Time          string        `json:"time"`
	GameDuration  int           `json:"gameDuration"`
	Duration      string        `json:"duration"`    // "15:24"
	DurationMin   string        `json:"durationMin"` // "15分"
	Remake        bool          `json:"remake"`
	SelfPuuid     string        `json:"selfPuuid"`
	SelfTeamIndex int           `json:"selfTeamIndex"` // 恒 0，保留字段便于前端对齐
	Teams         []TeamSummary `json:"teams"`
}

/* ─── LCU 原始 JSON 结构 ───────────────────────────────────────── */

// flexString 兼容 JSON 中 string / number 两种形态的字段（summonerId 历史上出现过数字）
type flexString string

func (f *flexString) UnmarshalJSON(b []byte) error {
	s := strings.Trim(string(b), `"`)
	if s == "null" || s == "" {
		*f = ""
		return nil
	}
	*f = flexString(s)
	return nil
}

type lcuIdentity struct {
	ParticipantID int `json:"participantId"`
	Player        struct {
		Puuid         flexString `json:"puuid"`
		SummonerID    flexString `json:"summonerId"`
		GameName      string     `json:"gameName"`
		TagLine       string     `json:"tagLine"`
		SummonerName  string     `json:"summonerName"`
		DisplayName   string     `json:"displayName"`
		ProfileIconID int        `json:"profileIcon"`
	} `json:"player"`
}

type lcuStats struct {
	Win                         bool   `json:"win"`
	Kills                       int    `json:"kills"`
	Deaths                      int    `json:"deaths"`
	Assists                     int    `json:"assists"`
	ChampLevel                  int    `json:"champLevel"`
	Item0                       int    `json:"item0"`
	Item1                       int    `json:"item1"`
	Item2                       int    `json:"item2"`
	Item3                       int    `json:"item3"`
	Item4                       int    `json:"item4"`
	Item5                       int    `json:"item5"`
	Item6                       int    `json:"item6"`
	Perk0                       int    `json:"perk0"`
	TotalMinionsKilled          int    `json:"totalMinionsKilled"`
	NeutralMinionsKilled        int    `json:"neutralMinionsKilled"`
	GoldEarned                  int    `json:"goldEarned"`
	TotalDamageDealtToChampions int    `json:"totalDamageDealtToChampions"`
	TotalHeal                   int    `json:"totalHeal"`
	GameEndedInEarlySurrender   bool   `json:"gameEndedInEarlySurrender"`
	TeamEarlySurrendered        bool   `json:"teamEarlySurrendered"`
	SubteamPlacement            int    `json:"subteamPlacement"`
	HighestAchievedSeasonTier   string `json:"highestAchievedSeasonTier"`
	Augments                    []int  `json:"augments"`
	PlayerAugment1              int    `json:"playerAugment1"`
	PlayerAugment2              int    `json:"playerAugment2"`
	PlayerAugment3              int    `json:"playerAugment3"`
	PlayerAugment4              int    `json:"playerAugment4"`
	PlayerAugment5              int    `json:"playerAugment5"`
}

type lcuParticipant struct {
	ParticipantID int      `json:"participantId"`
	TeamID        int      `json:"teamID"` // Riot 序列化历史上两种大小写并存
	TeamIDAlt     int      `json:"teamId"`
	ChampionID    int      `json:"championId"`
	Spell1ID      int      `json:"spell1Id"`
	Spell2ID      int      `json:"spell2Id"`
	Stats         lcuStats `json:"stats"`
}

func (p lcuParticipant) team() int {
	if p.TeamID != 0 {
		return p.TeamID
	}
	return p.TeamIDAlt
}

type lcuGame struct {
	GameID                    int64            `json:"gameId"`
	GameCreation              int64            `json:"gameCreation"`
	GameDuration              int              `json:"gameDuration"`
	QueueID                   int              `json:"queueId"`
	MapID                     int              `json:"mapId"`
	GameEndedInEarlySurrender bool             `json:"gameEndedInEarlySurrender"`
	Participants              []lcuParticipant `json:"participants"`
	ParticipantIdentities     []lcuIdentity    `json:"participantIdentities"`
}

/* ─── 解析入口 ─────────────────────────────────────────────────── */

// ParseMatchSummaries 解析战绩列表。selfPuuid 非空时按 identities 匹配本人参与者，
// 否则回退首个参与者（LCU 对指定 puuid 查询时本人恒在首位，兼容缺失 identities 的响应）。
func ParseMatchSummaries(data []byte, selfPuuid string) ([]MatchSummary, int, error) {
	var wrap struct {
		Games struct {
			Games     []json.RawMessage `json:"games"`
			GameCount int               `json:"gameCount"`
		} `json:"games"`
	}
	if err := json.Unmarshal(data, &wrap); err != nil {
		return nil, 0, fmt.Errorf("解析战绩列表失败: %w", err)
	}

	summaries := make([]MatchSummary, 0, len(wrap.Games.Games))
	for _, raw := range wrap.Games.Games {
		var g lcuGame
		if err := json.Unmarshal(raw, &g); err != nil || len(g.Participants) == 0 {
			continue
		}
		p := pickParticipant(g, selfPuuid)
		summaries = append(summaries, buildSummary(g, p))
	}

	gameCount := wrap.Games.GameCount
	if gameCount <= 0 {
		gameCount = len(summaries)
	}
	return summaries, gameCount, nil
}

// ParseMatchDetail 解析单局明细：分组（普通=teamId / 竞技场=subteamPlacement）、
// 计算伤转与评分排名，本人所在队伍置于 Teams[0]。
func ParseMatchDetail(data []byte, selfPuuid string) (*MatchDetail, error) {
	var g lcuGame
	if err := json.Unmarshal(data, &g); err != nil {
		return nil, fmt.Errorf("解析对局明细失败: %w", err)
	}
	if len(g.Participants) == 0 {
		return nil, fmt.Errorf("对局明细无参与者数据")
	}

	qi := QueueInfoFor(g.QueueID)

	// identities → 参与者信息表
	idByID := make(map[int]lcuIdentity, len(g.ParticipantIdentities))
	for _, ident := range g.ParticipantIdentities {
		idByID[ident.ParticipantID] = ident
	}

	// 构建全部行
	remake := g.GameEndedInEarlySurrender
	rows := make([]PlayerRow, 0, len(g.Participants))
	selfKey := ""
	for _, p := range g.Participants {
		s := p.Stats
		if s.GameEndedInEarlySurrender || s.TeamEarlySurrendered {
			remake = true
		}
		ident, hasIdent := idByID[p.ParticipantID]
		row := PlayerRow{
			ParticipantID: p.ParticipantID,
			TeamID:        p.team(),
			Placement:     s.SubteamPlacement,
			ChampionID:    p.ChampionID,
			ChampLevel:    s.ChampLevel,
			Spell1ID:      p.Spell1ID,
			Spell2ID:      p.Spell2ID,
			RuneID:        s.Perk0,
			Kills:         s.Kills,
			Deaths:        s.Deaths,
			Assists:       s.Assists,
			KDA:           KDAString(s.Kills, s.Deaths, s.Assists),
			Items:         []int{s.Item0, s.Item1, s.Item2, s.Item3, s.Item4, s.Item5, s.Item6},
			CS:            s.TotalMinionsKilled + s.NeutralMinionsKilled,
			Gold:          s.GoldEarned,
			TotalDamage:   s.TotalDamageDealtToChampions,
			TotalHeal:     s.TotalHeal,
			Win:           s.Win,
			Remake:        s.GameEndedInEarlySurrender || s.TeamEarlySurrendered,
			AugmentIDs:    augmentIDs(s),
			TierShort:     TierCN(s.HighestAchievedSeasonTier),
			Rating:        Rating(s.Kills, s.Deaths, s.Assists, s.TotalDamageDealtToChampions, s.GoldEarned, s.Win),
		}
		if hasIdent {
			row.Puuid = string(ident.Player.Puuid)
			row.SummonerID = string(ident.Player.SummonerID)
			row.ProfileIconID = ident.Player.ProfileIconID
			row.Name = identityName(ident, p.ParticipantID)
		} else {
			row.Name = fmt.Sprintf("玩家 %d", p.ParticipantID)
		}
		if selfPuuid != "" && strings.EqualFold(row.Puuid, selfPuuid) {
			row.IsSelf = true
			selfKey = groupKey(qi, row.TeamID, row.Placement)
		}
		rows = append(rows, row)
	}

	// 伤转：个人伤害 / 本组平均伤害；参团率：(K+A) / 本组总击杀
	sums := map[string]struct{ dmg, kills, n int }{}
	for _, r := range rows {
		k := groupKey(qi, r.TeamID, r.Placement)
		st := sums[k]
		st.dmg += r.TotalDamage
		st.kills += r.Kills
		st.n++
		sums[k] = st
	}
	for i := range rows {
		st := sums[groupKey(qi, rows[i].TeamID, rows[i].Placement)]
		avg := float64(st.dmg) / math.Max(float64(st.n), 1)
		if avg > 0 {
			rows[i].DmgRatio = math.Round(float64(rows[i].TotalDamage)/avg*10) / 10
		}
		if st.kills > 0 {
			rows[i].KillPct = int(math.Round(float64(rows[i].Kills+rows[i].Assists) / float64(st.kills) * 100))
		}
	}

	// 评分全局名次：评分降序，同分按伤害降序
	rankOrder := make([]int, len(rows))
	for i := range rankOrder {
		rankOrder[i] = i
	}
	sort.SliceStable(rankOrder, func(a, b int) bool {
		if rows[rankOrder[a]].Rating != rows[rankOrder[b]].Rating {
			return rows[rankOrder[a]].Rating > rows[rankOrder[b]].Rating
		}
		return rows[rankOrder[a]].TotalDamage > rows[rankOrder[b]].TotalDamage
	})
	for rank, idx := range rankOrder {
		rows[idx].RatingRank = rank + 1
	}

	// 分组：同 key 归入一个 TeamSummary
	type bucket struct {
		teamID, placement int
		win               bool
		rows              []PlayerRow
	}
	var order []string
	buckets := map[string]*bucket{}
	for _, r := range rows {
		k := groupKey(qi, r.TeamID, r.Placement)
		b, ok := buckets[k]
		if !ok {
			b = &bucket{teamID: r.TeamID, placement: r.Placement}
			buckets[k] = b
			order = append(order, k)
		}
		b.win = b.win || r.Win
		b.rows = append(b.rows, r)
	}
	if selfKey == "" && len(rows) > 0 {
		selfKey = groupKey(qi, rows[0].TeamID, rows[0].Placement)
	}
	// 竞技场：第 1 名小队视为胜利方
	for _, b := range buckets {
		if qi.Arena && b.placement == 1 {
			b.win = true
		}
	}

	// 排序：本人队伍优先；其余按 placement→teamID 升序
	sort.SliceStable(order, func(i, j int) bool {
		a, b := order[i], order[j]
		if a == selfKey {
			return true
		}
		if b == selfKey {
			return false
		}
		ba, bb := buckets[a], buckets[b]
		if ba.placement != bb.placement {
			return ba.placement < bb.placement
		}
		return ba.teamID < bb.teamID
	})

	teams := make([]TeamSummary, 0, len(order))
	for _, k := range order {
		b := buckets[k]
		sort.SliceStable(b.rows, func(i, j int) bool { return b.rows[i].Rating > b.rows[j].Rating })
		ts := TeamSummary{TeamID: b.teamID, Placement: b.placement, Win: b.win}
		for _, r := range b.rows {
			ts.Kills += r.Kills
			ts.Deaths += r.Deaths
			ts.Assists += r.Assists
			ts.Gold += r.Gold
			ts.Damage += r.TotalDamage
		}
		ts.Players = b.rows
		teams = append(teams, ts)
	}

	return &MatchDetail{
		GameID:        g.GameID,
		QueueID:       g.QueueID,
		QueueName:     qi.Name,
		MapName:       qi.Map,
		Arena:         qi.Arena,
		GameCreation:  g.GameCreation,
		Time:          FormatTime(g.GameCreation),
		GameDuration:  g.GameDuration,
		Duration:      FormatDuration(g.GameDuration),
		DurationMin:   fmt.Sprintf("%d分", g.GameDuration/60),
		Remake:        remake,
		SelfPuuid:     selfPuuid,
		SelfTeamIndex: 0,
		Teams:         teams,
	}, nil
}

/* ─── 纯函数工具 ───────────────────────────────────────────────── */

// Rating 本工具评分（透明公式，UI 有说明）：
//
//	kdaPart = (K+A)/max(D,1)，封顶 10（防 Perfect 场次爆分）
//	rating  = kdaPart*1.1 + 伤害/10000*1.2 + 金钱/10000*0.6 + 胜利加成 1.5
//
// 保留一位小数。数值区间与参考截图量级一致（约 3~16）。
func Rating(kills, deaths, assists, damage, gold int, win bool) float64 {
	kdaPart := float64(kills+assists) / math.Max(float64(deaths), 1)
	if kdaPart > 10 {
		kdaPart = 10
	}
	r := kdaPart*1.1 +
		float64(damage)/10000*1.2 +
		float64(gold)/10000*0.6
	if win {
		r += 1.5
	}
	return math.Round(r*10) / 10
}

// KDAString KDA 展示：零死亡 → "Perfect"，否则保留两位小数
func KDAString(kills, deaths, assists int) string {
	if deaths == 0 {
		return "Perfect"
	}
	return fmt.Sprintf("%.2f", float64(kills+assists)/float64(deaths))
}

// TierCN 段位英文 → 中文；未知值原样返回；未定级返回空串
func TierCN(tier string) string {
	switch strings.ToUpper(strings.TrimSpace(tier)) {
	case "":
		return ""
	case "UNRANKED", "NONE":
		return ""
	case "IRON":
		return "黑铁"
	case "BRONZE":
		return "青铜"
	case "SILVER":
		return "白银"
	case "GOLD":
		return "黄金"
	case "PLATINUM":
		return "白金"
	case "EMERALD":
		return "翡翠"
	case "DIAMOND":
		return "钻石"
	case "MASTER":
		return "大师"
	case "GRANDMASTER":
		return "宗师"
	case "CHALLENGER":
		return "王者"
	default:
		return strings.ToUpper(strings.TrimSpace(tier))
	}
}

// FormatTime 毫秒时间戳 → 本机时区 "2006/1/2 15:04:05"（对齐参考布局底部时间）
func FormatTime(ms int64) string {
	if ms <= 0 {
		return ""
	}
	return time.UnixMilli(ms).Format("2006/1/2 15:04:05")
}

// FormatShortTime 毫秒时间戳 → "01-02 15:04"（列表卡片）
func FormatShortTime(ms int64) string {
	if ms <= 0 {
		return ""
	}
	return time.UnixMilli(ms).Format("01-02 15:04")
}

// FormatDuration 秒 → "15:24"（不进位小时，对齐 Yuumi 语义）
func FormatDuration(secs int) string {
	if secs < 0 {
		secs = 0
	}
	return fmt.Sprintf("%02d:%02d", secs/60, secs%60)
}

/* ─── 内部辅助 ─────────────────────────────────────────────────── */

// pickParticipant 选择查询对象的参与者：identities 按 puuid 匹配，否则取首个
func pickParticipant(g lcuGame, selfPuuid string) lcuParticipant {
	if selfPuuid != "" && len(g.ParticipantIdentities) > 0 {
		idByID := make(map[int]lcuIdentity, len(g.ParticipantIdentities))
		for _, ident := range g.ParticipantIdentities {
			idByID[ident.ParticipantID] = ident
		}
		for _, p := range g.Participants {
			if ident, ok := idByID[p.ParticipantID]; ok &&
				strings.EqualFold(string(ident.Player.Puuid), selfPuuid) {
				return p
			}
		}
	}
	return g.Participants[0]
}

// buildSummary 单个 game + 目标参与者 → 列表卡片数据
func buildSummary(g lcuGame, p lcuParticipant) MatchSummary {
	s := p.Stats
	qi := QueueInfoFor(g.QueueID)
	return MatchSummary{
		GameID:       g.GameID,
		QueueID:      g.QueueID,
		QueueName:    qi.Name,
		QueueShort:   qi.Short,
		MapName:      qi.Map,
		Arena:        qi.Arena,
		GameCreation: g.GameCreation,
		GameDuration: g.GameDuration,
		Time:         FormatTime(g.GameCreation),
		ShortTime:    FormatShortTime(g.GameCreation),
		Duration:     FormatDuration(g.GameDuration),
		ChampionID:   p.ChampionID,
		ChampLevel:   s.ChampLevel,
		Spell1ID:     p.Spell1ID,
		Spell2ID:     p.Spell2ID,
		RuneID:       s.Perk0,
		Kills:        s.Kills,
		Deaths:       s.Deaths,
		Assists:      s.Assists,
		KDA:          KDAString(s.Kills, s.Deaths, s.Assists),
		Win:          s.Win,
		Remake:       g.GameEndedInEarlySurrender || s.GameEndedInEarlySurrender,
		Placement:    s.SubteamPlacement,
		Items:        []int{s.Item0, s.Item1, s.Item2, s.Item3, s.Item4, s.Item5, s.Item6},
		CS:           s.TotalMinionsKilled + s.NeutralMinionsKilled,
		Gold:         s.GoldEarned,
		TotalDamage:  s.TotalDamageDealtToChampions,
		TotalHeal:    s.TotalHeal,
		AugmentIDs:   augmentIDs(s),
		TeamID:       p.team(),
	}
}

// augmentIDs 海克斯强化 ID：augments 数组 + playerAugment1..5 合并去重，最多 5 个
func augmentIDs(s lcuStats) []int {
	seen := map[int]bool{}
	out := make([]int, 0, 5)
	push := func(id int) {
		if id != 0 && !seen[id] && len(out) < 5 {
			seen[id] = true
			out = append(out, id)
		}
	}
	for _, id := range s.Augments {
		push(id)
	}
	for _, id := range []int{s.PlayerAugment1, s.PlayerAugment2, s.PlayerAugment3, s.PlayerAugment4, s.PlayerAugment5} {
		push(id)
	}
	if len(out) == 0 {
		return []int{}
	}
	return out
}

// identityName 玩家展示名：gameName#tagLine → summonerName#tagLine → displayName → 玩家N
func identityName(ident lcuIdentity, participantID int) string {
	p := ident.Player
	name := p.GameName
	if name == "" {
		name = p.SummonerName
	}
	if name == "" {
		name = p.DisplayName
	}
	if name == "" {
		return fmt.Sprintf("玩家 %d", participantID)
	}
	if tag := p.TagLine; tag != "" {
		return name + "#" + tag
	}
	return name
}

// groupKey 分组键：竞技场（placement>0）按小队，否则按 teamId
func groupKey(qi QueueInfo, teamID, placement int) string {
	if qi.Arena && placement > 0 {
		return fmt.Sprintf("p%d", placement)
	}
	return fmt.Sprintf("t%d", teamID)
}
