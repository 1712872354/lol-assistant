// Package gameinfo 对局信息页数据聚合（M3）：
//
//	按 gameflow 阶段选择数据源——
//	  Lobby/Matchmaking/ReadyCheck → /lol-lobby/v2/lobby 房间成员（自定义房按 team 1/2 分队）
//	  ChampSelect                  → /lol-champ-select/v1/session 双队（盲选敌方无 id → 空槽）
//	                                  + gameflow 花名册补名/头像（best-effort）
//	  GameStart~Reconnect          → /lol-gameflow/v1/session gameData.teamOne/teamTwo 花名册（确定性双队）
//	                                  + playerChampionSelections 补洞（teamTwo 实测漏第 5 人）
//	                                  Live Client :2999 playerlist 兜底（花名册不全时）
//	  None                         → 空视图（前端空态提示）
//	统一补数（复用 history 服务，fail-soft）：
//	  标识互查（by-puuid → by-summonerId → 按名搜索 三级回退）
//	  + 段位 GetPlayersRanked 批量 + 每人近 20 场 GetMatches（按对局类型口径过滤）→ 胜率/KDA/评分/生涯隐藏。
//
// 所有外部依赖均以接口注入，单元测试用 fake 全链覆盖。
package gameinfo

import (
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"net/url"
	"strings"
	"sync"
	"time"

	"github.com/1712872354/lol-assistant/internal/lcu"
	"github.com/1712872354/lol-assistant/internal/liveclient"
	"github.com/1712872354/lol-assistant/internal/parser"
	"github.com/1712872354/lol-assistant/service/history"
)

// ── 依赖面（*lcu.Client / *history.Service / *liveclient.Client 各自实现） ──

type lcuAPI interface {
	Get(path string) (int, []byte, error)
}

type histAPI interface {
	GetPlayersRanked(ids []string) ([]history.RankedInfo, error)
	GetMatches(puuid string, page int) (history.MatchPage, error)
	SearchSummoner(name string) (history.SummonerResult, error)
}

type liveAPI interface {
	PlayerList() ([]liveclient.Player, error)
	ActivePlayerName() (string, error)
}

// ── 输出视图模型（JSON 小写驼峰，与前端 lib/types.ts Gameinfo* 对齐） ──

// RecentMatch 近期对局摘要（卡片底部列表）
type RecentMatch struct {
	QueueShort   string `json:"queueShort"`   // 队列短名（行内药丸）
	QueueName    string `json:"queueName"`    // 队列全称（行内主文案）
	TimeShort    string `json:"timeShort"`    // 绝对短时间（相对时间兜底）
	GameCreation int64  `json:"gameCreation"` // ms 时间戳，前端算「X 小时前」
	Win          bool   `json:"win"`
	Kills        int    `json:"kills"`
	Deaths       int    `json:"deaths"`
	Assists      int    `json:"assists"`
	ChampionID   int    `json:"championId"`
}

// PlayerSlot 玩家卡槽（恒有 5 槽/队，filled=false 为骨架空槽）
type PlayerSlot struct {
	Filled        bool          `json:"filled"`
	IsSelf        bool          `json:"isSelf"`
	Puuid         string        `json:"puuid,omitempty"`
	SummonerID    string        `json:"summonerId,omitempty"`
	GameName      string        `json:"gameName,omitempty"`
	TagLine       string        `json:"tagLine,omitempty"`
	ProfileIconID int           `json:"profileIconId,omitempty"`
	ChampionID    int           `json:"championId,omitempty"`
	Solo          string        `json:"solo,omitempty"`
	Flex          string        `json:"flex,omitempty"`
	WinRate       float64       `json:"winRate,omitempty"`
	WinRateSample int           `json:"winRateSample,omitempty"`
	AvgKda        float64       `json:"avgKda,omitempty"`
	Rating        float64       `json:"rating,omitempty"`
	HiddenCareer  bool          `json:"hiddenCareer,omitempty"`
	Recent        []RecentMatch `json:"recent,omitempty"`
}

// TeamView 队伍区块（slots 恒 5）
type TeamView struct {
	Key         string       `json:"key"` // ally / enemy
	Label       string       `json:"label"`
	SideText    string       `json:"sideText"`
	Badge       string       `json:"badge"`
	PlayerCount int          `json:"playerCount"`
	PhaseLabel  string       `json:"phaseLabel"`
	WinRate     float64      `json:"winRate"`
	CompScore   float64      `json:"compScore"`
	Rating      int          `json:"rating"`
	Slots       []PlayerSlot `json:"slots"`
}

