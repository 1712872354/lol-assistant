// Package history 历史战绩服务层（M2）：绑定层与 LCU 之间的业务逻辑。
// 数据通路：monitor.Client() 复用 M1 连接层客户端；本包不持有连接状态。
// 端点契约见开发方案 §6.1；所有 JSON 输出 camelCase，前端 lib/types.ts 一一对应。
package history

import (
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"net/url"
	"strconv"
	"strings"
	"sync"

	"github.com/1712872354/lol-assistant/internal/lcu"
	"github.com/1712872354/lol-assistant/internal/parser"
	"github.com/1712872354/lol-assistant/internal/sgp"
)

// ErrNotConnected LCU 未连接（绑定层直接透传给前端展示）
var ErrNotConnected = errors.New("LCU 未连接，请先启动英雄联盟客户端并登录")

// Service 战绩服务：查询/明细/段位/资源代理
type Service struct {
	clientFn func() (*lcu.Client, bool)
	statusFn func() lcu.ConnStatus

	pageMu   sync.RWMutex
	pageSize int

	semMu       sync.RWMutex
	sem         chan struct{} // LCU 请求并发闸门（配置 apiConcurrency，可热更新）
	concurrency int
	assets      *assetCache

	// SGP 云端主数据源（开发方案 §4.2：SGP 优先、LCU 兜底；config.sgpEnabled 关闭则回退纯 LCU）
	sgpMu           sync.RWMutex
	sgpEnabled      bool
	sgpFetchFn      sgpFetchFn      // 测试注入；nil 时用 sgp.FetchRankedStats
	sgpMatchFetchFn sgpMatchFetchFn // 测试注入；nil 时用 sgp.FetchMatchHistory
}

// New 由 Monitor 构造服务（pageSize/apiConcurrency 来自应用配置）
// SGP 数据源开关由 app 层按 config.sgpEnabled 显式注入（默认开启，fail-soft 兜底 LCU）
func New(mon *lcu.Monitor, pageSize, concurrency int) *Service {
	return NewWithClient(mon.Client, mon.Status, pageSize, concurrency)
}

// NewWithClient 供单元测试注入客户端与状态函数
func NewWithClient(
	clientFn func() (*lcu.Client, bool),
	statusFn func() lcu.ConnStatus,
	pageSize, concurrency int,
) *Service {
	if pageSize < 5 || pageSize > 50 {
		pageSize = 20
	}
	if concurrency < 2 || concurrency > 10 {
		concurrency = 5
	}
	return &Service{
		clientFn:    clientFn,
		statusFn:    statusFn,
		pageSize:    pageSize,
		sem:         make(chan struct{}, concurrency),
		concurrency: concurrency,
		assets:      newAssetCache(),
	}
}

// SetSGPEnabled 配置变更时同步 SGP 数据源开关（config.sgpEnabled；关闭后段位/战绩回退纯 LCU）
func (s *Service) SetSGPEnabled(on bool) {
	s.sgpMu.Lock()
	s.sgpEnabled = on
	s.sgpMu.Unlock()
}

// SetConcurrency 配置变更时同步并发闸门容量（config.apiConcurrency，三挡 2/5/10）
func (s *Service) SetConcurrency(n int) {
	if n != 2 && n != 5 && n != 10 {
		return
	}
	s.semMu.Lock()
	defer s.semMu.Unlock()
	if s.concurrency == n {
		return
	}
	s.concurrency = n
	s.sem = make(chan struct{}, n)
}

// acquire 占用一个并发槽；返回 release
func (s *Service) acquire() func() {
	s.semMu.RLock()
	sem := s.sem
	s.semMu.RUnlock()
	sem <- struct{}{}
	return func() { <-sem }
}

// SetPageSize 配置变更时同步分页大小（与 config.sanitize 取值域一致）
func (s *Service) SetPageSize(n int) {
	if n < 5 || n > 50 {
		return
	}
	s.pageMu.Lock()
	s.pageSize = n
	s.pageMu.Unlock()
}

func (s *Service) currentPageSize() int {
	s.pageMu.RLock()
	defer s.pageMu.RUnlock()
	return s.pageSize
}

