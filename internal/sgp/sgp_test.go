package sgp

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"testing"
)

func TestHost(t *testing.T) {
	cases := map[string]string{
		"GZ100":  "https://gz100-sgp.lol.qq.com:21019",
		"gz100":  "https://gz100-sgp.lol.qq.com:21019",
		"HN1":    "https://hn1-k8s-sgp.lol.qq.com:21019", // Akari 2026-07 k8s 主机
		"BGP2":   "https://bgp2-k8s-sgp.lol.qq.com:21019",
		"XYZ999": "https://xyz999-sgp.lol.qq.com:21019", // 未命中映射按 lowercase 兜底
		"":       "",
		"a/b":    "",
		"x.y":    "",
	}
	for in, want := range cases {
		if got := Host(in); got != want {
			t.Errorf("Host(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestQueueEntryDiv(t *testing.T) {
	if got := (QueueEntry{Rank: "II", Division: "IV"}).Div(); got != "II" {
		t.Errorf("rank 优先: Div() = %q, want II", got)
	}
	if got := (QueueEntry{Rank: "", Division: "III"}).Div(); got != "III" {
		t.Errorf("division 兜底: Div() = %q, want III", got)
	}
}

// newTestServer 构造 SGP leagues-ledge TLS 测试服务器，返回 host 与请求记录指针。
// 注入 srv.Client()（信任自签证书），生产 httpCli 保持完整 TLS 校验。
func newTestServer(t *testing.T, handler http.HandlerFunc) (string, *[]string) {
	t.Helper()
	var requests []string
	srv := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests = append(requests, r.URL.Path+"|"+r.Header.Get("Authorization"))
		handler(w, r)
	}))
	t.Cleanup(srv.Close)
	prev := httpCli
	SetHTTPClient(srv.Client())
	t.Cleanup(func() { SetHTTPClient(prev) })
	u, _ := url.Parse(srv.URL)
	port64, _ := strconv.ParseUint(u.Port(), 10, 16)
	return fmt.Sprintf("https://127.0.0.1:%d", port64), &requests
}

func TestFetchRankedStats_Success(t *testing.T) {
	host, reqs := newTestServer(t, func(w http.ResponseWriter, _ *http.Request) {
		fmt.Fprint(w, `{"queues":[
			{"queueType":"RANKED_SOLO_5x5","tier":"GOLD","rank":"IV","leaguePoints":45,"wins":10,"losses":8},
			{"queueType":"RANKED_FLEX_SR","tier":"PLATINUM","rank":"I","leaguePoints":91,"wins":2,"losses":3}]}`)
	})

	stats, err := FetchRankedStats(host, "PUUID-1", "test-token")
	if err != nil {
		t.Fatal(err)
	}
	if len(stats.Queues) != 2 {
		t.Fatalf("queues = %+v", stats.Queues)
	}
	solo := stats.Queues[0]
	if solo.Tier != "GOLD" || solo.Div() != "IV" || solo.LeaguePoints != 45 {
		t.Fatalf("solo = %+v", solo)
	}
	flex := stats.Queues[1]
	if flex.Tier != "PLATINUM" || flex.Div() != "I" || flex.LeaguePoints != 91 {
		t.Fatalf("flex = %+v", flex)
	}
	// 路径与 Bearer 认证头
	if len(*reqs) != 1 {
		t.Fatalf("requests = %v", *reqs)
	}
	if !strings.HasPrefix((*reqs)[0], "/leagues-ledge/v2/rankedStats/puuid/PUUID-1|Bearer test-token") {
		t.Fatalf("request = %s", (*reqs)[0])
	}
}

func TestFetchRankedStats_EmptyQueues(t *testing.T) {
	host, _ := newTestServer(t, func(w http.ResponseWriter, _ *http.Request) {
		fmt.Fprint(w, `{"queues":[]}`)
	})
	stats, err := FetchRankedStats(host, "P2", "tok")
	if err != nil {
		t.Fatal(err)
	}
	if stats == nil || len(stats.Queues) != 0 {
		t.Fatalf("stats = %+v", stats)
	}
}

func TestFetchRankedStats_HTTPErrors(t *testing.T) {
	host, _ := newTestServer(t, func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(403)
	})
	if _, err := FetchRankedStats(host, "P", "tok"); err == nil || !strings.Contains(err.Error(), "403") {
		t.Fatalf("403 error = %v", err)
	}
}

func TestFetchRankedStats_MissingParams(t *testing.T) {
	if _, err := FetchRankedStats("", "P", "tok"); err == nil {
		t.Fatal("empty host should error")
	}
	if _, err := FetchRankedStats("h", "", "tok"); err == nil {
		t.Fatal("empty puuid should error")
	}
	if _, err := FetchRankedStats("h", "P", ""); err == nil {
		t.Fatal("empty token should error")
	}
}