// ViewState 对局页整视图
type ViewState struct {
	Phase      string     `json:"phase"`
	QueueLabel string     `json:"queueLabel"`
	QueueID    int        `json:"queueId"` // 当前对局队列 id（0=未知；前端菜单高亮"当前类型"）
	Teams      []TeamView `json:"teams"`   // [ally, enemy]
}

// PhaseLabelCN gameflow 阶段 → 中文状态文案（与前端 lib/phase.ts 同一文案）
func PhaseLabelCN(phase string) string {
	switch phase {
	case "None", "":
		return "大厅中"
	case "Lobby":
		return "房间内"
	case "Matchmaking":
		return "匹配中"
	case "ReadyCheck":
		return "接受对局"
	case "ChampSelect":
		return "选人中"
	case "GameStart":
		return "游戏启动"
	case "InProgress":
		return "游戏中"
	case "WaitingForStats", "PreEndOfGame":
		return "结算中"
	case "EndOfGame":
		return "对局结束"
	case "Reconnect":
		return "重新连接"
	}
	return "大厅中"
}

// ── 内部中间态 ──

// playerRef 聚合中间态（补数前的玩家标识）
type playerRef struct {
	puuid         string
	summonerID    string
	gameName      string
	tagLine       string
	profileIconID int
	championID    int
	isSelf        bool
}

// flexStr 兼容 LCU 中 string/number 形态的 id 字段
type flexStr string

func (f *flexStr) UnmarshalJSON(b []byte) error {
	s := strings.Trim(string(b), `"`)
	if s == "null" {
		s = ""
	}
	*f = flexStr(s)
	return nil
}

// champEntry champion-summary.json 单条
type champEntry struct {
	ID    int    `json:"id"`
	Alias string `json:"alias"`
	Name  string `json:"name"`
}

const champIndexTTL = 10 * time.Minute

// careerTTL 近况补数缓存时长：事件驱动刷新（选人阶段高频 WS）下保护 LCU/SGP
const careerTTL = 90 * time.Second

// defaultCareerLimit 每人近况默认场数（config.pageSize 可热更新为 10/20/30）
const defaultCareerLimit = 20

// careerScanPages 队列过滤时最多扫描页数（凑满 careerLimit 即停）
const careerScanPages = 4

// careerEntry 近况缓存条目（含 hidden 结果，避免反复打隐藏档案）
type careerEntry struct {
	at time.Time
	c  career
}

// Service 对局信息聚合服务
type Service struct {
	cliFn  func() (lcuAPI, error)
	selfFn func() lcu.ConnStatus
	hist   histAPI
	live   liveAPI

	champMu    sync.Mutex
	champAt    time.Time
	champIndex map[string]int // lower(alias|name) → championId

	careerMu    sync.Mutex
	careerLimit int
	careerCache map[string]careerEntry // puuid|filter → 近况（90s TTL）
}

// SetCareerLimit 配置变更时同步每人近况场数（config.pageSize，10/20/30），并清空近况缓存
func (s *Service) SetCareerLimit(n int) {
	if n < 5 || n > 50 {
		return
	}
	s.careerMu.Lock()
	if s.careerLimit != n {
		s.careerLimit = n
		s.careerCache = map[string]careerEntry{}
	}
	s.careerMu.Unlock()
}

func (s *Service) currentCareerLimit() int {
	s.careerMu.Lock()
	defer s.careerMu.Unlock()
	if s.careerLimit <= 0 {
		return defaultCareerLimit
	}
	return s.careerLimit
}

// New 构造聚合服务（live 注入 *liveclient.Client）。
// 并发策略：LCU 请求由 internal/lcu 客户端闸门固定限 2 并发；SGP 战绩不限并发
func New(mon *lcu.Monitor, hist *history.Service, live liveAPI) *Service {
	return &Service{
		cliFn: func() (lcuAPI, error) {
			c, ok := mon.Client()
			if !ok {
				return nil, errors.New("客户端未连接")
			}
			return c, nil
		},
		selfFn: mon.Status,
		hist:   hist,
		live:   live,
	}
}

