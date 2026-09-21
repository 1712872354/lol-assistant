package lcu

// Monitor LCU 连接状态机（M1 实装，开发方案 §4.3）：
//
//	双通道检测（lockfile 注册表/配置目录 主通道 → gopsutil 进程命令行 兜底通道）
//	→ HTTP 就绪探测（/system/v1/builds，≤5 次）防客户端未就绪误报
//	→ 登录态判定（/lol-summoner/v1/current-summoner：200=connected / 404=unauthenticated）
//	→ 状态提交（conn:status 回调 + WS 订阅托管）
//
// 轮询节奏：检测中 2s / 已连接稳定态 5s；双通道连续 3 轮无结果才判定断开（防瞬时误断）。

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sync"
	"time"
)

const (
	pollInterval       = 2 * time.Second
	pollIntervalStable = 5 * time.Second
	missLimit          = 3 // 连续 N 轮双通道均无结果才判定断开
)

// 探测参数（var 以便测试注入短间隔）
var (
	probeMaxRetries = 5
	probeInterval   = 600 * time.Millisecond
)

// Credentials 已验证的 LCU 连接凭据
type Credentials struct {
	PID        int32
	Port       uint16
	Token      string
	PlatformID string
}

// equal 凭据等价判定（pid/port/token 任一变化即需重连；PlatformID 不触发重连）
func (c Credentials) equal(o Credentials) bool {
	return c.PID == o.PID && c.Port == o.Port && c.Token == o.Token
}

// detectFunc / probeFunc 可注入的检测与探测函数（单元测试替换真实环境依赖）
type detectFunc func(clientPath string) (*Credentials, error)
type probeFunc func(creds Credentials) (ConnStatus, error)

// Monitor LCU 连接检测器
type Monitor struct {
	mu         sync.RWMutex
	status     ConnStatus
	client     *Client // 当前会话 HTTP 客户端（断开为 nil）
	creds      *Credentials
	clientPath string // 用户配置的客户端目录（lockfile 通道候选）

	stop     chan struct{}
	stopOnce sync.Once
	ws       wsRunner

	detect detectFunc
	probe  probeFunc
}

// NewMonitor 创建检测器（初始态：未连接；detect/probe 为真实现）
func NewMonitor() *Monitor {
	return &Monitor{
		status: ConnStatus{State: StateDisconnected},
		stop:   make(chan struct{}),
		detect: detectCredentials,
		probe:  probeAndBuild,
	}
}

// SetClientPath 设置用户配置的客户端目录（设置变更时同步）
func (m *Monitor) SetClientPath(p string) {
	m.mu.Lock()
	m.clientPath = p
	m.mu.Unlock()
}

// Status 返回当前连接状态快照
func (m *Monitor) Status() ConnStatus {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return m.status
}

// Client 返回当前会话的 LCU HTTP 客户端；未连接时 ok=false。
// M2/M3 服务层经此发起请求，凭据换代由 Monitor 内部托管。
func (m *Monitor) Client() (*Client, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	if m.client == nil {
		return nil, false
	}
	return m.client, true
}

// Start 启动检测循环：
//
//	onChange — 连接状态变化回调（→ conn:status 事件）
//	onEvent  — LCU WS 事件回调（→ gameinfo:update 事件）
func (m *Monitor) Start(ctx context.Context, onChange func(ConnStatus), onEvent func(LcuEvent)) {
	go func() {
		miss := 0
		for {
			interval := m.tick(ctx, onChange, onEvent, &miss)
			select {
			case <-ctx.Done():
				m.ws.Stop()
				return
			case <-m.stop:
				m.ws.Stop()
				return
			case <-time.After(interval):
			}
		}
	}()
}

// Stop 停止检测循环（幂等）
func (m *Monitor) Stop() {
	m.stopOnce.Do(func() { close(m.stop) })
}