func (s *Service) client() (*lcu.Client, error) {
	if cli, ok := s.clientFn(); ok && cli != nil {
		return cli, nil
	}
	return nil, ErrNotConnected
}

/* ─── 召唤师查询 ───────────────────────────────────────────────── */

// SummonerResult 召唤师查询结果（标签页打开凭据）
type SummonerResult struct {
	Puuid         string `json:"puuid"`
	GameName      string `json:"gameName"`
	TagLine       string `json:"tagLine"`
	DisplayName   string `json:"displayName"` // "gameName#tagLine"
	ProfileIconID int    `json:"profileIconId"`
	SummonerLevel int    `json:"summonerLevel"`
	SummonerID    string `json:"summonerId"`
}

// flexStr 兼容 LCU 中 string/number 形态的 summonerId
type flexStr string

func (f *flexStr) UnmarshalJSON(b []byte) error {
	s := strings.Trim(string(b), `"`)
	if s == "null" {
		s = ""
	}
	*f = flexStr(s)
	return nil
}

// SearchSummoner 按 Riot ID（昵称#TAG）或召唤师名查询
func (s *Service) SearchSummoner(name string) (SummonerResult, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		return SummonerResult{}, errors.New("请输入召唤师昵称或 Riot ID（昵称#TAG）")
	}
	cli, err := s.client()
	if err != nil {
		return SummonerResult{}, err
	}

	path := lcu.PathSummonersByName + "?name=" + url.QueryEscape(name)
	status, body, err := cli.Get(path)
	if err != nil {
		return SummonerResult{}, fmt.Errorf("查询召唤师失败: %w", err)
	}
	switch {
	case status == 404:
		return SummonerResult{}, fmt.Errorf("未找到召唤师「%s」，请检查昵称#TAG", name)
	case status < 200 || status >= 300:
		return SummonerResult{}, fmt.Errorf("查询召唤师失败: HTTP %d", status)
	}

	var raw struct {
		Puuid         string  `json:"puuid"`
		GameName      string  `json:"gameName"`
		TagLine       string  `json:"tagLine"`
		DisplayName   string  `json:"displayName"`
		ProfileIconID int     `json:"profileIconId"`
		SummonerLevel int     `json:"summonerLevel"`
		SummonerID    flexStr `json:"summonerId"`
	}
	if err := json.Unmarshal(body, &raw); err != nil {
		return SummonerResult{}, fmt.Errorf("解析召唤师数据失败: %w", err)
	}
	if raw.Puuid == "" {
		return SummonerResult{}, fmt.Errorf("未找到召唤师「%s」，请检查昵称#TAG", name)
	}
	return SummonerResult{
		Puuid:         raw.Puuid,
		GameName:      raw.GameName,
		TagLine:       raw.TagLine,
		DisplayName:   displayNameOf(raw.GameName, raw.TagLine, raw.DisplayName),
		ProfileIconID: raw.ProfileIconID,
		SummonerLevel: raw.SummonerLevel,
		SummonerID:    string(raw.SummonerID),
	}, nil
}

// GetSelfSummoner 当前登录召唤师（Monitor 状态快照 + by-puuid 补全 summonerId，尽力而为）
func (s *Service) GetSelfSummoner() (SummonerResult, error) {
	st := s.statusFn()
	if st.State != lcu.StateConnected || st.Puuid == "" {
		return SummonerResult{}, errors.New("客户端未登录，无法获取当前召唤师")
	}
	res := SummonerResult{
		Puuid:         st.Puuid,
		GameName:      st.GameName,
		TagLine:       st.TagLine,
		DisplayName:   displayNameOf(st.GameName, st.TagLine, ""),
		ProfileIconID: st.ProfileIconId,
		SummonerLevel: st.SummonerLevel,
	}
	if cli, ok := s.clientFn(); ok && cli != nil {
		if status, body, err := cli.Get(fmt.Sprintf(lcu.PathSummonerByPuuid, url.PathEscape(st.Puuid))); err == nil &&
			status >= 200 && status < 300 {
			var raw struct {
				SummonerID    flexStr `json:"summonerId"`
				ProfileIconID int     `json:"profileIconId"`
				SummonerLevel int     `json:"summonerLevel"`
			}
			if json.Unmarshal(body, &raw) == nil {
				res.SummonerID = string(raw.SummonerID)
				if raw.ProfileIconID > 0 {
					res.ProfileIconID = raw.ProfileIconID
				}
				if raw.SummonerLevel > 0 {
					res.SummonerLevel = raw.SummonerLevel
				}
			}
		}
	}
	return res, nil
}