// GetGameflowState 聚合当前对局视图（绑定方法；未连接报错，前端离线态自行兜底）。
// queueFilter 近况口径：空 = 全部对局；[-1] = 跟随当前对局队列（未知 → 全部）；其余 = 队列 id 集（同类型多 id 归组）。
func (s *Service) GetGameflowState(queueFilter []int) (ViewState, error) {
	cli, err := s.cliFn()
	if err != nil {
		return ViewState{}, err
	}
	self := s.selfFn()
	phase := fetchPhase(cli)
	label := PhaseLabelCN(phase)
	queueLabel := ""
	queueID := 0

	var ally, enemy []playerRef
	switch phase {
	case "Lobby", "Matchmaking", "ReadyCheck":
		ally, enemy, queueID = fromLobby(cli, self)
		if queueID > 0 {
			queueLabel = parser.QueueInfoFor(queueID).Name
		}
	case "ChampSelect":
		ally, enemy = fromChampSelect(cli, self)
		if sess, ok := fetchGameflowSession(cli); ok {
			queueID = sess.queueID()
			queueLabel = sess.queueName()
			// 花名册按 puuid/summonerId 对齐，补齐选人条目缺失的名/头像
			one, two := sess.roster(self)
			// 己方允许补洞（身份不全被 conv 丢弃 / session 未到齐 5 人）；敌方仍禁止，防盲选泄露
			ally = backfillFromRoster(ally, one)
			enrichFromRoster(enemy, one, two)
		}
	case "GameStart", "InProgress", "WaitingForStats", "PreEndOfGame", "EndOfGame", "Reconnect":
		if sess, ok := fetchGameflowSession(cli); ok {
			queueID = sess.queueID()
			queueLabel = sess.queueName()
			ally, enemy = sess.roster(self)
		}
		// 花名册不全（老版本客户端/对局数据未写入）→ Live Client 兜底（取总量更大的一侧）
		if len(ally) == 0 || len(enemy) == 0 {
			if a2, e2 := s.fromLive(cli, self); len(a2)+len(e2) > len(ally)+len(enemy) {
				ally, enemy = a2, e2
			}
		}
	default: // None / 未知：空视图
	}

	filter := resolveQueueFilter(queueFilter, queueID)
	allySlots := s.buildSlots(cli, ally, filter)
	enemySlots := s.buildSlots(cli, enemy, filter)
	return ViewState{
		Phase:      phase,
		QueueLabel: queueLabel,
		QueueID:    queueID,
		Teams: []TeamView{
			sumTeam("ally", "我方", "蓝方·房间", "我方", label, allySlots),
			sumTeam("enemy", "敌方", "红方", "敌方", label, enemySlots),
		},
	}, nil
}

// resolveQueueFilter 近况过滤口径解析：
// [-1]（负数哨兵）= 跟随当前对局队列（未知 → 不过滤）；空 = 全部；其余 = 队列 id 集。
func resolveQueueFilter(queueFilter []int, currentQueueID int) []int {
	if len(queueFilter) == 1 && queueFilter[0] < 0 {
		if currentQueueID > 0 {
			return []int{currentQueueID}
		}
		return nil
	}
	return queueFilter
}

// queueMatches 单场是否命中过滤口径（空口径 = 全部）
func queueMatches(queueID int, filter []int) bool {
	if len(filter) == 0 {
		return true
	}
	for _, f := range filter {
		if f == queueID {
			return true
		}
	}
	return false
}

// ── 数据源 ──

// fetchPhase 当前 gameflow 阶段（失败/非 200 回落 None）
func fetchPhase(cli lcuAPI) string {
	status, body, err := cli.Get(lcu.PathGameflowPhase)
	if err != nil || status != 200 {
		return "None"
	}
	var p string
	if err := json.Unmarshal(body, &p); err != nil || p == "" {
		return "None"
	}
	return p
}

// gsParticipant /lol-gameflow/v1/session gameData.teamOne/teamTwo 花名册条目（字段随版本浮动容错）。
// 国服实测（2026-09-22 探针）：无 gameName/tagLine 且 summonerName 恒空串——名字靠 fillIdentity 回查。
type gsParticipant struct {
	Puuid         string  `json:"puuid"`
	SummonerID    flexStr `json:"summonerId"`
	GameName      string  `json:"gameName"`
	TagLine       string  `json:"tagLine"`
	SummonerName  string  `json:"summonerName"` // 旧版 "name#tag" 兜底（新版恒空）
	ProfileIconID int     `json:"profileIconId"`
	ChampionID    int     `json:"championId"`
}

// gsPick gameData.playerChampionSelections 条目：全量 10 人（teamOne/teamTwo 漏人时的补洞源）。
// 顺序稳定为 participantId 序——前 numPlayersPerTeam 条归我方。
type gsPick struct {
	Puuid      string `json:"puuid"`
	ChampionID int    `json:"championId"`
}

// gameflowSession /lol-gameflow/v1/session 关注部分（队列 + 双队花名册 + 全员选角）。
// 实测形状：队列在 gameData.queue{id,name,numPlayersPerTeam}（顶层/gameData.queueId 均不存在），
// 且 teamTwo 会漏第 5 人——漏网者可在 playerChampionSelections 找回。
type gameflowSession struct {
	GameData struct {
		Queue struct {
			ID                int    `json:"id"`
			Name              string `json:"name"` // 官方本地化队列名（国服中文，可能带尾空格）
			NumPlayersPerTeam int    `json:"numPlayersPerTeam"`
		} `json:"queue"`
		QueueID int             `json:"queueId"` // 历史形态兜底
		TeamOne []gsParticipant `json:"teamOne"`
		TeamTwo []gsParticipant `json:"teamTwo"`
		Picks   []gsPick        `json:"playerChampionSelections"`
	} `json:"gameData"`
	Queue struct {
		ID int `json:"id"`
	} `json:"queue"`
	QueueID int `json:"queueId"` // 历史形态兜底
}

