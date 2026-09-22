// Package sgp 腾讯 SGP（Service Gateway Proxy）云端数据源（主源，LCU 兜底）。
// 设计原则（开发方案 §4.2）：玩家段位/战绩优先取 SGP 云端（腾讯官方后端直连），
// SGP 失败或整体禁用（config.sgpEnabled=false）时回退 LCU 本地接口；
// SGP 网络失败不等价于"玩家无数据"，调用方须保持 fail-soft 语义。
//
// 端点（契约取自 League Akari 抓包配置，2026-07）：
//   - 段位：GET {host}/leagues-ledge/v2/rankedStats/puuid/{puuid}
//     认证 Bearer {league-session-token}；响应 queues[].{queueType, tier, rank, leaguePoints, wins, losses}
//   - 战绩：GET {host}/match-history-query/v1/products/lol/player/{puuid}/SUMMARY?startIndex=&count=
//     认证 Bearer {entitlements.accessToken}；响应 {games:[{metadata, json}]}（与 LCU 战绩形态不同，
//     由 internal/parser.ParseSGPSummaries 解析）
//
// 注意：SGP 响应用 rank 字段（I/II/III/IV）表示段位小级，与 LCU 的 division 同义不同名。
package sgp

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"time"
)

// sgpPort 腾讯 SGP 网关端口（国服各区统一）
const sgpPort = 21019

// clientPlatform X-Riot-ClientPlatform 请求头（LCU 命令行同源值）
const clientPlatform = "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiDQp9"

// maxBodyBytes SGP 响应体上限
const maxBodyBytes = 16 << 20

// rePlatformID PlatformID 仅允许字母数字下划线连字符
var rePlatformID = regexp.MustCompile(`^[A-Za-z0-9_-]+$`)

// httpCli SGP 请求专用客户端：默认校验证书（国服 SGP 为正式域名）；禁用系统代理防劫持。
// 单元测试可替换为跳过校验的 httptest 客户端。
var httpCli = &http.Client{
	Timeout: 8 * time.Second,
	Transport: &http.Transport{
		Proxy: nil,
	},
}

// SetHTTPClient 供测试注入（生产勿调用）
func SetHTTPClient(c *http.Client) {
	if c != nil {
		httpCli = c
	}
}

// knownHosts PlatformID → SGP 网关完整地址（取自 Akari builtin.ts 2026-07 服务器表）。
// HN1/HN10/BGP2 已迁移 -k8s- 主机；其余为 {region}-sgp 形态。
// 未命中映射按 https://{lowercase(PlatformID)}-sgp.lol.qq.com:21019 兜底（实测有效）。
var knownHosts = map[string]string{
	"GZ100":  "https://gz100-sgp.lol.qq.com:21019",    // 联盟二区
	"HN1":    "https://hn1-k8s-sgp.lol.qq.com:21019",  // 艾欧尼亚（k8s 主机）
	"HN10":   "https://hn10-k8s-sgp.lol.qq.com:21019", // 黑色玫瑰（k8s 主机）
	"TJ100":  "https://tj100-sgp.lol.qq.com:21019",    // 联盟四区
	"TJ101":  "https://tj101-sgp.lol.qq.com:21019",    // 联盟五区
	"NJ100":  "https://nj100-sgp.lol.qq.com:21019",    // 联盟一区
	"CQ100":  "https://cq100-sgp.lol.qq.com:21019",    // 联盟三区
	"BGP2":   "https://bgp2-k8s-sgp.lol.qq.com:21019", // 峡谷之巅（k8s 主机）
	"PBE":    "https://pbe-sgp.lol.qq.com:21019",      // 体验服
	"PREPBE": "https://prepbe-sgp.lol.qq.com:21019",
}

// Host 由 LCU 命令行 PlatformID 推导 SGP 网关地址。
// 优先查 knownHosts；未命中按 https://{lowercase}-sgp.lol.qq.com:21019 兜底。
// platformID 强制字符白名单，防 URL 注入。返回空串表示无法确定区域（调用方应跳过 SGP）。
func Host(platformID string) string {
	platformID = strings.TrimSpace(platformID)
	if platformID == "" || !rePlatformID.MatchString(platformID) {
		return ""
	}
	if h, ok := knownHosts[strings.ToUpper(platformID)]; ok {
		return h
	}
	return fmt.Sprintf("https://%s-sgp.lol.qq.com:%d", strings.ToLower(platformID), sgpPort)
}

// QueueEntry SGP leagues-ledge 段位队列条目
type QueueEntry struct {
	QueueType    string `json:"queueType"`
	Tier         string `json:"tier"`
	Rank         string `json:"rank"` // I/II/III/IV（SGP 专用字段名，等价 LCU division）
	Division     string `json:"division"`
	LeaguePoints int    `json:"leaguePoints"`
	Wins         int    `json:"wins"`
	Losses       int    `json:"losses"`
}

// Div 返回段位小级（rank 优先，division 兜底）
func (e QueueEntry) Div() string {
	if e.Rank != "" {
		return e.Rank
	}
	return e.Division
}

// RankedStats SGP leagues-ledge 段位响应
type RankedStats struct {
	Queues []QueueEntry `json:"queues"`
}