func displayNameOf(gameName, tagLine, fallback string) string {
	name := gameName
	if name == "" {
		name = fallback
	}
	if name == "" {
		return ""
	}
	if tagLine != "" {
		return name + "#" + tagLine
	}
	return name
}

/* ─── 战绩列表 ─────────────────────────────────────────────────── */

// MatchPage 分页战绩结果
type MatchPage struct {
	Puuid      string                `json:"puuid"`
	Page       int                   `json:"page"`
	PageSize   int                   `json:"pageSize"`
	GameCount  int                   `json:"gameCount"`
	TotalPages int                   `json:"totalPages"`
	HasMore    bool                  `json:"hasMore"`
	Summaries  []parser.MatchSummary `json:"summaries"`
}

// GetMatches 指定 puuid 的战绩分页（begIndex/endIndex 步进，pageSize 来自配置）。
// 数据通路：SGP match-history-query 优先（腾讯云端，国服他人战绩更全），
// SGP 失败/禁用/首页空结果回退 LCU match-history。
func (s *Service) GetMatches(puuid string, page int) (MatchPage, error) {
	puuid = strings.TrimSpace(puuid)
	if puuid == "" {
		return MatchPage{}, errors.New("缺少召唤师 puuid")
	}
	if page < 0 {
		page = 0
	}
	pageSize := s.currentPageSize()
	beg := page * pageSize
	end := beg + pageSize - 1

	// ── SGP 优先：成功即返回；失败/无凭据静默回退 LCU（SGP 网络失败 ≠ 无战绩）──
	if mp, ok := s.getMatchesSGP(puuid, page, beg, pageSize); ok {
		return mp, nil
	}

	cli, err := s.client()
	if err != nil {
		return MatchPage{}, err
	}

	path := fmt.Sprintf(lcu.PathMatchHistory, url.PathEscape(puuid)) +
		fmt.Sprintf("?begIndex=%d&endIndex=%d", beg, end)
	status, body, err := cli.Get(path)
	if err != nil {
		return MatchPage{}, fmt.Errorf("获取战绩失败: %w", err)
	}
	switch {
	case status == 404:
		return MatchPage{}, errors.New("未找到战绩数据（该账号近期无对局记录）")
	case status < 200 || status >= 300:
		return MatchPage{}, fmt.Errorf("获取战绩失败: HTTP %d（国服查询他人战绩可能受限）", status)
	}

	summaries, gameCount, perr := parser.ParseMatchSummaries(body, puuid)
	if perr != nil {
		return MatchPage{}, perr
	}
	totalPages := 1
	if gameCount > 0 {
		totalPages = (gameCount + pageSize - 1) / pageSize
	}
	hasMore := beg+len(summaries) < gameCount
	if gameCount <= 0 {
		hasMore = len(summaries) >= pageSize
	}
	return MatchPage{
		Puuid:      puuid,
		Page:       page,
		PageSize:   pageSize,
		GameCount:  gameCount,
		TotalPages: totalPages,
		HasMore:    hasMore,
		Summaries:  summaries,
	}, nil
}