// fetchGameflowSession 拉取 gameflow 会话（失败返回 ok=false，调用方降级）
func fetchGameflowSession(cli lcuAPI) (gameflowSession, bool) {
	status, body, err := cli.Get(lcu.PathGameflowSession)
	if err != nil || status != 200 {
		return gameflowSession{}, false
	}
	var sess gameflowSession
	if json.Unmarshal(body, &sess) != nil {
		return gameflowSession{}, false
	}
	return sess, true
}

// queueID 当前对局队列 id（gameData.queue.id → gameData.queueId → queue.id → 顶层 queueId 四级取值；未知 0）
func (sess gameflowSession) queueID() int {
	q := sess.GameData.Queue.ID
	if q <= 0 {
		q = sess.GameData.QueueID
	}
	if q <= 0 {
		q = sess.Queue.ID
	}
	if q <= 0 {
		q = sess.QueueID
	}
	return q
}

// queueName 队列名：本地表优先；未收录回落 LCU 官方名 gameData.queue.name（裁掉尾空格），再回落 "其他模式 %d"
func (sess gameflowSession) queueName() string {
	q := sess.queueID()
	if q > 0 {
		if info, ok := parser.LookupQueue(q); ok {
			return info.Name
		}
	}
	if official := strings.TrimSpace(sess.GameData.Queue.Name); official != "" {
		return official
	}
	if q > 0 {
		return parser.QueueInfoFor(q).Name
	}
	return ""
}

// roster 双队花名册 → playerRef（teamOne=我方；本人在 teamTwo 时翻转）。
// playerChampionSelections（全量 10 人）补洞：实测 teamTwo 会漏第 5 人，漏网者按 picks 序号归属补回
// （前 numPlayersPerTeam 条归我方）；花名册整体缺失时直接按 picks 对半分。全空返回 nil（调用方降级 Live）。
func (sess gameflowSession) roster(self lcu.ConnStatus) (ally, enemy []playerRef) {
	toRefs := func(list []gsParticipant) []playerRef {
		out := make([]playerRef, 0, len(list))
		for _, p := range list {
			g, t := p.GameName, p.TagLine
			if g == "" {
				g, t = splitName(p.SummonerName)
			}
			sid := string(p.SummonerID)
			if p.Puuid == "" && g == "" && (sid == "" || sid == "0") {
				continue
			}
			out = append(out, playerRef{
				puuid:         p.Puuid,
				summonerID:    sid,
				gameName:      g,
				tagLine:       t,
				profileIconID: p.ProfileIconID,
				championID:    p.ChampionID,
				isSelf:        isSelfMatch(p.Puuid, g, t, self),
			})
		}
		return out
	}
	one, two := toRefs(sess.GameData.TeamOne), toRefs(sess.GameData.TeamTwo)

	// picks 去重保序 + puuid 归属表（显式花名册优先）
	champBy := map[string]int{}
	var ordered []gsPick
	seen := map[string]bool{}
	for _, pk := range sess.GameData.Picks {
		if pk.Puuid == "" {
			continue
		}
		k := strings.ToLower(pk.Puuid)
		champBy[k] = pk.ChampionID
		if !seen[k] {
			seen[k] = true
			ordered = append(ordered, pk)
		}
	}
	side := map[string]int{}
	for _, r := range one {
		if r.puuid != "" {
			side[strings.ToLower(r.puuid)] = 0
		}
	}
	for _, r := range two {
		if r.puuid != "" {
			side[strings.ToLower(r.puuid)] = 1
		}
	}

	// ① 已有条目补 championId（花名册缺选角时）
	for i := range one {
		if one[i].championID == 0 && one[i].puuid != "" {
			one[i].championID = champBy[strings.ToLower(one[i].puuid)]
		}
	}
	for i := range two {
		if two[i].championID == 0 && two[i].puuid != "" {
			two[i].championID = champBy[strings.ToLower(two[i].puuid)]
		}
	}

	// ② 漏网者补回（teamOne/teamTwo 缺条 → 按 picks 序号归属）
	perTeam := sess.GameData.Queue.NumPlayersPerTeam
	if perTeam <= 0 {
		perTeam = 5
	}
	var extraOne, extraTwo []playerRef
	for i, pk := range ordered {
		if _, ok := side[strings.ToLower(pk.Puuid)]; ok {
			continue
		}
		ref := playerRef{
			puuid:      pk.Puuid,
			championID: pk.ChampionID,
			isSelf:     isSelfMatch(pk.Puuid, "", "", self),
		}
		if i < perTeam {
			extraOne = append(extraOne, ref)
		} else {
			extraTwo = append(extraTwo, ref)
		}
	}
	one = append(one, extraOne...)
	two = append(two, extraTwo...)

	if len(one) == 0 && len(two) == 0 {
		return nil, nil
	}
	for _, r := range two {
		if r.isSelf {
			return two, one // 本人在 teamTwo（自定义房视角翻转）
		}
	}
	return one, two
}

