package lcu

import (
	"context"
	"errors"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"strconv"
	"sync"
	"testing"
	"time"
)

func TestMain(m *testing.M) {
	// 单测静默 slog（monitor 流转测试会产生提交/断开日志）
	slog.SetDefault(slog.New(slog.NewTextHandler(io.Discard, nil)))
	os.Exit(m.Run())
}

func TestParseCurrentSummoner(t *testing.T) {
	st, ok := parseCurrentSummoner([]byte(
		`{"puuid":"pu-1","gameName":"召唤师A","#tag":"x","tagLine":"TAG1","summonerLevel":305,"profileIconId":4562}`))
	if !ok || st.GameName != "召唤师A" || st.TagLine != "TAG1" ||
		st.SummonerLevel != 305 || st.ProfileIconId != 4562 || st.Puuid != "pu-1" {
		t.Fatalf("ok=%v st=%+v", ok, st)
	}

	// 旧版 displayName 兜底
	st, ok = parseCurrentSummoner([]byte(`{"puuid":"p2","displayName":"旧名"}`))
	if !ok || st.GameName != "旧名" {
		t.Fatalf("displayName fallback failed: %+v", st)
	}

	if _, ok := parseCurrentSummoner([]byte(`{"displayName":"无puuid"}`)); ok {
		t.Fatal("missing puuid must fail")
	}
	if _, ok := parseCurrentSummoner([]byte(`bad json`)); ok {
		t.Fatal("bad json must fail")
	}
}

func TestCredentialsEqual(t *testing.T) {
	a := Credentials{PID: 1, Port: 2, Token: "t", PlatformID: "HN1"}
	b := Credentials{PID: 1, Port: 2, Token: "t", PlatformID: "HN2"} // 平台变化不触发重连
	if !a.equal(b) {
		t.Fatal("platform change should not force reconnect")
	}
	c := Credentials{PID: 1, Port: 3, Token: "t"}
	if a.equal(c) {
		t.Fatal("port change must be detected")
	}
}

// newTestMonitor 构造注入 detect/probe 的状态机
func newTestMonitor(detect detectFunc, probe probeFunc) *Monitor {
	m := NewMonitor()
	m.detect = detect
	m.probe = probe
	return m
}

func TestMonitor_ConnectFlow(t *testing.T) {
	creds := &Credentials{PID: 111, Port: 222, Token: "tok", PlatformID: "HN1"}
	probeCalls := 0
	var mu sync.Mutex
	var states []State

	m := newTestMonitor(
		func(string) (*Credentials, error) { return creds, nil },
		func(c Credentials) (ConnStatus, error) {
			mu.Lock()
			probeCalls++
			mu.Unlock()
			return ConnStatus{State: StateConnected, GameName: "测试召唤师", TagLine: "TAG", Puuid: "p"}, nil
		},
	)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	onChange := func(st ConnStatus) {
		mu.Lock()
		states = append(states, st.State)
		mu.Unlock()
	}

	var miss int
	// tick1：检测到 + 探测通过 → 提交 connected
	interval := m.tick(ctx, onChange, nil, &miss)
	if interval != pollInterval {
		t.Fatalf("tick1 interval=%v", interval)
	}
	st := m.Status()
	if st.State != StateConnected || st.GameName != "测试召唤师" {
		t.Fatalf("status=%+v", st)
	}
	if _, ok := m.Client(); !ok {
		t.Fatal("client must be bound after commit")
	}
	if miss != 0 {
		t.Fatalf("miss=%d", miss)
	}

	// tick2：凭据未变、已连接 → 稳定态降频，不再重复探测
	interval = m.tick(ctx, onChange, nil, &miss)
	if interval != pollIntervalStable {
		t.Fatalf("tick2 interval=%v want stable", interval)
	}
	mu.Lock()
	pc := probeCalls
	mu.Unlock()
	if pc != 1 {
		t.Fatalf("probeCalls=%d want 1", pc)
	}

	mu.Lock()
	got := states
	mu.Unlock()
	if len(got) != 1 || got[0] != StateConnected {
		t.Fatalf("states=%v", got)
	}
}

func TestMonitor_ProbeFailureNotCommitted(t *testing.T) {
	creds := &Credentials{PID: 1, Port: 2, Token: "t"}
	m := newTestMonitor(
		func(string) (*Credentials, error) { return creds, nil },
		func(Credentials) (ConnStatus, error) { return ConnStatus{}, errors.New("http not ready") },
	)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	var miss int
	interval := m.tick(ctx, nil, nil, &miss)
	if interval != pollInterval {
		t.Fatalf("interval=%v", interval)
	}
	if m.Status().State != StateDisconnected {
		t.Fatalf("probe failure must not commit: %+v", m.Status())
	}
	if _, ok := m.Client(); ok {
		t.Fatal("client must stay nil on probe failure")
	}
}