// FetchRankedStats 查询指定 puuid 的段位数据（主凭据 league-session-token）。
// 返回 (stats, nil) 且 queues 为空表示该玩家确实无段位（HTTP 200 正常响应）。
func FetchRankedStats(host, puuid, token string) (*RankedStats, error) {
	if host == "" || puuid == "" || token == "" {
		return nil, fmt.Errorf("sgp: missing host/puuid/token")
	}
	body, err := sgpGet(fmt.Sprintf("%s/leagues-ledge/v2/rankedStats/puuid/%s", host, url.PathEscape(puuid)), token)
	if err != nil {
		return nil, err
	}
	var stats RankedStats
	if err := json.Unmarshal(body, &stats); err != nil {
		return nil, fmt.Errorf("sgp parse failed: %w", err)
	}
	return &stats, nil
}

// FetchMatchHistory 查询指定 puuid 的战绩列表（match-history-query SUMMARY，主凭据
// entitlements.accessToken）。startIndex 为起点（0=最近一场），count 为单页条数。
// 返回原始响应体 {games:[{metadata, json}]}——与 LCU 战绩形态不同，
// 由 internal/parser.ParseSGPSummaries 解析；本函数只负责传输。
func FetchMatchHistory(host, puuid, token string, startIndex, count int) ([]byte, error) {
	if host == "" || puuid == "" || token == "" {
		return nil, fmt.Errorf("sgp: missing host/puuid/token")
	}
	if startIndex < 0 {
		startIndex = 0
	}
	if count <= 0 {
		count = 20
	}
	u := fmt.Sprintf("%s/match-history-query/v1/products/lol/player/%s/SUMMARY?startIndex=%d&count=%d",
		host, url.PathEscape(puuid), startIndex, count)
	return sgpGet(u, token)
}

// sgpGet SGP GET 请求公共路径：Bearer 认证 + ClientPlatform 头，非 2xx 返回错误。
func sgpGet(url, token string) ([]byte, error) {
	return sgpGetContext(context.Background(), url, token)
}

func sgpGetContext(ctx context.Context, url, token string) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("X-Riot-ClientPlatform", clientPlatform)

	resp, err := httpCli.Do(req)
	if err != nil {
		return nil, fmt.Errorf("sgp request failed: %w", err)
	}
	defer resp.Body.Close()
	body, rerr := io.ReadAll(io.LimitReader(resp.Body, maxBodyBytes))
	if rerr != nil {
		return nil, fmt.Errorf("sgp read body: %w", rerr)
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("sgp http %d", resp.StatusCode)
	}
	return body, nil
}

/* ── SGP 认证凭据（LCU 端点取数；注入 getter 保持本包零 LCU 依赖） ── */

// lcuPath* token 端点（与 internal/lcu/endpoints.go 同值字面量，避免反向依赖）
const (
	lcuPathLeagueSessionToken = "/lol-league-session/v1/league-session-token"
	lcuPathEntitlementsToken  = "/entitlements/v1/token"
)

// LCUGetter LCU GET 函数签名（注入 lcu.Client.Get 方法值；测试注入假实现）
type LCUGetter func(path string) (status int, body []byte, err error)

// FetchLeagueSessionToken 获取 SGP 主认证凭据。
// 响应形态兼容 JSON 字符串（"…"）与纯文本 token；空响应视为不可用。
func FetchLeagueSessionToken(get LCUGetter) (string, error) {
	return fetchLCUToken(get, lcuPathLeagueSessionToken, "")
}

// FetchEntitlementsToken 获取 SGP 兜底认证凭据（entitlements 响应的 accessToken 字段）。
func FetchEntitlementsToken(get LCUGetter) (string, error) {
	return fetchLCUToken(get, lcuPathEntitlementsToken, "accessToken")
}

// fetchLCUToken LCU token 端点取数：HTTP 非 2xx 或空 token 均返回错误（调用方降级处理）。
// jsonField 非空时按对象字段提取，否则兼容 JSON 字符串/纯文本。
func fetchLCUToken(get LCUGetter, path, jsonField string) (string, error) {
	if get == nil {
		return "", fmt.Errorf("sgp: nil LCU getter")
	}
	status, body, err := get(path)
	if err != nil {
		return "", fmt.Errorf("sgp token %s: %w", path, err)
	}
	if status < 200 || status >= 300 {
		return "", fmt.Errorf("sgp token %s: http %d", path, status)
	}
	tok := parseTokenBody(body, jsonField)
	if tok == "" {
		return "", fmt.Errorf("sgp token %s: empty token", path)
	}
	return tok, nil
}

// parseTokenBody 解析 token 响应体：对象形态取 jsonField；
// 其余按 JSON 字符串 → 纯文本顺序兼容。
func parseTokenBody(body []byte, jsonField string) string {
	trim := strings.TrimSpace(string(body))
	if trim == "" {
		return ""
	}
	if jsonField != "" && strings.HasPrefix(trim, "{") {
		var obj map[string]any
		if json.Unmarshal([]byte(trim), &obj) == nil {
			if s, ok := obj[jsonField].(string); ok {
				return strings.TrimSpace(s)
			}
		}
		return ""
	}
	var s string
	if json.Unmarshal([]byte(trim), &s) == nil {
		return strings.TrimSpace(s)
	}
	return trim
}