// enrichFromRoster 用花名册补齐 refs 缺失字段（按 puuid/summonerId 对齐；不新增条目）
func enrichFromRoster(refs []playerRef, teams ...[]playerRef) {
	idx := map[string]playerRef{}
	for _, list := range teams {
		for _, r := range list {
			if r.puuid != "" {
				idx["p:"+r.puuid] = r
			}
			if r.summonerID != "" && r.summonerID != "0" {
				idx["s:"+r.summonerID] = r
			}
		}
	}
	for i := range refs {
		src, ok := playerRef{}, false
		if refs[i].puuid != "" {
			src, ok = idx["p:"+refs[i].puuid]
		}
		if !ok && refs[i].summonerID != "" && refs[i].summonerID != "0" {
			src, ok = idx["s:"+refs[i].summonerID]
		}
		if !ok {
			continue
		}
		if refs[i].gameName == "" {
			refs[i].gameName, refs[i].tagLine = src.gameName, src.tagLine
		}
		if refs[i].profileIconID == 0 {
			refs[i].profileIconID = src.profileIconID
		}
		if refs[i].summonerID == "" || refs[i].summonerID == "0" {
			refs[i].summonerID = src.summonerID
		}
		if refs[i].puuid == "" {
			refs[i].puuid = src.puuid
		}
		if refs[i].championID == 0 {
			refs[i].championID = src.championID
		}
	}
}

// backfillFromRoster 花名册补洞：先 enrich 已有条目字段，再把 src 中缺失的己方成员追加回 refs。
// 选人分支专用（敌方仍走 enrichFromRoster，防盲选泄露）。
func backfillFromRoster(refs, src []playerRef) []playerRef {
	enrichFromRoster(refs, src)
	if len(src) == 0 {
		return refs
	}
	haveP, haveS := map[string]bool{}, map[string]bool{}
	for _, r := range refs {
		if r.puuid != "" {
			haveP[r.puuid] = true
		}
		if r.summonerID != "" && r.summonerID != "0" {
			haveS[r.summonerID] = true
		}
	}
	for _, r := range src {
		if r.puuid != "" && haveP[r.puuid] {
			continue
		}
		if r.summonerID != "" && r.summonerID != "0" && haveS[r.summonerID] {
			continue
		}
		refs = append(refs, r)
		if r.puuid != "" {
			haveP[r.puuid] = true
		}
		if r.summonerID != "" && r.summonerID != "0" {
			haveS[r.summonerID] = true
		}
	}
	return refs
}

// isSelfMatch 本人判定：puuid 精确匹配优先，回落 gameName#tagLine（忽略大小写）
func isSelfMatch(puuid, gameName, tag string, self lcu.ConnStatus) bool {
	if puuid != "" && self.Puuid != "" && puuid == self.Puuid {
		return true
	}
	if self.GameName != "" && gameName != "" &&
		strings.EqualFold(gameName, self.GameName) && tagEqual(tag, self.TagLine) {
		return true
	}
	return false
}

// lobbyMember /lol-lobby/v2/lobby 成员
type lobbyMember struct {
	Puuid         string  `json:"puuid"`
	SummonerID    flexStr `json:"summonerId"`
	GameName      string  `json:"gameName"`
	TagLine       string  `json:"tagLine"`
	ProfileIconID int     `json:"profileIconId"`
	Team          int     `json:"team"` // 自定义房 1/2；普通房间 0/1
}