func TestMonitor_DisconnectAfterMisses(t *testing.T) {
	creds := &Credentials{PID: 1, Port: 2, Token: "t"}
	var detectNil atomicBool
	m := newTestMonitor(
		func(string) (*Credentials, error) {
			if detectNil.get() {
				return nil, nil
			}
			return creds, nil
		},
		func(Credentials) (ConnStatus, error) {
			return ConnStatus{State: StateConnected}, nil
		},
	)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	var states []State
	var mu sync.Mutex
	onChange := func(st ConnStatus) { mu.Lock(); states = append(states, st.State); mu.Unlock() }

	var miss int
	_ = m.tick(ctx, onChange, nil, &miss) // 连接提交
	if m.Status().State != StateConnected {
		t.Fatal("should be connected")
	}

	detectNil.set(true)
	// missLimit-1 轮无结果：状态保持（防瞬时误断）
	for i := 0; i < missLimit-1; i++ {
		_ = m.tick(ctx, onChange, nil, &miss)
		if m.Status().State != StateConnected {
			t.Fatalf("must stay connected before threshold, round %d", i)
		}
	}
	// 第 missLimit 轮：判定断开
	_ = m.tick(ctx, onChange, nil, &miss)
	if m.Status().State != StateDisconnected {
		t.Fatalf("should disconnect, status=%+v", m.Status())
	}
	if _, ok := m.Client(); ok {
		t.Fatal("client must be cleared on disconnect")
	}

	// 检测恢复 → 重新连接提交
	detectNil.set(false)
	miss = 0
	_ = m.tick(ctx, onChange, nil, &miss)
	if m.Status().State != StateConnected {
		t.Fatalf("should reconnect, status=%+v", m.Status())
	}

	mu.Lock()
	got := states
	mu.Unlock()
	want := []State{StateConnected, StateDisconnected, StateConnected}
	if len(got) != len(want) {
		t.Fatalf("states=%v want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("states=%v want %v", got, want)
		}
	}
}

// TestMonitor_UnauthUpgrade 经 httptest TLS 服务模拟：提交 unauthenticated 后，
// 稳定态轻量复查发现登录完成 → 升级 connected。
func TestMonitor_UnauthUpgrade(t *testing.T) {
	var loggedIn atomicBool
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case PathCurrentSummoner:
			if loggedIn.get() {
				w.WriteHeader(http.StatusOK)
				_, _ = w.Write([]byte(`{"puuid":"p9","gameName":"后登录","tagLine":"T9"}`))
			} else {
				w.WriteHeader(http.StatusNotFound)
			}
		case PathBuildInfo:
			w.WriteHeader(http.StatusOK)
		default:
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	defer srv.Close()
	u, _ := url.Parse(srv.URL)
	port64, _ := strconv.ParseUint(u.Port(), 10, 16)
	creds := &Credentials{PID: 5, Port: uint16(port64), Token: "tk", PlatformID: "HN1"}

	m := newTestMonitor(
		func(string) (*Credentials, error) { return creds, nil },
		probeAndBuild, // 真探测链路走 httptest 服务
	)
	oldInterval := probeInterval
	probeInterval = time.Millisecond
	defer func() { probeInterval = oldInterval }()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	var mu sync.Mutex
	var states []State
	onChange := func(st ConnStatus) { mu.Lock(); states = append(states, st.State); mu.Unlock() }

	var miss int
	_ = m.tick(ctx, onChange, nil, &miss) // probe → 404 → unauthenticated 提交
	if m.Status().State != StateUnauthenticated {
		t.Fatalf("want unauthenticated, got %+v", m.Status())
	}

	// 稳定态复查：仍未登录 → 保持 unauthenticated
	_ = m.tick(ctx, onChange, nil, &miss)
	if m.Status().State != StateUnauthenticated {
		t.Fatalf("want still unauthenticated, got %+v", m.Status())
	}

	// 登录完成 → 升级 connected
	loggedIn.set(true)
	_ = m.tick(ctx, onChange, nil, &miss)
	st := m.Status()
	if st.State != StateConnected || st.GameName != "后登录" {
		t.Fatalf("upgrade failed: %+v", st)
	}

	mu.Lock()
	got := states
	mu.Unlock()
	want := []State{StateUnauthenticated, StateConnected}
	if len(got) != len(want) || got[0] != want[0] || got[1] != want[1] {
		t.Fatalf("states=%v want %v", got, want)
	}
}

// atomicBool 测试辅助
type atomicBool struct {
	mu sync.Mutex
	v  bool
}

func (b *atomicBool) set(v bool) { b.mu.Lock(); b.v = v; b.mu.Unlock() }
func (b *atomicBool) get() bool  { b.mu.Lock(); defer b.mu.Unlock(); return b.v }
