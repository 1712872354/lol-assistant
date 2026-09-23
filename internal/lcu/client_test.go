package lcu

import (
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"sync/atomic"
	"testing"
	"time"
)

func TestAllowedPath(t *testing.T) {
	allowed := []string{
		"/system/v1/builds",
		"/lol-summoner/v1/current-summoner",
		"/lol-match-history/v1/products/lol/abc/matches?begIndex=0&endIndex=20",
		"/lol-ranked/v1/current-ranked-stats",
		"/lol-gameflow/v1/gameflow-phase",
		"/lol-champ-select/v1/session",
		"/lol-game-data/assets/v1/champion-icons/1.png",
		"/fe/lol-loot/augment_7018.png", // 海克斯强化图标（窄前缀）
	}
	for _, p := range allowed {
		if !AllowedPath(p) {
			t.Errorf("should allow %s", p)
		}
	}

	denied := []string{
		"/internal/debug",
		"/lol-login/v1/session",            // 白名单外前缀
		"/riotclient/ux-commands",          // 白名单外前缀
		"lol-summoner/v1/current-summoner", // 缺前导斜杠
		"",                                 // 空路径
		"/lol-game-data/assets/../../../etc/passwd", // 路径穿越
		"/lol-game-data/assets/..%2f..%2f",
		`/lol-game-data\assets\1.png`,
	}
	for _, p := range denied {
		if AllowedPath(p) {
			t.Errorf("should deny %s", p)
		}
	}
}

func TestClientDo_AuthAndWhitelist(t *testing.T) {
	var hits atomic.Int32
	var gotAuth string
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hits.Add(1)
		gotAuth = r.Header.Get("Authorization")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer srv.Close()

	u, _ := url.Parse(srv.URL)
	port64, _ := strconv.ParseUint(u.Port(), 10, 16)
	cli := NewClient(uint16(port64), "secret-token")

	status, body, err := cli.Get(PathBuildInfo)
	if err != nil || status != http.StatusOK {
		t.Fatalf("status=%d err=%v", status, err)
	}
	if string(body) != `{"ok":true}` {
		t.Fatalf("body=%s", body)
	}
	if gotAuth != AuthHeader("secret-token") {
		t.Fatalf("auth header mismatch: %q", gotAuth)
	}
	if want := "Basic cmlvdDpzZWNyZXQtdG9rZW4="; gotAuth != want { // base64("riot:secret-token")
		t.Fatalf("auth=%q want=%q", gotAuth, want)
	}

	// 白名单拒绝：不得触达服务端
	before := hits.Load()
	_, _, err = cli.Do(http.MethodGet, "/evil/path", nil)
	if !errors.Is(err, ErrPathDenied) {
		t.Fatalf("want ErrPathDenied, got %v", err)
	}
	if hits.Load() != before {
		t.Fatal("denied path must not hit server")
	}
}

func TestClientDo_TransportRetry(t *testing.T) {
	oldDelay := clientRetryDelay
	clientRetryDelay = 0
	defer func() { clientRetryDelay = oldDelay }()

	// 指向无人监听的端口：传输层错误，应重试 clientMaxRetries 次后失败
	cli := &Client{port: 1, token: "t", baseURL: "https://127.0.0.1:1"}
	_, _, err := cli.Get(PathBuildInfo)
	if err == nil {
		t.Fatal("expected transport error")
	}
}

func TestClientDo_HttpStatusNotRetried(t *testing.T) {
	var hits atomic.Int32
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hits.Add(1)
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer srv.Close()

	u, _ := url.Parse(srv.URL)
	port64, _ := strconv.ParseUint(u.Port(), 10, 16)
	cli := NewClient(uint16(port64), "t")

	status, _, err := cli.Get(PathBuildInfo)
	if err != nil || status != http.StatusInternalServerError {
		t.Fatalf("status=%d err=%v", status, err)
	}
	if hits.Load() != 1 {
		t.Fatalf("HTTP 500 must not be retried, hits=%d", hits.Load())
	}
}

// TestClientDo_ConcurrencyCappedAt2 验证 LCU 请求在途并发固定上限为 2：
// SGP 云端不经此闸门（走 internal/sgp 独立 HTTP 客户端）。
func TestClientDo_ConcurrencyCappedAt2(t *testing.T) {
	var inFlight, maxInFlight atomic.Int32
	release := make(chan struct{})
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		n := inFlight.Add(1)
		for {
			m := maxInFlight.Load()
			if n <= m || maxInFlight.CompareAndSwap(m, n) {
				break
			}
		}
		<-release // 阻塞至测试放行，放大在途窗口
		inFlight.Add(-1)
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	u, _ := url.Parse(srv.URL)
	port64, _ := strconv.ParseUint(u.Port(), 10, 16)
	cli := NewClient(uint16(port64), "t")

	const n = 10
	done := make(chan error, n)
	for i := 0; i < n; i++ {
		go func() {
			status, _, err := cli.Get(PathBuildInfo)
			if err != nil {
				done <- err
				return
			}
			if status != http.StatusOK {
				done <- fmt.Errorf("status=%d", status)
				return
			}
			done <- nil
		}()
	}

	// 等待闸门填满（2 个在途 + 排队 8 个），给调度留足时间
	deadline := time.Now().Add(2 * time.Second)
	for inFlight.Load() < maxConcurrent && time.Now().Before(deadline) {
		time.Sleep(5 * time.Millisecond)
	}
	time.Sleep(50 * time.Millisecond) // 若无闸门，10 个应已全部在途
	close(release)
	for i := 0; i < n; i++ {
		if err := <-done; err != nil {
			t.Fatal(err)
		}
	}
	if got := maxInFlight.Load(); got > maxConcurrent {
		t.Fatalf("max in-flight = %d, want <= %d", got, maxConcurrent)
	}
}