// fromLobby 房间成员 → 双队（自定义房 team==2 归敌方，其余归我方）+ 当前房间队列 id
func fromLobby(cli lcuAPI, self lcu.ConnStatus) (ally, enemy []playerRef, queueID int) {
	status, body, err := cli.Get(lcu.PathLobby)
	if err != nil || status != 200 {
		return nil, nil, 0
	}
	var lb struct {
		GameQueueConfig struct {
			QueueID int `json:"queueId"`
		} `json:"gameQueueConfig"`
		Members []lobbyMember `json:"members"`
	}
	if err := json.Unmarshal(body, &lb); err != nil {
		return nil, nil, 0
	}
	queueID = lb.GameQueueConfig.QueueID
	for _, m := range lb.Members {
		ref := playerRef{
			puuid:         m.Puuid,
			summonerID:    string(m.SummonerID),
			gameName:      m.GameName,
			tagLine:       m.TagLine,
			profileIconID: m.ProfileIconID,
			isSelf:        isSelfMatch(m.Puuid, m.GameName, m.TagLine, self),
		}
		if m.Team == 2 {
			enemy = append(enemy, ref)
		} else {
			ally = append(ally, ref)
		}
	}
	return ally, enemy, queueID
}

// csPlayer /lol-champ-select/v1/session 队列条目（新版本含 gameName/tagLine，旧版本仅 id）
type csPlayer struct {
	Puuid         string  `json:"puuid"`
	SummonerID    flexStr `json:"summonerId"`
	ChampionID    int     `json:"championId"`
	GameName      string  `json:"gameName"`
	TagLine       string  `json:"tagLine"`
	ProfileIconID int     `json:"profileIconId"`
}

// fromChampSelect 选人会话 → 双队（盲选敌方无 puuid/summonerId → 跳过为空槽）
func fromChampSelect(cli lcuAPI, self lcu.ConnStatus) (ally, enemy []playerRef) {
	status, body, err := cli.Get(lcu.PathChampSelectSession)
	if err != nil || status != 200 {
		return nil, nil
	}
	var cs struct {
		MyTeam    []csPlayer `json:"myTeam"`
		TheirTeam []csPlayer `json:"theirTeam"`
	}
	if err := json.Unmarshal(body, &cs); err != nil {
		return nil, nil
	}
	conv := func(list []csPlayer, isAlly bool) []playerRef {
		var out []playerRef
		for _, p := range list {
			if p.Puuid == "" && (p.SummonerID == "" || p.SummonerID == "0") {
				continue // 盲选隐藏：留给空槽文案（summonerId 以数字 0 出现）
			}
			out = append(out, playerRef{
				puuid:         p.Puuid,
				summonerID:    string(p.SummonerID),
				gameName:      p.GameName,
				tagLine:       p.TagLine,
				profileIconID: p.ProfileIconID,
				championID:    p.ChampionID,
				isSelf:        isAlly && isSelfMatch(p.Puuid, p.GameName, p.TagLine, self),
			})
		}
		return out
	}
	return conv(cs.MyTeam, true), conv(cs.TheirTeam, false)
}

// fromLive Live Client 10 人 → 按本人队伍分组（数据不可达时返回空，前端保持骨架/自动重试）
func (s *Service) fromLive(cli lcuAPI, self lcu.ConnStatus) (ally, enemy []playerRef) {
	list, err := s.live.PlayerList()
	if err != nil {
		slog.Warn("[gameinfo] live playerlist unavailable", "err", err)
		return nil, nil
	}
	activeName, _ := s.live.ActivePlayerName()
	activeGame, activeTag := splitName(activeName)
	match := func(puuid, g, t string) bool {
		if isSelfMatch(puuid, g, t, self) {
			return true
		}
		return activeGame != "" && strings.EqualFold(g, activeGame) && tagEqual(t, activeTag)
	}

	// 本人队伍编号：puuid/名字命中的条目
	selfTeam := liveclient.TeamNone
	for _, p := range list {
		g, t := p.Name()
		if match(p.Puuid, g, t) {
			selfTeam = p.Team
			break
		}
	}

	for _, p := range list {
		g, t := p.Name()
		ref := playerRef{
			puuid:      p.Puuid,
			gameName:   g,
			tagLine:    t,
			championID: s.championIDByName(cli, p.ChampionName),
			isSelf:     match(p.Puuid, g, t),
		}
		// 本人队为 ally；无法判定本人（改名/离线）时 100 蓝方兜底为 ally
		team := p.Team
		switch {
		case selfTeam != liveclient.TeamNone && team == selfTeam:
			ally = append(ally, ref)
		case selfTeam != liveclient.TeamNone:
			enemy = append(enemy, ref)
		case team == liveclient.TeamRed:
			enemy = append(enemy, ref)
		default:
			ally = append(ally, ref)
		}
	}
	// 兜底：team 字段不可用导致 10 人挤单侧（回归：游戏中敌方 0 人）→ 按列表序对半分
	if len(ally) == 10 && len(enemy) == 0 {
		mid := len(ally) / 2
		ally, enemy = ally[:mid], ally[mid:]
	} else if len(enemy) == 10 && len(ally) == 0 {
		mid := len(enemy) / 2
		enemy, ally = enemy[:mid], enemy[mid:]
	}
	return ally, enemy
}