// tick 单轮检测（纯调度逻辑，直接可测），返回下一轮间隔
func (m *Monitor) tick(ctx context.Context, onChange func(ConnStatus), onEvent func(LcuEvent), miss *int) time.Duration {
	m.mu.RLock()
	clientPath := m.clientPath
	cur := m.creds
	curState := m.status.State
	m.mu.RUnlock()

	creds, err := m.detect(clientPath)
	if err != nil {
		slog.Warn("[lcu] detect failed", "err", err)
	}

	// ── 断开路径：双通道连续 missLimit 轮无结果 ──
	if creds == nil {
		*miss++
		if cur != nil && *miss >= missLimit {
			slog.Info("[lcu] disconnected", "misses", *miss)
			m.commitDisconnect(onChange)
		}
		return pollInterval
	}
	*miss = 0

	connected := cur != nil && cur.equal(*creds) &&
		(curState == StateConnected || curState == StateUnauthenticated)

	// ── 稳定态：凭据未变且已提交 ──
	if connected {
		if curState == StateUnauthenticated {
			// 客户端在线但未登录：轻量复查，登录完成后升级 connected
			if st, err := fetchSummonerStatus(*creds); err == nil && st.State == StateConnected {
				m.mu.Lock()
				m.status = st
				m.mu.Unlock()
				slog.Info("[lcu] authenticated", "summoner", st.GameName, "tag", st.TagLine)
				if onChange != nil {
					onChange(st)
				}
			}
			return pollInterval // 登录等待期保持 2s 复查节奏
		}
		return pollIntervalStable
	}

	// ── 新凭据 / 凭据变化 / 尚未提交：就绪探测 + 状态构建（失败不提交，下轮重试）──
	st, err := m.probe(*creds)
	if err != nil {
		slog.Warn("[lcu] probe failed, will retry", "err", err)
		return pollInterval
	}
	m.commitConnected(ctx, creds, st, onChange, onEvent)
	return pollInterval
}

// commitConnected 提交连接状态：客户端绑定 + 状态落库 + 回调 + WS 订阅
func (m *Monitor) commitConnected(ctx context.Context, creds *Credentials, st ConnStatus, onChange func(ConnStatus), onEvent func(LcuEvent)) {
	cli := NewClient(creds.Port, creds.Token)
	m.mu.Lock()
	m.creds = creds
	m.client = cli
	m.status = st
	m.mu.Unlock()

	slog.Info("[lcu] state committed",
		"state", st.State, "pid", creds.PID, "port", creds.Port,
		"platform", creds.PlatformID, "summoner", st.GameName)
	if onChange != nil {
		onChange(st)
	}
	// WS 订阅（换凭据自动取消旧循环）
	m.ws.Connect(ctx, *creds, onEvent)
}

// commitDisconnect 提交断开状态：停止 WS + 清空会话
func (m *Monitor) commitDisconnect(onChange func(ConnStatus)) {
	m.ws.Stop()
	m.mu.Lock()
	m.creds = nil
	m.client = nil
	st := ConnStatus{State: StateDisconnected}
	m.status = st
	m.mu.Unlock()

	slog.Info("[lcu] state committed", "state", st.State)
	if onChange != nil {
		onChange(st)
	}
}

/* ── 真实检测 / 探测实现 ──────────────────────────────────────── */

// detectCredentials 双通道检测：
// 通道 A（主）：注册表 Location / 用户配置目录 → lockfile 解析 + PID 存活校验；
// 通道 B（兜底）：gopsutil 枚举 LeagueClientUx 命令行（覆盖 WeGame 场景）。
// 未找到返回 (nil, nil)，非错误。
func detectCredentials(clientPath string) (*Credentials, error) {
	// 通道 A：lockfile
	if path, info, ok := readLockfile(clientPath); ok {
		creds := &Credentials{
			PID:        info.PID,
			Port:       info.Port,
			Token:      info.Password,
			PlatformID: platformIDForPID(info.PID), // lockfile 不含大区，命令行补充
		}
		slog.Debug("[lcu] lockfile channel hit", "path", path, "pid", info.PID, "port", info.Port)
		return creds, nil
	}
	// 通道 B：进程命令行
	if scan, ok := ScanLeagueClientUx(); ok {
		creds := &Credentials{
			PID:        scan.PID,
			Port:       scan.Port,
			Token:      scan.Token,
			PlatformID: scan.PlatformID,
		}
		slog.Debug("[lcu] cmdline channel hit", "pid", scan.PID, "port", scan.Port,
			"cmdline", SanitizeCmdline(fmt.Sprintf("--app-port=%d --remoting-auth-token=%s", scan.Port, scan.Token)))
		return creds, nil
	}
	return nil, nil
}