// getMatchesSGP SGP match-history-query 战绩通路（主源）。
// ok=false 表示 SGP 不可用或失败，调用方回退 LCU。首页空结果同样回退，
// 保留 LCU 404 → "生涯隐藏/无记录" 的既有分类语义；后续页空结果视为正常翻页终点。
// SGP 响应无总数字段：gameCount 为已见下界，hasMore 按返回条数是否满页推断。
func (s *Service) getMatchesSGP(puuid string, page, beg, pageSize int) (MatchPage, bool) {
	s.sgpMu.RLock()
	sgpOn := s.sgpEnabled
	s.sgpMu.RUnlock()
	if !sgpOn {
		return MatchPage{}, false
	}
	cli, err := s.client()
	if err != nil {
		return MatchPage{}, false
	}
	var host string
	if st := s.statusFn(); st.PlatformId != "" {
		host = sgp.Host(st.PlatformId)
	}
	if host == "" {
		return MatchPage{}, false
	}
	toks := s.fetchSGPTokens(cli).match()
	if len(toks) == 0 {
		return MatchPage{}, false // 无凭据 → 本批仅走 LCU，不视为错误
	}

	release := s.acquire()
	defer release()

	fetch := s.sgpMatchFetchFn
	if fetch == nil {
		fetch = sgp.FetchMatchHistory
	}
	var body []byte
	var lastErr error
	for _, tok := range toks {
		b, err := fetch(host, puuid, tok, beg, pageSize)
		if err == nil {
			body = b
			break
		}
		lastErr = err
	}
	if body == nil {
		slog.Debug("[sgp] match history failed, fallback to LCU", "puuid", puuid, "err", lastErr)
		return MatchPage{}, false
	}
	summaries, err := parser.ParseSGPSummaries(body, puuid)
	if err != nil {
		slog.Debug("[sgp] match history parse failed, fallback to LCU", "puuid", puuid, "err", err)
		return MatchPage{}, false
	}
	if len(summaries) == 0 && page == 0 {
		return MatchPage{}, false // 首页空结果交 LCU 裁决（隐藏/无记录分类）
	}

	hasMore := len(summaries) >= pageSize
	totalPages := page + 1
	if hasMore {
		totalPages++
	}
	return MatchPage{
		Puuid:      puuid,
		Page:       page,
		PageSize:   pageSize,
		GameCount:  beg + len(summaries), // SGP 无总数字段：未翻完时为总数下界
		TotalPages: totalPages,
		HasMore:    hasMore,
		Summaries:  summaries,
	}, true
}

/* ─── 对局明细 ─────────────────────────────────────────────────── */

// GetMatchDetail 单局 10 人（竞技场为全场）明细；selfPuuid 用于标记本人行与队伍置顶
func (s *Service) GetMatchDetail(gameID int64, selfPuuid string) (*parser.MatchDetail, error) {
	if gameID <= 0 {
		return nil, errors.New("无效对局 ID")
	}
	cli, err := s.client()
	if err != nil {
		return nil, err
	}
	status, body, err := cli.Get(fmt.Sprintf(lcu.PathMatchGameDetail, gameID))
	if err != nil {
		return nil, fmt.Errorf("获取对局明细失败: %w", err)
	}
	if status < 200 || status >= 300 {
		return nil, fmt.Errorf("获取对局明细失败: HTTP %d", status)
	}
	return parser.ParseMatchDetail(body, strings.TrimSpace(selfPuuid))
}

/* ─── 段位查询（SGP 主 + LCU 兜底；明细页/对局页段位列，尽力而为） ── */

// RankedInfo 单个召唤师的段位展示串
type RankedInfo struct {
	SummonerID string `json:"summonerId"` // 查询用的 id（summonerId 或 puuid）
	Puuid      string `json:"puuid,omitempty"`
	Solo       string `json:"solo"` // "黄金 IV 45" / "未定级"
	Flex       string `json:"flex"`
}

// sgpFetchFn SGP 段位拉取函数签名（测试注入用）
type sgpFetchFn func(host, puuid, token string) (*sgp.RankedStats, error)

// sgpMatchFetchFn SGP 战绩拉取函数签名（测试注入用）
type sgpMatchFetchFn func(host, puuid, token string, startIndex, count int) ([]byte, error)

// unranked 未定级展示串（与前端 format.ts rankedDisplay 判定一致）
const unranked = "未定级"