// ── 补数与汇总 ──

// buildSlots 补数（标识互查 → 段位批量 → 近况并发）并补齐槽位。
// 常规 5 人队 pad 到 5；竞技场/多队伍按 refs 实际人数，避免 refs[:5] 截断丢人。
// filter 为近况队列口径（空 = 全部；统计与列表同源同口径）。
func (s *Service) buildSlots(cli lcuAPI, refs []playerRef, filter []int) []PlayerSlot {
	want := 5
	if len(refs) > 5 {
		want = len(refs)
	}
	slots := make([]PlayerSlot, 0, want)
	// 不再硬截 5：多队伍模式保留全部 refs

	// ① 标识互查（LCU 并发由 internal/lcu 闸门兜底）：标识不全即查——live 常缺 icon/sid，champ-select 缺名，老版本 live 缺 puuid
	var wg sync.WaitGroup
	for i := range refs {
		incomplete := refs[i].puuid == "" || refs[i].gameName == "" ||
			refs[i].profileIconID == 0 || refs[i].summonerID == ""
		if !incomplete {
			continue
		}
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			s.fillIdentity(cli, &refs[i])
		}(i)
	}
	wg.Wait()

	// ② 段位批量（双键）
	ids := make([]string, 0, len(refs)*2)
	for _, r := range refs {
		if r.summonerID != "" {
			ids = append(ids, r.summonerID)
		}
		if r.puuid != "" {
			ids = append(ids, r.puuid)
		}
	}
	rankMap := map[string]history.RankedInfo{}
	if len(ids) > 0 {
		if rows, err := s.hist.GetPlayersRanked(ids); err == nil {
			for _, r := range rows {
				if r.SummonerID != "" {
					rankMap[r.SummonerID] = r
				}
				if r.Puuid != "" {
					rankMap[r.Puuid] = r
				}
			}
		}
	}

	// ③ 近况并发（每人 GetMatches，条数 = config.pageSize；SGP 不限并发，LCU 由客户端闸门限 2）
	careers := make([]career, len(refs))
	for i := range refs {
		if refs[i].puuid == "" {
			careers[i].hidden = true
			continue
		}
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			careers[i] = s.fetchCareer(refs[i].puuid, filter)
		}(i)
	}
	wg.Wait()

	for i, r := range refs {
		c := careers[i]
		slot := PlayerSlot{
			Filled:        true,
			IsSelf:        r.isSelf,
			Puuid:         r.puuid,
			SummonerID:    r.summonerID,
			GameName:      r.gameName,
			TagLine:       r.tagLine,
			ProfileIconID: r.profileIconID,
			ChampionID:    r.championID,
			HiddenCareer:  c.hidden,
			Recent:        c.recent,
		}
		if rk, ok := rankMap[r.summonerID]; ok {
			slot.Solo, slot.Flex = pickRank(rk)
		} else if rk, ok := rankMap[r.puuid]; ok {
			slot.Solo, slot.Flex = pickRank(rk)
		}
		slot.WinRate, slot.WinRateSample, slot.AvgKda, slot.Rating = careerStats(c.recent)
		slots = append(slots, slot)
	}
	for len(slots) < 5 {
		slots = append(slots, PlayerSlot{})
	}
	return slots
}

// summonerRaw /lol-summoner/v1/summoners/* 响应（字段随版本浮动：新 gameName/tagLine，旧 displayName）
type summonerRaw struct {
	Puuid         string  `json:"puuid"`
	GameName      string  `json:"gameName"`
	TagLine       string  `json:"tagLine"`
	DisplayName   string  `json:"displayName"` // "gameName#tagLine" 旧版兜底
	ProfileIconID int     `json:"profileIconId"`
	SummonerID    flexStr `json:"summonerId"`
}

// apply 补齐缺失字段（不覆盖已有值；displayName 兜底拆分）
func (r summonerRaw) apply(ref *playerRef) {
	if ref.gameName == "" {
		if r.GameName != "" {
			ref.gameName, ref.tagLine = r.GameName, r.TagLine
		} else if r.DisplayName != "" {
			ref.gameName, ref.tagLine = splitName(r.DisplayName)
		}
	}
	if ref.profileIconID == 0 && r.ProfileIconID != 0 {
		ref.profileIconID = r.ProfileIconID
	}
	if (ref.summonerID == "" || ref.summonerID == "0") && r.SummonerID != "" && r.SummonerID != "0" {
		ref.summonerID = string(r.SummonerID)
	}
	if ref.puuid == "" && r.Puuid != "" {
		ref.puuid = r.Puuid
	}
}

