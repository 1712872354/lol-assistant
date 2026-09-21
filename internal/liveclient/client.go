// Package liveclient Live Client Data API 客户端（仅游戏进程内可达，https://127.0.0.1:2999）。
// M3 对局信息页「游戏中/结算」阶段的 10 人数据源；客户端不开或游戏结束时请求失败，
// 调用方 fail-soft 处理。自签名证书跳过校验 + 禁用系统代理 + 2s 短超时。
package liveclient

import (
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"
)

const (
	base          = "https://127.0.0.1:2999"
	pathPlayerLst = "/liveclientdata/playerlist"
	pathActive    = "/liveclientdata/activeplayername"
	reqTimeout    = 2 * time.Second
)

// TeamID 队伍编号。live client 响应随版本浮动：数字 100/200（含浮点字面量）
// 或字符串 "100"/"200"/"BLUE"/"RED"/"ORDER"/"CHAOS"。
type TeamID int

const (
	TeamNone TeamID = 0
	TeamBlue TeamID = 100
	TeamRed  TeamID = 200
)

// UnmarshalJSON 容错解析数字/字符串两种编码（json.Number 同时覆盖 100 与 100.0 字面量）
func (t *TeamID) UnmarshalJSON(b []byte) error {
	var n json.Number
	if err := json.Unmarshal(b, &n); err == nil {
		if f, err := n.Float64(); err == nil {
			*t = normalizeTeam(int(f), "")
			return nil
		}
		*t = normalizeTeam(0, n.String())
		return nil
	}
	var s string
	if err := json.Unmarshal(b, &s); err == nil {
		*t = normalizeTeam(0, s)
		return nil
	}
	*t = TeamNone
	return nil
}

// normalizeTeam 归一化队伍编号（纯函数，可测）：
// 数字 100/200、数字字符串（含小数形式）、"BLUE"/"ORDER"→蓝方、"RED"/"CHAOS"→红方。
func normalizeTeam(n int, s string) TeamID {
	s = strings.TrimSpace(strings.ToUpper(s))
	if n == 0 {
		if f, err := strconv.ParseFloat(s, 64); err == nil {
			n = int(f)
		}
	}
	switch {
	case n == 100, s == "100", s == "BLUE", s == "ORDER":
		return TeamBlue
	case n == 200, s == "200", s == "RED", s == "CHAOS":
		return TeamRed
	}
	return TeamNone
}

// Player playerlist 单条（字段随客户端版本浮动，全部容错取值）
type Player struct {
	SummonerName   string `json:"summonerName"`
	RiotIdGameName string `json:"riotIdGameName"`
	RiotIdTagLine  string `json:"riotIdTagLine"`
	Puuid          string `json:"puuid"`
	ChampionName   string `json:"championName"` // 英文别名（Annie），经 champion-summary 映射 id
	Team           TeamID `json:"team"`
	Level          int    `json:"level"`
}

// Name 解析显示名：riotIdGameName/TagLine 优先（新版本），回落 summonerName（可能是 "name#tag"）
func (p Player) Name() (gameName, tagLine string) {
	if p.RiotIdGameName != "" {
		return p.RiotIdGameName, p.RiotIdTagLine
	}
	if i := strings.IndexByte(p.SummonerName, '#'); i >= 0 {
		return p.SummonerName[:i], p.SummonerName[i+1:]
	}
	return p.SummonerName, ""
}

// Client Live Client HTTP 客户端
type Client struct {
	hc *http.Client
}

// New 创建客户端（TLS 跳自签校验；回环地址禁用系统代理）
func New() *Client {
	return &Client{hc: &http.Client{
		Timeout: reqTimeout,
		Transport: &http.Transport{
			//nolint:gosec // live client 自签名证书
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
			Proxy:           nil,
		},
	}}
}

// getJSON GET + JSON 解码（游戏未开/结束时连接失败，错误上抛给调用方 fail-soft）
func (c *Client) getJSON(path string, out any) error {
	resp, err := c.hc.Get(base + path) //nolint:noctx // 回环短超时，无取消需求
	if err != nil {
		return fmt.Errorf("live client %s: %w", path, err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("live client %s: http %d", path, resp.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, 4<<20))
	if err != nil {
		return fmt.Errorf("live client %s read: %w", path, err)
	}
	if err := json.Unmarshal(body, out); err != nil {
		return fmt.Errorf("live client %s decode: %w", path, err)
	}
	return nil
}

// PlayerList 本局玩家列表（10 人）
func (c *Client) PlayerList() ([]Player, error) {
	var list []Player
	if err := c.getJSON(pathPlayerLst, &list); err != nil {
		return nil, err
	}
	return list, nil
}

// ActivePlayerName 当前玩家名（响应为 JSON 字符串，常为 "name#tag"）
func (c *Client) ActivePlayerName() (string, error) {
	var name string
	if err := c.getJSON(pathActive, &name); err != nil {
		return "", err
	}
	return name, nil
}