// GetPlayersRanked 批量查询段位（并发受 apiConcurrency 限流）。
// 入参可为 summonerId 或 puuid（国服对局 identities 两种都可能出现，前端双键收集）。
//
// 数据通路（探针 v11 实测校准；SGP 契约取自 Akari 2026-07）：
//  1. 归一分桶：summonerId 形态先经 LCU /lol-summoner/v1/summoners/{sid} 反查 puuid
//     （实测他人 10/10 可反查且一致）；LCU ranked-stats 仅 puuid 键返回真实段位，
//     summonerId 键恒为空 tier —— 此为明细页段位缺失的根因。
//  2. 每玩家唯一查询：SGP leagues-ledge 主查（腾讯云端权威源，HTTP 200 即采用，
//     "未定级"亦为数据事实，不再回退）；SGP 失败/禁用/无凭据时回退
//     LCU /lol-ranked/v1/ranked-stats/{puuid}。
//  3. 双键展开：同一玩家的每个入参 id（summonerId/puuid）各输出一条同数据条目，
//     前端 buildRankedMap 双键索引任一键命中；查询彻底失败的 id 不输出条目，
//     前端回退 PlayerRow.tierShort（历史最高段位）展示。
func (s *Service) GetPlayersRanked(summonerIDs []string) ([]RankedInfo, error) {
	cli, err := s.client()
	if err != nil {
		return nil, err
	}
	seen := map[string]bool{}
	ids := make([]string, 0, len(summonerIDs))
	for _, id := range summonerIDs {
		id = strings.TrimSpace(id)
		if id == "" || seen[id] {
			continue
		}
		seen[id] = true
		ids = append(ids, id)
		if len(ids) >= 40 { // 双键入参（10 人 × 2 键 + tab 键）需放宽上限
			break
		}
	}
	if len(ids) == 0 {
		return []RankedInfo{}, nil
	}

	// ── 阶段 1：归一分桶（并发反查 summonerId → puuid）──
	// bucket: canonical id（puuid 优先，反查失败保留原始 id）→ 入参 id 列表
	type bucket struct {
		canonical string
		inputIDs  []string
	}
	var (
		bmu      sync.Mutex
		buckets  = map[string]*bucket{}
		bktOrder = make([]string, 0, len(ids))
	)
	var wg sync.WaitGroup
	for _, id := range ids {
		wg.Add(1)
		go func(id string) {
			defer wg.Done()
			canonical := id
			if !isPuuid(id) {
				if pu := resolvePuuid(cli, id); pu != "" {
					canonical = pu
				}
			}
			bmu.Lock()
			if b, ok := buckets[canonical]; ok {
				b.inputIDs = append(b.inputIDs, id)
			} else {
				buckets[canonical] = &bucket{canonical: canonical, inputIDs: []string{id}}
				bktOrder = append(bktOrder, canonical)
			}
			bmu.Unlock()
		}(id)
	}
	wg.Wait()

	// ── SGP 运行参数快照：开关 + 区服 host + 双凭据（每调用现取，无凭据则本批回退 LCU）──
	s.sgpMu.RLock()
	sgpOn := s.sgpEnabled
	s.sgpMu.RUnlock()
	var sgpHost string
	var toks sgpTokens
	if sgpOn {
		if st := s.statusFn(); st.PlatformId != "" {
			sgpHost = sgp.Host(st.PlatformId)
		}
		if sgpHost != "" {
			toks = s.fetchSGPTokens(cli)
		}
		if len(toks.ranked()) == 0 {
			sgpHost = "" // 无凭据 → 本批仅走 LCU，不视为错误
		}
	}

	// ── 阶段 2：每玩家唯一查询（SGP 主 → LCU 兜底）──
	type rankResult struct {
		solo, flex string
		ok         bool // 有条目可输出（SGP/LCU 200 即使未定级也算）
	}
	results := make(map[string]rankResult, len(bktOrder))
	var rmu sync.Mutex
	wg = sync.WaitGroup{}
	for _, canonical := range bktOrder {
		wg.Add(1)
		go func(c string) {
			defer wg.Done()
			release := s.acquire()
			defer release()

			res := rankResult{solo: unranked, flex: unranked}
			filled := false
			// SGP 主查：腾讯云端权威源，HTTP 200 即采用（"未定级"也是数据事实，不回退 LCU）
			if sgpHost != "" && isPuuid(c) {
				fetch := s.sgpFetchFn
				if fetch == nil {
					fetch = sgp.FetchRankedStats
				}
				for _, tok := range toks.ranked() {
					stats, err := fetch(sgpHost, c, tok)
					if err != nil {
						// 网络失败 ≠ 玩家无段位：换凭据重试，全败则回退 LCU
						slog.Debug("[sgp] ranked fetch failed", "puuid", c, "err", err)
						continue
					}
					solo, flex := sgpRankedDisplay(stats)
					res.solo, res.flex, res.ok = solo, flex, true
					filled = true
					slog.Debug("[sgp] ranked hit", "puuid", c, "solo", solo, "flex", flex)
					break
				}
			}
			// LCU 兜底：SGP 不可用/失败，或非 puuid 键（summonerId 反查失败）
			if !filled {
				if info, ok := fetchRanked(cli, c); ok {
					res.solo, res.flex, res.ok = info.Solo, info.Flex, true
				}
			}
			rmu.Lock()
			results[c] = res
			rmu.Unlock()
		}(canonical)
	}
	wg.Wait()

	// ── 阶段 3：双键展开输出（保持入参顺序；失败桶无条目）──
	out := make([]RankedInfo, 0, len(ids))
	for _, canonical := range bktOrder {
		res := results[canonical]
		if !res.ok {
			continue
		}
		b := buckets[canonical]
		puuid := ""
		if isPuuid(canonical) {
			puuid = canonical
		}
		for _, inputID := range b.inputIDs {
			out = append(out, RankedInfo{
				SummonerID: inputID,
				Puuid:      puuid,
				Solo:       res.solo,
				Flex:       res.flex,
			})
		}
	}
	return out, nil
}

