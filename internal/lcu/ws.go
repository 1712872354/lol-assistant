package lcu

// LCU WAMP WebSocket 层：
//   - 握手：wss://127.0.0.1:{port}/，TLS 跳验证 + Basic riot:<token>（对齐 Yuumi ws.rs / Python ssl=False）
//   - 订阅：[5,"OnJsonApiEvent"]，接收 [8,"OnJsonApiEvent_*",{uri,eventType,data}]
//   - 过滤：仅转发 watchedURIs 命中的事件（endpoints.go）
//   - 节流：/lol-champ-select/v1/session ≥500ms 合并（开发方案 §4.3）
//   - 重连：指数退避 1s→15s；Connect 换凭据时取消旧循环，防僵尸重试

import (
	"context"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

// LcuEvent LCU WS 事件（转发前端 gameinfo:update，前端按 uri 分发）
type LcuEvent struct {
	URI       string          `json:"uri"`
	EventType string          `json:"eventType"`
	Data      json.RawMessage `json:"data"`
}

const (
	// subscribeMsg WAMP 订阅帧（OnJsonApiEvent 全量事件，URI 过滤在本层完成）
	subscribeMsg = `[5,"OnJsonApiEvent"]`

	// champSelectThrottle 选人会话事件节流间隔（≥500ms，开发方案 §4.3）
	champSelectThrottle = 500 * time.Millisecond

	wsBackoffInitial = 1 * time.Second
	wsBackoffMax     = 15 * time.Second
	wsPingInterval   = 30 * time.Second
	wsReadIdle       = 90 * time.Second
	wsHandshake      = 5 * time.Second
	wsReadLimit      = 8 << 20 // 8MB：选人会话事件体较大
)

// parseWampEvent 解析 LCU 事件帧 [8,"OnJsonApiEvent_x",{uri,eventType,data}]（纯函数，可测）
func parseWampEvent(text []byte) (LcuEvent, bool) {
	var arr []json.RawMessage
	if err := json.Unmarshal(text, &arr); err != nil || len(arr) < 3 {
		return LcuEvent{}, false
	}
	var opcode int
	if err := json.Unmarshal(arr[0], &opcode); err != nil || opcode != 8 {
		return LcuEvent{}, false
	}
	var evt LcuEvent
	if err := json.Unmarshal(arr[2], &evt); err != nil {
		return LcuEvent{}, false
	}
	if evt.URI == "" {
		return LcuEvent{}, false
	}
	return evt, true
}

// throttle 帧合并器：窗口内仅放行首帧，中间帧丢弃（时钟可注入，便于测试）
type throttle struct {
	mu       sync.Mutex
	interval time.Duration
	last     time.Time
	now      func() time.Time
}

func newThrottle(interval time.Duration) *throttle {
	return &throttle{interval: interval, now: time.Now}
}

// allow 判断当前帧是否放行
func (t *throttle) allow() bool {
	t.mu.Lock()
	defer t.mu.Unlock()
	now := t.now()
	if now.Sub(t.last) < t.interval {
		return false
	}
	t.last = now
	return true
}

// wsRunner WS 连接管理器：同一时刻至多一个活动连接循环
type wsRunner struct {
	mu     sync.Mutex
	cancel context.CancelFunc
}

// Connect 以新凭据启动连接循环；若存在旧循环先取消（pid/port/token 任一变化即重建）
func (r *wsRunner) Connect(parent context.Context, creds Credentials, onEvent func(LcuEvent)) {
	r.Stop()
	ctx, cancel := context.WithCancel(parent)
	r.mu.Lock()
	r.cancel = cancel
	r.mu.Unlock()
	go r.loop(ctx, creds, onEvent)
}

// Stop 取消当前连接循环（幂等）
func (r *wsRunner) Stop() {
	r.mu.Lock()
	if r.cancel != nil {
		r.cancel()
		r.cancel = nil
	}
	r.mu.Unlock()
}

// loop 连接循环：session 结束后按指数退避重连
func (r *wsRunner) loop(ctx context.Context, creds Credentials, onEvent func(LcuEvent)) {
	backoff := wsBackoffInitial
	for {
		if ctx.Err() != nil {
			return
		}
		established, err := r.session(ctx, creds, onEvent)
		if ctx.Err() != nil {
			return
		}
		if established {
			backoff = wsBackoffInitial // 握手成功过的断开，退避复位
		}
		if err != nil {
			slog.Warn("[ws] session ended, will retry", "port", creds.Port, "err", err, "backoff", backoff)
		}
		select {
		case <-ctx.Done():
			return
		case <-time.After(backoff):
		}
		if backoff < wsBackoffMax {
			backoff *= 2
			if backoff > wsBackoffMax {
				backoff = wsBackoffMax
			}
		}
	}
}

// session 单次连接生命周期：握手 → 订阅 → 读循环。
// established 表示握手+订阅是否成功（用于退避复位判定）。
func (r *wsRunner) session(ctx context.Context, creds Credentials, onEvent func(LcuEvent)) (established bool, err error) {
	dialer := websocket.Dialer{
		TLSClientConfig:   &tls.Config{InsecureSkipVerify: true}, //nolint:gosec // LCU 自签名证书
		Proxy:             nil,                                   // 本机回环禁用系统代理
		HandshakeTimeout:  wsHandshake,
		EnableCompression: false,
	}
	header := http.Header{}
	header.Set("Authorization", AuthHeader(creds.Token))
	header.Set("Content-Type", "application/json")
	header.Set("Accept", "application/json")

	wsURL := fmt.Sprintf("wss://127.0.0.1:%d/", creds.Port)
	conn, _, err := dialer.DialContext(ctx, wsURL, header)
	if err != nil {
		return false, err
	}
	defer conn.Close()
	established = true
	slog.Info("[ws] connected", "port", creds.Port)

	// 订阅全量 JSON API 事件
	if err := conn.WriteMessage(websocket.TextMessage, []byte(subscribeMsg)); err != nil {
		return true, fmt.Errorf("subscribe: %w", err)
	}
	slog.Info("[ws] subscribed", "msg", subscribeMsg)

	conn.SetReadLimit(wsReadLimit)
	_ = conn.SetReadDeadline(time.Now().Add(wsReadIdle))
	conn.SetPongHandler(func(string) error {
		return conn.SetReadDeadline(time.Now().Add(wsReadIdle))
	})

	// ctx 取消时关闭连接，中断阻塞读
	go func() {
		<-ctx.Done()
		_ = conn.Close()
	}()

	// ping 保活：30s 一帧，服务端 pong 由 PongHandler 续期读超时
	pingStop := make(chan struct{})
	defer close(pingStop)
	go func() {
		ticker := time.NewTicker(wsPingInterval)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case <-pingStop:
				return
			case <-ticker.C:
				if err := conn.WriteControl(websocket.PingMessage, nil, time.Now().Add(5*time.Second)); err != nil {
					return
				}
			}
		}
	}()

	sessThrottle := newThrottle(champSelectThrottle)
	for {
		_, msg, err := conn.ReadMessage()
		if err != nil {
			return true, err
		}
		evt, ok := parseWampEvent(msg)
		if !ok || !uriWatched(evt.URI) {
			continue
		}
		// 选人会话每秒多帧，500ms 节流合并（丢中间帧，保最新态）
		if strings.HasPrefix(evt.URI, PathChampSelectSession) && !sessThrottle.allow() {
			continue
		}
		if onEvent != nil {
			onEvent(evt)
		}
	}
}