// probeAndBuild 就绪探测 + 登录态构建：
//   - GET /system/v1/builds ≤5 次（间隔 600ms），HTTP 2xx 视为 LCU 就绪；
//   - 就绪后拉取 current-summoner 构建完整 ConnStatus。
//
// 失败返回 error，Monitor 不提交状态、下轮重试（对齐 Yuumi probe_lcu_readiness 语义）。
func probeAndBuild(creds Credentials) (ConnStatus, error) {
	cli := NewClient(creds.Port, creds.Token)
	var lastErr error
	for attempt := 1; attempt <= probeMaxRetries; attempt++ {
		status, _, err := cli.Get(PathBuildInfo)
		if err == nil && status >= 200 && status < 300 {
			slog.Info("[lcu] http ready", "port", creds.Port, "attempt", attempt)
			return fetchSummonerStatus(creds)
		}
		if err != nil {
			lastErr = err
		} else {
			lastErr = fmt.Errorf("builds http %d", status)
		}
		if attempt < probeMaxRetries {
			time.Sleep(probeInterval)
		}
	}
	return ConnStatus{}, fmt.Errorf("lcu http not ready after %d probes: %w", probeMaxRetries, lastErr)
}

// fetchSummonerStatus 拉取 current-summoner 并映射登录态：
// 200 且含 puuid → connected；404 → unauthenticated（客户端在、未登录）；其余 → error 下轮重试。
func fetchSummonerStatus(creds Credentials) (ConnStatus, error) {
	cli := NewClient(creds.Port, creds.Token)
	status, body, err := cli.Get(PathCurrentSummoner)
	if err != nil {
		return ConnStatus{}, fmt.Errorf("current-summoner: %w", err)
	}
	switch {
	case status >= 200 && status < 300:
		info, ok := parseCurrentSummoner(body)
		if !ok {
			return ConnStatus{State: StateUnauthenticated, PlatformId: creds.PlatformID}, nil
		}
		info.State = StateConnected
		info.PlatformId = creds.PlatformID
		return info, nil
	case status == 404:
		return ConnStatus{State: StateUnauthenticated, PlatformId: creds.PlatformID}, nil
	default:
		return ConnStatus{}, fmt.Errorf("current-summoner http %d", status)
	}
}

// parseCurrentSummoner 解析 current-summoner 响应（纯函数，可测）。
// 新版 Riot ID 字段 gameName/tagLine；旧版 displayName 兜底；无 puuid 视为无效。
func parseCurrentSummoner(data []byte) (ConnStatus, bool) {
	var raw struct {
		Puuid         string `json:"puuid"`
		GameName      string `json:"gameName"`
		TagLine       string `json:"tagLine"`
		DisplayName   string `json:"displayName"`
		SummonerLevel int    `json:"summonerLevel"`
		ProfileIconId int    `json:"profileIconId"`
	}
	if err := json.Unmarshal(data, &raw); err != nil {
		return ConnStatus{}, false
	}
	if raw.Puuid == "" {
		return ConnStatus{}, false
	}
	name := raw.GameName
	if name == "" {
		name = raw.DisplayName
	}
	return ConnStatus{
		GameName:      name,
		TagLine:       raw.TagLine,
		SummonerLevel: raw.SummonerLevel,
		ProfileIconId: raw.ProfileIconId,
		Puuid:         raw.Puuid,
	}, true
}