// sgpTokens SGP 双凭据快照（league-session 主认证 + entitlements 兜底）
type sgpTokens struct {
	session, entitlements string
}

// ranked 段位通路凭据顺序：league-session（实测通路）→ entitlements
func (t sgpTokens) ranked() []string { return t.nonEmpty(t.session, t.entitlements) }

// match 战绩通路凭据顺序：entitlements（Akari 契约）→ league-session
func (t sgpTokens) match() []string { return t.nonEmpty(t.entitlements, t.session) }

func (t sgpTokens) nonEmpty(a, b string) []string {
	out := make([]string, 0, 2)
	if a != "" {
		out = append(out, a)
	}
	if b != "" && b != a {
		out = append(out, b)
	}
	return out
}

// fetchSGPTokens 获取 SGP 双凭据（每调用现取；缺失项留空，由调用方降级）
func (s *Service) fetchSGPTokens(cli *lcu.Client) sgpTokens {
	var t sgpTokens
	if tok, err := sgp.FetchLeagueSessionToken(cli.Get); err == nil {
		t.session = tok
	} else {
		slog.Debug("[sgp] league-session token unavailable", "err", err)
	}
	if tok, err := sgp.FetchEntitlementsToken(cli.Get); err == nil {
		t.entitlements = tok
	} else {
		slog.Debug("[sgp] entitlements token unavailable", "err", err)
	}
	return t
}

// isPuuid 判定 id 是否 puuid 形态：含 '-' 且长度 ≥32（Riot puuid 为 UUID 形态；
// 国服 summonerId 为纯数字串，二者可稳定区分）
func isPuuid(id string) bool {
	return len(id) >= 32 && strings.Contains(id, "-")
}

// resolvePuuid summonerId → puuid（LCU /lol-summoner/v1/summoners/{sid}，尽力而为）
func resolvePuuid(cli *lcu.Client, summonerID string) string {
	status, body, err := cli.Get(fmt.Sprintf(lcu.PathSummonerByID, url.PathEscape(summonerID)))
	if err != nil || status < 200 || status >= 300 {
		return ""
	}
	var raw struct {
		Puuid string `json:"puuid"`
	}
	if err := json.Unmarshal(body, &raw); err != nil {
		return ""
	}
	return strings.TrimSpace(raw.Puuid)
}

// sgpRankedDisplay 解析 SGP leagues-ledge queues → (solo, flex) 展示串。
// 仅取 RANKED_SOLO_5x5 / RANKED_FLEX_SR；JADE_*（国服特殊队列）与 TFT 系列忽略。
// SGP 响应用 rank 字段（I/II/III/IV）表示小级，经 QueueEntry.Div() 归一。
func sgpRankedDisplay(stats *sgp.RankedStats) (string, string) {
	find := func(queue string) (string, string, int) {
		for _, q := range stats.Queues {
			if q.QueueType == queue && q.Tier != "" {
				return q.Tier, q.Div(), q.LeaguePoints
			}
		}
		return "", "", 0
	}
	soloT, soloD, soloLP := find("RANKED_SOLO_5x5")
	flexT, flexD, flexLP := find("RANKED_FLEX_SR")
	return rankedDisplay(soloT, soloD, soloLP), rankedDisplay(flexT, flexD, flexLP)
}