/* ── 战绩列表（match-history-query SUMMARY） ───────────────────── */

func TestFetchMatchHistory_Success(t *testing.T) {
	var gotPath, gotQuery string
	host, reqs := newTestServer(t, func(w http.ResponseWriter, r *http.Request) {
		gotPath, gotQuery = r.URL.Path, r.URL.RawQuery
		fmt.Fprint(w, `{"games":[{"metadata":{"participants":["P"]},"json":{"gameId":1,"queueId":420}}]}`)
	})

	body, err := FetchMatchHistory(host, "PUUID-1", "ent-token", 20, 10)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(body), `"gameId":1`) {
		t.Fatalf("body = %s", body)
	}
	if gotPath != "/match-history-query/v1/products/lol/player/PUUID-1/SUMMARY" {
		t.Fatalf("path = %s", gotPath)
	}
	if !strings.Contains(gotQuery, "startIndex=20") || !strings.Contains(gotQuery, "count=10") {
		t.Fatalf("query = %s", gotQuery)
	}
	if len(*reqs) != 1 || !strings.HasPrefix((*reqs)[0], gotPath+"|Bearer ent-token") {
		t.Fatalf("requests = %v", *reqs)
	}
}

func TestFetchMatchHistory_Errors(t *testing.T) {
	host, _ := newTestServer(t, func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(404)
	})
	if _, err := FetchMatchHistory(host, "P", "tok", 0, 20); err == nil || !strings.Contains(err.Error(), "404") {
		t.Fatalf("404 error = %v", err)
	}
	if _, err := FetchMatchHistory("", "P", "tok", 0, 20); err == nil {
		t.Fatal("empty host should error")
	}
	if _, err := FetchMatchHistory("h", "", "tok", 0, 20); err == nil {
		t.Fatal("empty puuid should error")
	}
	if _, err := FetchMatchHistory("h", "P", "", 0, 20); err == nil {
		t.Fatal("empty token should error")
	}
}

/* ── SGP 认证凭据（LCU token 端点） ───────────────────────────── */

func TestFetchLeagueSessionToken(t *testing.T) {
	// JSON 字符串形态（LCU 实测形态，service 层单测同夹具）
	get := func(path string) (int, []byte, error) {
		if path != "/lol-league-session/v1/league-session-token" {
			t.Errorf("path = %s", path)
		}
		return 200, []byte(`"tok-abc"`), nil
	}
	tok, err := FetchLeagueSessionToken(get)
	if err != nil || tok != "tok-abc" {
		t.Fatalf("json-string form: tok=%q err=%v", tok, err)
	}

	// 纯文本形态（JWT 直出）
	get = func(string) (int, []byte, error) { return 200, []byte("raw.jwt.token\n"), nil }
	if tok, err = FetchLeagueSessionToken(get); err != nil || tok != "raw.jwt.token" {
		t.Fatalf("raw form: tok=%q err=%v", tok, err)
	}

	// 空响应 / 非 2xx / getter 错误 / nil getter
	get = func(string) (int, []byte, error) { return 200, []byte("  "), nil }
	if _, err = FetchLeagueSessionToken(get); err == nil {
		t.Fatal("empty body should error")
	}
	get = func(string) (int, []byte, error) { return 404, nil, nil }
	if _, err = FetchLeagueSessionToken(get); err == nil || !strings.Contains(err.Error(), "404") {
		t.Fatalf("404 error = %v", err)
	}
	get = func(string) (int, []byte, error) { return 0, nil, fmt.Errorf("conn refused") }
	if _, err = FetchLeagueSessionToken(get); err == nil {
		t.Fatal("getter error should propagate")
	}
	if _, err = FetchLeagueSessionToken(nil); err == nil {
		t.Fatal("nil getter should error")
	}
}

func TestFetchEntitlementsToken(t *testing.T) {
	// 对象形态：取 accessToken 字段
	get := func(path string) (int, []byte, error) {
		if path != "/entitlements/v1/token" {
			t.Errorf("path = %s", path)
		}
		return 200, []byte(`{"accessToken":"ent-token-1","entitlements":["lol_path"]}`), nil
	}
	tok, err := FetchEntitlementsToken(get)
	if err != nil || tok != "ent-token-1" {
		t.Fatalf("object form: tok=%q err=%v", tok, err)
	}

	// 对象缺 accessToken / 非 2xx
	get = func(string) (int, []byte, error) { return 200, []byte(`{"x":1}`), nil }
	if _, err = FetchEntitlementsToken(get); err == nil {
		t.Fatal("missing field should error")
	}
	get = func(string) (int, []byte, error) { return 403, nil, nil }
	if _, err = FetchEntitlementsToken(get); err == nil || !strings.Contains(err.Error(), "403") {
		t.Fatalf("403 error = %v", err)
	}
}