// lookupSummoner 单次召唤师查询（LCU；响应体无效视为未命中）
func lookupSummoner(cli lcuAPI, path string) (summonerRaw, bool) {
	status, body, err := cli.Get(path)
	if err != nil || status != 200 {
		return summonerRaw{}, false
	}
	var r summonerRaw
	if json.Unmarshal(body, &r) != nil {
		return summonerRaw{}, false
	}
	if r.Puuid == "" && r.GameName == "" && r.DisplayName == "" && r.ProfileIconID == 0 {
		return summonerRaw{}, false
	}
	return r, true
}

// fillIdentity 标识互查补齐（best-effort，三级回退）：
//
//	① by-puuid（live/选人新版本）；② by-summonerId（by-puuid 未命中兜底，实测对他人可达）；
//	③ 按名搜索（有名缺 icon/sid/puuid 时）。全部失败保留原值不报错。
func (s *Service) fillIdentity(cli lcuAPI, ref *playerRef) {
	if ref.puuid != "" {
		if r, ok := lookupSummoner(cli, fmt.Sprintf(lcu.PathSummonerByPuuid, url.PathEscape(ref.puuid))); ok {
			r.apply(ref)
		}
	}
	if (ref.gameName == "" || ref.profileIconID == 0) && ref.summonerID != "" && ref.summonerID != "0" {
		if r, ok := lookupSummoner(cli, fmt.Sprintf(lcu.PathSummonerByID, url.PathEscape(ref.summonerID))); ok {
			r.apply(ref)
		}
	}
	if (ref.puuid == "" || ref.profileIconID == 0) && ref.gameName != "" {
		name := ref.gameName
		if ref.tagLine != "" {
			name += "#" + ref.tagLine
		}
		if res, err := s.hist.SearchSummoner(name); err == nil {
			(summonerRaw{
				Puuid:         res.Puuid,
				GameName:      res.GameName,
				TagLine:       res.TagLine,
				ProfileIconID: res.ProfileIconID,
				SummonerID:    flexStr(res.SummonerID),
			}).apply(ref)
		}
	}
}

// fetchCareer 近 N 场摘要（N=config.pageSize；生涯隐藏/无数据 → hidden；90s 缓存抑制事件风暴）
func (s *Service) fetchCareer(puuid string, filter []int) career {
	key := puuid + "|" + fmt.Sprint(filter)
	s.careerMu.Lock()
	if e, ok := s.careerCache[key]; ok && time.Since(e.at) < careerTTL {
		s.careerMu.Unlock()
		return e.c
	}
	s.careerMu.Unlock()

	c := s.loadCareer(puuid, filter)

	s.careerMu.Lock()
	if s.careerCache == nil {
		s.careerCache = map[string]careerEntry{}
	}
	// 达上限先清过期，仍超则整表重置，防长会话无界增长
	if len(s.careerCache) >= 512 {
		now := time.Now()
		for k, e := range s.careerCache {
			if now.Sub(e.at) >= careerTTL {
				delete(s.careerCache, k)
			}
		}
		if len(s.careerCache) >= 512 {
			s.careerCache = map[string]careerEntry{}
		}
	}
	s.careerCache[key] = careerEntry{at: time.Now(), c: c}
	s.careerMu.Unlock()
	return c
}

// loadCareer 实际拉取近况（fetchCareer 的缓存未命中路径）：
// 多页扫描凑满 careerLimit 场同队列（最多 careerScanPages 页），凑满/翻完/首页失败即停；
// 首页失败 = 生涯隐藏或无记录（hidden）；成功但该类型 0 场 = 空列表（前端"暂无近战数据"）。
func (s *Service) loadCareer(puuid string, filter []int) career {
	limit := s.currentCareerLimit()
	out := make([]RecentMatch, 0, limit)
	for page := 0; page < careerScanPages; page++ {
		p, err := s.hist.GetMatches(puuid, page)
		if err != nil {
			if page == 0 {
				return career{hidden: true}
			}
			break // 部分页失败：用已收集
		}
		for _, m := range p.Summaries {
			if !queueMatches(m.QueueID, filter) {
				continue
			}
			out = append(out, RecentMatch{
				QueueShort:   m.QueueShort,
				QueueName:    m.QueueName,
				TimeShort:    m.ShortTime,
				GameCreation: m.GameCreation,
				Win:          m.Win,
				Kills:        m.Kills,
				Deaths:       m.Deaths,
				Assists:      m.Assists,
				ChampionID:   m.ChampionID,
			})
			if len(out) >= limit {
				return career{recent: out}
			}
		}
		if !p.HasMore || len(p.Summaries) == 0 {
			break
		}
	}
	return career{recent: out}
}