// rankedQueueEntry 段位队列条目（LCU 多种响应形态共用）
type rankedQueueEntry struct {
	QueueType    string `json:"queueType"`
	Tier         string `json:"tier"`
	Division     string `json:"division"`
	LeaguePoints int    `json:"leaguePoints"`
}

// fetchRanked GET /lol-ranked/v1/ranked-stats/{id}（id 应为 puuid；
// summonerId 键国服实测恒返回空 tier，仅作兼容兜底路径）。
// 解析兼容 queueMap / queues / leagues / highestRankedEntry 等多种 LCU 形态。
func fetchRanked(cli *lcu.Client, id string) (RankedInfo, bool) {
	paths := []string{
		fmt.Sprintf(lcu.PathRankedStatsBySummoner, url.PathEscape(id)),
	}
	var body []byte
	ok := false
	for _, p := range paths {
		status, b, err := cli.Get(p)
		if err != nil || status < 200 || status >= 300 {
			continue
		}
		body = b
		ok = true
		break
	}
	if !ok {
		return RankedInfo{}, false
	}

	var raw struct {
		Queues                              []rankedQueueEntry          `json:"queues"`
		QueueMap                            map[string]rankedQueueEntry `json:"queueMap"`
		Leagues                             []rankedQueueEntry          `json:"leagues"`
		HighestRankedEntry                  *rankedQueueEntry           `json:"highestRankedEntry"`
		HighestCurrentSeasonReachedTier     string                      `json:"highestCurrentSeasonReachedTier"`
		HighestCurrentSeasonReachedDivision string                      `json:"highestCurrentSeasonReachedDivision"`
	}
	if err := json.Unmarshal(body, &raw); err != nil {
		return RankedInfo{}, false
	}

	find := func(queue string) (string, string, int) {
		if e, ok := raw.QueueMap[queue]; ok && (e.Tier != "" || e.Division != "") {
			return e.Tier, e.Division, e.LeaguePoints
		}
		for _, q := range raw.Queues {
			if q.QueueType == queue && (q.Tier != "" || q.Division != "") {
				return q.Tier, q.Division, q.LeaguePoints
			}
		}
		for _, q := range raw.Leagues {
			if q.QueueType == queue && (q.Tier != "" || q.Division != "") {
				return q.Tier, q.Division, q.LeaguePoints
			}
		}
		return "", "", 0
	}

	soloT, soloD, soloLP := find("RANKED_SOLO_5x5")
	flexT, flexD, flexLP := find("RANKED_FLEX_SR")

	// 兜底：highestRankedEntry / 本赛季最高段位字段
	if soloT == "" && raw.HighestRankedEntry != nil {
		e := raw.HighestRankedEntry
		if e.QueueType == "" || e.QueueType == "RANKED_SOLO_5x5" {
			soloT, soloD, soloLP = e.Tier, e.Division, e.LeaguePoints
		}
	}
	if soloT == "" && raw.HighestCurrentSeasonReachedTier != "" {
		soloT = raw.HighestCurrentSeasonReachedTier
		soloD = raw.HighestCurrentSeasonReachedDivision
	}

	return RankedInfo{
		SummonerID: id,
		Solo:       rankedDisplay(soloT, soloD, soloLP),
		Flex:       rankedDisplay(flexT, flexD, flexLP),
	}, true
}

// rankedDisplay 段位展示：tier 空/未定级 → "未定级"；否则 "黄金 IV 45"（LP>0 才附带）
func rankedDisplay(tier, division string, lp int) string {
	cn := parser.TierCN(tier)
	if cn == "" {
		return unranked
	}
	parts := []string{cn}
	if division != "" {
		parts = append(parts, division)
	}
	if lp > 0 {
		parts = append(parts, strconv.Itoa(lp))
	}
	return strings.Join(parts, " ")
}
