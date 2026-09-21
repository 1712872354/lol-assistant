package lcu

import (
	"errors"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"sync/atomic"
	"testing"
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
