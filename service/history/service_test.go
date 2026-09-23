package history

import (
	"encoding/base64"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/1712872354/lol-assistant/internal/lcu"
	"github.com/1712872354/lol-assistant/internal/sgp"
)

// newTestService 构造指向 httptest TLS 服务器的注入式服务（LCU 自签证书语义一致）
func newTestService(t *testing.T, handler http.Handler, pageSize int) (*Service, *httptest.Server) {
	t.Helper()
	srv := httptest.NewTLSServer(handler)
	t.Cleanup(srv.Close)
	u, err := url.Parse(srv.URL)
	if err != nil {
		t.Fatal(err)
	}
	port64, err := strconv.ParseUint(u.Port(), 10, 16)
	if err != nil {
		t.Fatal(err)
	}
	cli := lcu.NewClient(uint16(port64), "test-token")
	svc := NewWithClient(
		func() (*lcu.Client, bool) { return cli, true },
		func() lcu.ConnStatus {
			return lcu.ConnStatus{
				State:         lcu.StateConnected,
				Puuid:         "PSELF",
				GameName:      "歪比巴卜小宝贝",
				TagLine:       "60021",
				ProfileIconId: 42,
				SummonerLevel: 300,
			}
		},
		pageSize,
	)
	return svc, srv
}

/* ─── 战绩列表 ─────────────────────────────────────────────────── */

func TestGetMatches_PaginationAndParse(t *testing.T) {
	var gotPath, gotQuery, gotAuth string
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotPath, gotQuery, gotAuth = r.URL.Path, r.URL.RawQuery, r.Header.Get("Authorization")
		fmt.Fprint(w, `{"games":{"games":[
			{"gameId":1,"gameCreation":1705329000000,"gameDuration":924,"queueId":2400,
			 "participants":[{"participantId":1,"championId":53,
			 "stats":{"win":true,"kills":4,"deaths":0,"assists":9}}]}],
			"gameCount":45}}`)
	})
	svc, _ := newTestService(t, handler, 20)

	page, err := svc.GetMatches("PUUID-1", 1)
	if err != nil {
		t.Fatal(err)
	}
	if gotPath != "/lol-match-history/v1/products/lol/PUUID-1/matches" {
		t.Fatalf("path = %s", gotPath)
	}
	if gotQuery != "begIndex=20&endIndex=39" {
		t.Fatalf("query = %s", gotQuery)
	}
	if !strings.HasPrefix(gotAuth, "Basic ") {
		t.Fatalf("auth = %q", gotAuth)
	}
	if page.GameCount != 45 || page.TotalPages != 3 || !page.HasMore || page.Page != 1 || page.PageSize != 20 {
		t.Fatalf("page meta = %+v", page)
	}
	if len(page.Summaries) != 1 || page.Summaries[0].QueueShort != "海斗" || page.Summaries[0].KDA != "Perfect" {
		t.Fatalf("summaries = %+v", page.Summaries)
	}

	// 越过末页：hasMore=false
	end, err := svc.GetMatches("PUUID-1", 3) // beg=60 > gameCount=45
	if err != nil {
		t.Fatal(err)
	}
	if end.HasMore {
		t.Fatalf("page beyond end should have hasMore=false: %+v", end)
	}
}

func TestGetMatches_Errors(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case strings.Contains(r.URL.Path, "HTTP403"):
			w.WriteHeader(403)
		case strings.Contains(r.URL.Path, "HTTP404"):
			w.WriteHeader(404)
		default:
			w.WriteHeader(403)
		}
	})
	svc, _ := newTestService(t, handler, 20)

	if _, err := svc.GetMatches("", 0); err == nil || !strings.Contains(err.Error(), "puuid") {
		t.Fatalf("empty puuid error = %v", err)
	}
	if _, err := svc.GetMatches("X", 0); err == nil || !strings.Contains(err.Error(), "HTTP 403") {
		t.Fatalf("403 error = %v", err)
	}

	// 未连接
	offline := NewWithClient(func() (*lcu.Client, bool) { return nil, false },
		func() lcu.ConnStatus { return lcu.ConnStatus{State: lcu.StateDisconnected} }, 20)
	if _, err := offline.GetMatches("X", 0); err != ErrNotConnected {
		t.Fatalf("offline error = %v", err)
	}
}

/* ─── 召唤师查询 ───────────────────────────────────────────────── */

func TestSearchSummoner_FoundNumericSummonerID(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/lol-summoner/v1/summoners" {
			t.Errorf("path = %s", r.URL.Path)
		}
		if name := r.URL.Query().Get("name"); name != "安静的亚索#CN1" {
			t.Errorf("query name = %q", name)
		}
		// summonerId 以数字形态返回（历史版本兼容）
		fmt.Fprint(w, `{"puuid":"P-1","gameName":"安静的亚索","tagLine":"CN1",
			"profileIconId":29,"summonerLevel":156,"summonerId":999}`)
	})
	svc, _ := newTestService(t, handler, 20)

	res, err := svc.SearchSummoner(" 安静的亚索#CN1 ")
	if err != nil {
		t.Fatal(err)
	}
	if res.Puuid != "P-1" || res.DisplayName != "安静的亚索#CN1" || res.SummonerID != "999" || res.SummonerLevel != 156 {
		t.Fatalf("result = %+v", res)
	}

	if _, err := svc.SearchSummoner("  "); err == nil {
		t.Fatal("blank name should error")
	}
}

func TestSearchSummoner_NotFound(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(404)
	})
	svc, _ := newTestService(t, handler, 20)
	if _, err := svc.SearchSummoner("不存在的人#CN1"); err == nil || !strings.Contains(err.Error(), "未找到") {
		t.Fatalf("404 error = %v", err)
	}
}

func TestGetSelfSummoner_Enrichment(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.Contains(r.URL.Path, "/by-puuid/PSELF") {
			fmt.Fprint(w, `{"summonerId":"SID-9","profileIconId":42,"summonerLevel":310}`)
			return
		}
		w.WriteHeader(404)
	})
	svc, _ := newTestService(t, handler, 20)

	res, err := svc.GetSelfSummoner()
	if err != nil {
		t.Fatal(err)
	}
	if res.DisplayName != "歪比巴卜小宝贝#60021" || res.SummonerID != "SID-9" || res.SummonerLevel != 310 {
		t.Fatalf("self = %+v", res)
	}

	// 未登录
	unauth := NewWithClient(func() (*lcu.Client, bool) { return nil, false },
		func() lcu.ConnStatus { return lcu.ConnStatus{State: lcu.StateUnauthenticated} }, 20)
	if _, err := unauth.GetSelfSummoner(); err == nil || !strings.Contains(err.Error(), "未登录") {
		t.Fatalf("unauth error = %v", err)
	}
}

/* ─── 对局明细 ─────────────────────────────────────────────────── */

func TestGetMatchDetail_Plumbing(t *testing.T) {
	const detail = `{"gameId":8000000001,"gameCreation":1705329000000,"gameDuration":924,"queueId":420,
		"participantIdentities":[
			{"participantId":1,"player":{"puuid":"PSELF","gameName":"歪比","tagLine":"60021","summonerId":"S1"}},
			{"participantId":2,"player":{"puuid":"P2","gameName":"B","tagLine":"0002","summonerId":"S2"}}],
		"participants":[
			{"participantId":1,"teamID":100,"championId":53,"stats":{"win":false,"kills":4,"deaths":11,"assists":20,"goldEarned":11000,"totalDamageDealtToChampions":17000}},
			{"participantId":2,"teamID":200,"championId":22,"stats":{"win":true,"kills":9,"deaths":3,"assists":12,"goldEarned":14000,"totalDamageDealtToChampions":23000}}]}`
	var gotPath string
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotPath = r.URL.Path
		fmt.Fprint(w, detail)
	})
	svc, _ := newTestService(t, handler, 20)

	d, err := svc.GetMatchDetail(8000000001, "PSELF")
	if err != nil {
		t.Fatal(err)
	}
	if gotPath != "/lol-match-history/v1/games/8000000001" {
		t.Fatalf("path = %s", gotPath)
	}
	if d.GameID != 8000000001 || len(d.Teams) != 2 || d.Teams[0].TeamID != 100 {
		t.Fatalf("detail = %+v", d)
	}
	if !d.Teams[0].Players[0].IsSelf || d.Teams[0].Win {
		t.Fatalf("self team mismatch: %+v", d.Teams[0])
	}

	if _, err := svc.GetMatchDetail(0, "PSELF"); err == nil {
		t.Fatal("gameID<=0 should error")
	}
}

/* ─── 段位补数 ─────────────────────────────────────────────────── */

func TestGetPlayersRanked_BestEffort(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case strings.HasSuffix(r.URL.Path, "/S1"):
			fmt.Fprint(w, `{"queueMap":{
				"RANKED_SOLO_5x5":{"tier":"GOLD","division":"IV","leaguePoints":45},
				"RANKED_FLEX_SR":{"tier":"SILVER","division":"II","leaguePoints":10}}}`)
		case strings.HasSuffix(r.URL.Path, "/S2"):
			fmt.Fprint(w, `{"queues":[{"queueType":"RANKED_SOLO_5x5","tier":"","division":"","leaguePoints":0}]}`)
		case strings.HasSuffix(r.URL.Path, "/PUUID-1"):
			fmt.Fprint(w, `{"leagues":[{"queueType":"RANKED_SOLO_5x5","tier":"PLATINUM","division":"II","leaguePoints":12}]}`)
		case strings.HasSuffix(r.URL.Path, "/PUUID-2"):
			fmt.Fprint(w, `{"highestRankedEntry":{"queueType":"RANKED_SOLO_5x5","tier":"EMERALD","division":"IV","leaguePoints":3}}`)
		default:
			w.WriteHeader(500)
		}
	})
	svc, _ := newTestService(t, handler, 20)

	infos, err := svc.GetPlayersRanked([]string{"S1", "S2", "S3", "", "S1", "PUUID-1", "PUUID-2"})
	if err != nil {
		t.Fatal(err)
	}
	byID := map[string]RankedInfo{}
	for _, r := range infos {
		byID[r.SummonerID] = r
	}
	if len(infos) != 4 {
		t.Fatalf("infos = %+v", infos)
	}
	if r := byID["S1"]; r.Solo != "黄金 IV 45" || r.Flex != "白银 II 10" {
		t.Fatalf("S1 ranked = %+v", r)
	}
	if r := byID["S2"]; r.Solo != "未定级" {
		t.Fatalf("S2 ranked = %+v", r)
	}
	if _, ok := byID["S3"]; ok {
		t.Fatal("failed id should be absent")
	}
	if r := byID["PUUID-1"]; r.Solo != "白金 II 12" {
		t.Fatalf("PUUID-1 leagues parse = %+v", r)
	}
	if r := byID["PUUID-2"]; r.Solo != "翡翠 IV 3" {
		t.Fatalf("PUUID-2 highestRankedEntry parse = %+v", r)
	}
}

func TestRankedDisplay(t *testing.T) {
	cases := []struct {
		tier, div string
		lp        int
		want      string
	}{
		{"GOLD", "IV", 45, "黄金 IV 45"},
		{"", "", 0, "未定级"},
		{"UNRANKED", "I", 0, "未定级"},
		{"DIAMOND", "II", 0, "钻石 II"},
	}
	for _, c := range cases {
		if got := rankedDisplay(c.tier, c.div, c.lp); got != c.want {
			t.Errorf("rankedDisplay(%q,%q,%d) = %q, want %q", c.tier, c.div, c.lp, got, c.want)
		}
	}
}

/* ─── 资源代理 ─────────────────────────────────────────────────── */

func TestGetAsset_ChampionWithCache(t *testing.T) {
	var hits atomic.Int32
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/lol-game-data/assets/v1/champion-icons/42.png" {
			w.WriteHeader(404)
			return
		}
		hits.Add(1)
		fmt.Fprint(w, "PNGDATA")
	})
	svc, _ := newTestService(t, handler, 20)

	want := base64.StdEncoding.EncodeToString([]byte("PNGDATA"))
	for i := 0; i < 2; i++ {
		res, err := svc.GetAsset("champion", 42)
		if err != nil {
			t.Fatal(err)
		}
		if res.Data != want || res.Mime != "image/png" || res.Kind != "champion" {
			t.Fatalf("asset = %+v", res)
		}
	}
	if hits.Load() != 1 {
		t.Fatalf("cache miss: server hits = %d, want 1", hits.Load())
	}
}

func TestGetAsset_SpellViaIndex(t *testing.T) {
	var indexHits atomic.Int32
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/lol-game-data/assets/v1/summoner-spells.json":
			indexHits.Add(1)
			fmt.Fprint(w, `[{"id":4,"iconPath":"/lol-game-data/assets/DATA/Spells/Icons2D/SummonerFlash.png"}]`)
		case "/lol-game-data/assets/DATA/Spells/Icons2D/SummonerFlash.png":
			fmt.Fprint(w, "FLASH")
		default:
			w.WriteHeader(404)
		}
	})
	svc, _ := newTestService(t, handler, 20)

	res, err := svc.GetAsset("spell", 4)
	if err != nil {
		t.Fatal(err)
	}
	if res.Data != base64.StdEncoding.EncodeToString([]byte("FLASH")) || res.Mime != "image/png" {
		t.Fatalf("spell asset = %+v", res)
	}
	// 二次调用走字节缓存，索引也命中 TTL 缓存
	if _, err := svc.GetAsset("spell", 4); err != nil {
		t.Fatal(err)
	}
	if indexHits.Load() != 1 {
		t.Fatalf("index fetched %d times, want 1", indexHits.Load())
	}
	if _, err := svc.GetAsset("spell", 999); err == nil {
		t.Fatal("missing spell mapping should error")
	}
}

func TestGetAsset_ItemFallbackAndAugmentFeRoute(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/lol-game-data/assets/v1/items/icons2d/3157.png":
			w.WriteHeader(404) // 直链缺失 → 触发索引回退
		case "/lol-game-data/assets/v1/items.json":
			fmt.Fprint(w, `[{"id":3157,"iconPath":"/lol-game-data/assets/ASSETS/Items/Icons2D/3157.png"}]`)
		case "/lol-game-data/assets/ASSETS/Items/Icons2D/3157.png":
			fmt.Fprint(w, "ITEM")
		case "/lol-game-data/assets/v1/cherry-augments.json":
			fmt.Fprint(w, `[{"id":7018,"augmentSmallImagePath":"/fe/lol-loot/aug_7018.png"}]`)
		case "/fe/lol-loot/aug_7018.png":
			fmt.Fprint(w, "AUG")
		default:
			w.WriteHeader(404)
		}
	})
	svc, _ := newTestService(t, handler, 20)

	item, err := svc.GetAsset("item", 3157)
	if err != nil {
		t.Fatal(err)
	}
	if item.Data != base64.StdEncoding.EncodeToString([]byte("ITEM")) {
		t.Fatalf("item fallback failed: %+v", item)
	}

	aug, err := svc.GetAsset("augment", 7018)
	if err != nil {
		t.Fatal(err)
	}
	if aug.Data != base64.StdEncoding.EncodeToString([]byte("AUG")) || aug.Mime != "image/png" {
		t.Fatalf("augment asset = %+v", aug)
	}

	if _, err := svc.GetAsset("unknown-kind", 1); err == nil {
		t.Fatal("unknown kind should error")
	}
	if _, err := svc.GetAsset("champion", 0); err == nil {
		t.Fatal("id<=0 should error")
	}
}

func TestNormalizeAssetPath(t *testing.T) {
	cases := map[string]string{
		"/lol-game-data/assets/DATA/x.png": "/lol-game-data/assets/DATA/x.png",
		"/lol-game-data/v1/perks/a.png":    "/lol-game-data/assets/v1/perks/a.png",
		"assets/items/i.png":               "/lol-game-data/assets/items/i.png",
		"/fe/lol-loot/aug.png":             "/fe/lol-loot/aug.png",
		"":                                 "",
		"ASSETS/Items/Icons2D/3157.png":    "/lol-game-data/assets/ASSETS/Items/Icons2D/3157.png",
	}
	for in, want := range cases {
		if got := normalizeAssetPath(in); got != want {
			t.Errorf("normalizeAssetPath(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestSetPageSizeBounds(t *testing.T) {
	svc := NewWithClient(func() (*lcu.Client, bool) { return nil, false },
		func() lcu.ConnStatus { return lcu.ConnStatus{} }, 20)
	svc.SetPageSize(10)
	if svc.currentPageSize() != 10 {
		t.Fatalf("pageSize = %d", svc.currentPageSize())
	}
	svc.SetPageSize(3) // 非法值不生效
	if svc.currentPageSize() != 10 {
		t.Fatalf("pageSize after invalid set = %d", svc.currentPageSize())
	}
}

/* ─── 段位补数：SGP 补数层（明细页段位缺失修复） ───────────────── */

const testPuuid = "5e65c58d-5b4a-5936-9104-806bb8443eef"

// newSGPTestService 构造开启 SGP 补数的服务：PlatformId=GZ100 → gz100 SGP host
func newSGPTestService(t *testing.T, handler http.Handler, fetch sgpFetchFn) *Service {
	t.Helper()
	srv := httptest.NewTLSServer(handler)
	t.Cleanup(srv.Close)
	u, err := url.Parse(srv.URL)
	if err != nil {
		t.Fatal(err)
	}
	port64, err := strconv.ParseUint(u.Port(), 10, 16)
	if err != nil {
		t.Fatal(err)
	}
	cli := lcu.NewClient(uint16(port64), "test-token")
	svc := NewWithClient(
		func() (*lcu.Client, bool) { return cli, true },
		func() lcu.ConnStatus {
			return lcu.ConnStatus{State: lcu.StateConnected, Puuid: "PSELF", PlatformId: "GZ100"}
		},
		20,
	)
	svc.SetSGPEnabled(true)
	svc.sgpFetchFn = fetch
	return svc
}

// sgpBaseHandler 模拟国服实测通路：summonerId 可反查 puuid；LCU ranked-stats 对 puuid 返回空段位
func sgpBaseHandler(tokenOK bool) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/lol-summoner/v1/summoners/2001":
			fmt.Fprintf(w, `{"puuid":%q}`, testPuuid)
		case strings.Contains(r.URL.Path, "/ranked-stats/"):
			fmt.Fprint(w, `{"queueMap":{}}`) // LCU 无段位数据（国服他人常态）
		case r.URL.Path == "/lol-league-session/v1/league-session-token":
			if tokenOK {
				fmt.Fprint(w, `"test-league-session-token"`)
				return
			}
			w.WriteHeader(404)
		default:
			w.WriteHeader(404)
		}
	}
}

// sgpCannedQueues 模拟 SGP leagues-ledge 响应：含 JADE/TFT 幽灵队列（应被忽略）
func sgpCannedQueues() *sgp.RankedStats {
	return &sgp.RankedStats{Queues: []sgp.QueueEntry{
		{QueueType: "RANKED_FLEX_SR", Tier: "PLATINUM", Rank: "IV", LeaguePoints: 91},
		{QueueType: "RANKED_SOLO_5x5", Tier: "GOLD", Rank: "IV", LeaguePoints: 45},
		{QueueType: "JADE_RANKED_SOLO_5x5", Tier: "WOOD", Rank: "I"},
		{QueueType: "RANKED_TFT"},
	}}
}

func TestGetPlayersRanked_SGPFillAndDualKey(t *testing.T) {
	var sgpCalls []string
	svc := newSGPTestService(t, sgpBaseHandler(true), func(host, puuid, token string) (*sgp.RankedStats, error) {
		sgpCalls = append(sgpCalls, host+"|"+puuid+"|"+token)
		return sgpCannedQueues(), nil
	})

	// 同一玩家双键入参（summonerId + puuid）→ 单桶查询、双键同数据输出
	infos, err := svc.GetPlayersRanked([]string{"2001", testPuuid})
	if err != nil {
		t.Fatal(err)
	}
	if len(sgpCalls) != 1 {
		t.Fatalf("sgp calls = %d (%v), want 1（按玩家去重后仅查一次）", len(sgpCalls), sgpCalls)
	}
	wantCall := "https://gz100-sgp.lol.qq.com:21019|" + testPuuid + "|test-league-session-token"
	if sgpCalls[0] != wantCall {
		t.Fatalf("sgp call = %q, want %q", sgpCalls[0], wantCall)
	}
	byID := map[string]RankedInfo{}
	for _, r := range infos {
		byID[r.SummonerID] = r
	}
	if len(infos) != 2 {
		t.Fatalf("infos = %+v", infos)
	}
	for _, key := range []string{"2001", testPuuid} {
		r, ok := byID[key]
		if !ok {
			t.Fatalf("missing key %q in %+v", key, infos)
		}
		if r.Solo != "黄金 IV 45" || r.Flex != "白金 IV 91" {
			t.Fatalf("key %q ranked = %+v", key, r)
		}
		if r.Puuid != testPuuid {
			t.Fatalf("key %q puuid = %q", key, r.Puuid)
		}
	}
}

func TestGetPlayersRanked_SGPDisabledNoCall(t *testing.T) {
	var sgpCalls int
	svc := newSGPTestService(t, sgpBaseHandler(true), func(_, _, _ string) (*sgp.RankedStats, error) {
		sgpCalls++
		return sgpCannedQueues(), nil
	})
	svc.SetSGPEnabled(false)

	infos, err := svc.GetPlayersRanked([]string{"2001"})
	if err != nil {
		t.Fatal(err)
	}
	if sgpCalls != 0 {
		t.Fatalf("sgp disabled but called %d times", sgpCalls)
	}
	if len(infos) != 1 || infos[0].Solo != "未定级" {
		t.Fatalf("infos = %+v", infos)
	}
}

func TestGetPlayersRanked_SGPFailSoft(t *testing.T) {
	cases := map[string]struct {
		handler   http.HandlerFunc
		fetchErr  error
		wantCalls int
	}{
		"网络失败静默降级":   {sgpBaseHandler(true), errors.New("sgp timeout"), 1},
		"token不可用跳过": {sgpBaseHandler(false), nil, 0},
	}
	for name, c := range cases {
		var sgpCalls int
		svc := newSGPTestService(t, c.handler, func(_, _, _ string) (*sgp.RankedStats, error) {
			sgpCalls++
			return sgpCannedQueues(), c.fetchErr
		})
		infos, err := svc.GetPlayersRanked([]string{testPuuid})
		if err != nil {
			t.Fatalf("%s: unexpected error: %v", name, err)
		}
		if sgpCalls != c.wantCalls {
			t.Fatalf("%s: sgp calls = %d, want %d", name, sgpCalls, c.wantCalls)
		}
		// SGP 失败 ≠ 玩家不存在：LCU 空段位条目仍输出"未定级"（前端 tierShort 兜底）
		if len(infos) != 1 || infos[0].Solo != "未定级" {
			t.Fatalf("%s: infos = %+v", name, infos)
		}
	}
}

func TestSGPRankedDisplay(t *testing.T) {
	solo, flex := sgpRankedDisplay(sgpCannedQueues())
	if solo != "黄金 IV 45" || flex != "白金 IV 91" {
		t.Fatalf("sgpRankedDisplay = (%q, %q)", solo, flex)
	}
	// 空 queues → 双未定级
	solo, flex = sgpRankedDisplay(&sgp.RankedStats{})
	if solo != "未定级" || flex != "未定级" {
		t.Fatalf("empty = (%q, %q)", solo, flex)
	}
	// 仅 flex 有数据（JADE WOOD 队列必须被忽略）
	solo, flex = sgpRankedDisplay(&sgp.RankedStats{Queues: []sgp.QueueEntry{
		{QueueType: "RANKED_FLEX_SR", Tier: "SILVER", Rank: "II", LeaguePoints: 10},
		{QueueType: "JADE_RANKED_SOLO_5x5", Tier: "WOOD", Rank: "I"},
	}})
	if solo != "未定级" || flex != "白银 II 10" {
		t.Fatalf("flex only = (%q, %q)", solo, flex)
	}
}

func TestIsPuuid(t *testing.T) {
	cases := map[string]bool{
		testPuuid:     true,
		"2001":        false,
		"16262047083": false,
		"PUUID-1":     false, // 过短（测试夹具 id）
		"":            false,
	}
	for in, want := range cases {
		if got := isPuuid(in); got != want {
			t.Errorf("isPuuid(%q) = %v, want %v", in, got, want)
		}
	}
}

/* ─── SGP 优先（段位 + 战绩主源云端，LCU 兜底） ────────────────── */

// rankedCountingHandler 统计 LCU ranked-stats 调用数，返回白银 II 7（用于验证兜底通路）
func rankedCountingHandler(counter *int, tokenEnt bool) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/lol-summoner/v1/summoners/2001":
			fmt.Fprintf(w, `{"puuid":%q}`, testPuuid)
		case r.URL.Path == "/lol-league-session/v1/league-session-token":
			fmt.Fprint(w, `"test-league-session-token"`)
		case r.URL.Path == "/entitlements/v1/token":
			if tokenEnt {
				fmt.Fprint(w, `{"accessToken":"test-ent-token"}`)
				return
			}
			w.WriteHeader(404)
		case strings.Contains(r.URL.Path, "/ranked-stats/"):
			*counter++
			fmt.Fprint(w, `{"queues":[{"queueType":"RANKED_SOLO_5x5","tier":"SILVER","division":"II","leaguePoints":7}]}`)
		case strings.Contains(r.URL.Path, "/lol-match-history/"):
			*counter++
			w.WriteHeader(404)
		default:
			w.WriteHeader(404)
		}
	}
}

func TestGetPlayersRanked_SGPPrioritySkipsLCU(t *testing.T) {
	var lcuCalls, sgpCalls int
	svc := newSGPTestService(t, rankedCountingHandler(&lcuCalls, true), func(_, _, _ string) (*sgp.RankedStats, error) {
		sgpCalls++
		return sgpCannedQueues(), nil
	})

	infos, err := svc.GetPlayersRanked([]string{testPuuid})
	if err != nil {
		t.Fatal(err)
	}
	if sgpCalls != 1 {
		t.Fatalf("sgp calls = %d, want 1", sgpCalls)
	}
	// SGP 优先：命中即采用，不再打 LCU ranked-stats
	if lcuCalls != 0 {
		t.Fatalf("LCU ranked-stats calls = %d, want 0（SGP 命中跳过 LCU）", lcuCalls)
	}
	if len(infos) != 1 || infos[0].Solo != "黄金 IV 45" || infos[0].Flex != "白金 IV 91" {
		t.Fatalf("infos = %+v", infos)
	}
}

func TestGetPlayersRanked_SGPEmptyIsUnrankedFact(t *testing.T) {
	var lcuCalls, sgpCalls int
	svc := newSGPTestService(t, rankedCountingHandler(&lcuCalls, true), func(_, _, _ string) (*sgp.RankedStats, error) {
		sgpCalls++
		return &sgp.RankedStats{}, nil // SGP 200 空 queues = "未定级" 数据事实
	})

	infos, err := svc.GetPlayersRanked([]string{testPuuid})
	if err != nil {
		t.Fatal(err)
	}
	if sgpCalls != 1 || lcuCalls != 0 {
		t.Fatalf("sgp=%d lcu=%d, want 1/0（SGP 200 即权威，不回退）", sgpCalls, lcuCalls)
	}
	if len(infos) != 1 || infos[0].Solo != "未定级" || infos[0].Flex != "未定级" {
		t.Fatalf("infos = %+v", infos)
	}
}

func TestGetPlayersRanked_SGPFallbackToLCU(t *testing.T) {
	var lcuCalls, sgpCalls int
	// tokenEnt=false → 仅 league-session 单凭据，SGP 全败恰好 1 次后回退
	svc := newSGPTestService(t, rankedCountingHandler(&lcuCalls, false), func(_, _, _ string) (*sgp.RankedStats, error) {
		sgpCalls++
		return nil, errors.New("sgp timeout")
	})

	infos, err := svc.GetPlayersRanked([]string{testPuuid})
	if err != nil {
		t.Fatal(err)
	}
	if sgpCalls != 1 || lcuCalls != 1 {
		t.Fatalf("sgp=%d lcu=%d, want 1/1（SGP 失败回退 LCU）", sgpCalls, lcuCalls)
	}
	if len(infos) != 1 || infos[0].Solo != "白银 II 7" {
		t.Fatalf("infos = %+v", infos)
	}
}

// sgpMatchFixture SGP match-history-query SUMMARY 夹具（1 局，本人 puuid）
const sgpMatchFixture = `{"games":[{"metadata":{"participants":["` + testPuuid + `"]},
	"json":{"gameId":900001,"gameCreation":1758000000000,"gameDuration":1200,"queueId":420,"mapId":11,
	"participants":[{"puuid":"` + testPuuid + `","teamId":100,"championId":22,"champLevel":16,
	"spell1Id":7,"spell2Id":6,"kills":6,"deaths":1,"assists":8,"win":true,
	"item0":10,"item1":0,"item2":0,"item3":0,"item4":0,"item5":0,"item6":3340,
	"totalMinionsKilled":205,"neutralMinionsKilled":15,"goldEarned":14200,
	"totalDamageDealtToChampions":18500,"totalHeal":420,
	"perks":{"statPerks":{},"styles":[{"style":8100,"selections":[{"perk":8112}]}]}}]}}]}`

// lcuMatchFixture LCU match-history 夹具（1 局）
const lcuMatchFixture = `{"games":{"games":[{"gameId":800001,"gameCreation":1758000000000,"gameDuration":900,
	"queueId":450,"mapId":11,
	"participants":[{"participantId":1,"teamID":100,"championId":99,"spell1Id":4,"spell2Id":6,
	"stats":{"win":false,"kills":3,"deaths":1,"assists":5,"champLevel":14}}],
	"participantIdentities":[{"participantId":1,"player":{"puuid":"` + testPuuid + `"}}]}],"gameCount":40}}`

// matchCountingHandler 统计 LCU match-history 调用数（tokenEnt 控制 entitlements 凭据可用性）
func matchCountingHandler(counter *int, tokenEnt bool, lcuBody string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.URL.Path == "/lol-league-session/v1/league-session-token":
			fmt.Fprint(w, `"test-league-session-token"`)
		case r.URL.Path == "/entitlements/v1/token":
			if tokenEnt {
				fmt.Fprint(w, `{"accessToken":"test-ent-token"}`)
				return
			}
			w.WriteHeader(404)
		case strings.Contains(r.URL.Path, "/lol-match-history/"):
			*counter++
			if lcuBody == "" {
				w.WriteHeader(404)
				return
			}
			fmt.Fprint(w, lcuBody)
		default:
			w.WriteHeader(404)
		}
	}
}

func TestGetMatches_SGPPriorityEntitlementsFirst(t *testing.T) {
	var lcuCalls int
	var gotTokens []string
	var gotStart, gotCount int
	svc := newSGPTestService(t, matchCountingHandler(&lcuCalls, true, lcuMatchFixture), nil)
	svc.sgpMatchFetchFn = func(host, puuid, token string, startIndex, count int) ([]byte, error) {
		gotTokens = append(gotTokens, token)
		gotStart, gotCount = startIndex, count
		if host != "https://gz100-sgp.lol.qq.com:21019" || puuid != testPuuid {
			t.Errorf("host/puuid = %q %q", host, puuid)
		}
		return []byte(sgpMatchFixture), nil
	}

	page, err := svc.GetMatches(testPuuid, 0)
	if err != nil {
		t.Fatal(err)
	}
	// match-history-query 契约：entitlements 凭据优先
	if len(gotTokens) != 1 || gotTokens[0] != "test-ent-token" {
		t.Fatalf("tokens = %v, want [test-ent-token]", gotTokens)
	}
	if gotStart != 0 || gotCount != 20 {
		t.Fatalf("startIndex/count = %d/%d", gotStart, gotCount)
	}
	if lcuCalls != 0 {
		t.Fatalf("LCU match-history calls = %d, want 0（SGP 命中跳过 LCU）", lcuCalls)
	}
	if len(page.Summaries) != 1 || page.Summaries[0].GameID != 900001 || page.HasMore {
		t.Fatalf("page = %+v", page)
	}
}

func TestGetMatches_SGPTokenRetryThenFallback(t *testing.T) {
	cases := map[string]struct {
		tokenEnt     bool
		fetchErr     error // 非 nil 时两凭据全败
		wantSGPCalls int
		wantLCUCalls int
		wantGameID   int64
	}{
		"entitlements失败换session重试": {true, nil, 2, 0, 900001},
		"SGP全败回退LCU":               {true, errors.New("sgp down"), 2, 1, 800001},
		"单凭据失败直接回退LCU":             {false, errors.New("sgp down"), 1, 1, 800001},
	}
	for name, c := range cases {
		var lcuCalls, sgpCalls int
		svc := newSGPTestService(t, matchCountingHandler(&lcuCalls, c.tokenEnt, lcuMatchFixture), nil)
		svc.sgpMatchFetchFn = func(_, _, token string, _, _ int) ([]byte, error) {
			sgpCalls++
			if c.fetchErr != nil {
				return nil, c.fetchErr
			}
			if token == "test-ent-token" {
				return nil, errors.New("401") // 第一凭据失败 → 应换 session 重试
			}
			return []byte(sgpMatchFixture), nil
		}

		page, err := svc.GetMatches(testPuuid, 0)
		if err != nil {
			t.Fatalf("%s: %v", name, err)
		}
		if sgpCalls != c.wantSGPCalls || lcuCalls != c.wantLCUCalls {
			t.Fatalf("%s: sgp=%d lcu=%d, want %d/%d", name, sgpCalls, lcuCalls, c.wantSGPCalls, c.wantLCUCalls)
		}
		if len(page.Summaries) != 1 || page.Summaries[0].GameID != c.wantGameID {
			t.Fatalf("%s: page = %+v", name, page)
		}
	}
}

func TestGetMatches_SGPPage0EmptyFallbackHidden(t *testing.T) {
	var lcuCalls, sgpCalls int
	svc := newSGPTestService(t, matchCountingHandler(&lcuCalls, true, ""), nil) // LCU 404
	svc.sgpMatchFetchFn = func(_, _, _ string, _, _ int) ([]byte, error) {
		sgpCalls++
		return []byte(`{"games":[]}`), nil
	}

	// 首页空结果回退 LCU 裁决：LCU 404 → "生涯隐藏/无记录" 语义保留
	_, err := svc.GetMatches(testPuuid, 0)
	if err == nil || !strings.Contains(err.Error(), "未找到战绩数据") {
		t.Fatalf("err = %v", err)
	}
	if sgpCalls != 1 || lcuCalls != 1 {
		t.Fatalf("sgp=%d lcu=%d, want 1/1", sgpCalls, lcuCalls)
	}
}

func TestGetMatches_SGPDisabledNoCall(t *testing.T) {
	var lcuCalls, sgpCalls int
	svc := newSGPTestService(t, matchCountingHandler(&lcuCalls, true, lcuMatchFixture), nil)
	svc.sgpMatchFetchFn = func(_, _, _ string, _, _ int) ([]byte, error) {
		sgpCalls++
		return []byte(sgpMatchFixture), nil
	}
	svc.SetSGPEnabled(false)

	page, err := svc.GetMatches(testPuuid, 0)
	if err != nil {
		t.Fatal(err)
	}
	if sgpCalls != 0 || lcuCalls != 1 {
		t.Fatalf("sgp=%d lcu=%d, want 0/1", sgpCalls, lcuCalls)
	}
	if len(page.Summaries) != 1 || page.Summaries[0].GameID != 800001 {
		t.Fatalf("page = %+v", page)
	}
}

/* ─── SGP 凭据缓存（对局页 10 人聚合防 token 风暴打爆 LCU 闸门） ─── */

// TestFetchSGPTokens_Cached 验证同会话内双凭据仅首取，后续命中缓存不再打 LCU。
func TestFetchSGPTokens_Cached(t *testing.T) {
	var sessionHits, entHits atomic.Int32
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/lol-league-session/v1/league-session-token":
			sessionHits.Add(1)
			fmt.Fprint(w, `"sess-tok"`)
		case "/entitlements/v1/token":
			entHits.Add(1)
			fmt.Fprint(w, `{"accessToken":"ent-tok"}`)
		default:
			w.WriteHeader(http.StatusNotFound)
		}
	})
	svc := newSGPTestService(t, handler, nil)
	cli, err := svc.client()
	if err != nil {
		t.Fatal(err)
	}

	t1 := svc.fetchSGPTokens(cli)
	t2 := svc.fetchSGPTokens(cli)
	t3 := svc.fetchSGPTokens(cli)

	if t1.session != "sess-tok" || t1.entitlements != "ent-tok" {
		t.Fatalf("t1 = %+v", t1)
	}
	if t2 != t1 || t3 != t1 {
		t.Fatalf("cache miss: t2=%+v t3=%+v want %+v", t2, t3, t1)
	}
	if s, e := sessionHits.Load(), entHits.Load(); s != 1 || e != 1 {
		t.Fatalf("token hits session=%d ent=%d, want 1/1（缓存后不重复取）", s, e)
	}
}

// TestFetchSGPTokens_FailureNotSticky 验证全败只做短负缓存，过期后重试（不永久卡死 SGP）。
func TestFetchSGPTokens_FailureNotSticky(t *testing.T) {
	oldErr := sgpTokenErrTTL
	sgpTokenErrTTL = 0
	defer func() { sgpTokenErrTTL = oldErr }()

	var hits atomic.Int32
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/lol-league-session/v1/league-session-token" ||
			r.URL.Path == "/entitlements/v1/token" {
			hits.Add(1)
			w.WriteHeader(http.StatusInternalServerError)
		}
	})
	svc := newSGPTestService(t, handler, nil)
	cli, err := svc.client()
	if err != nil {
		t.Fatal(err)
	}

	if tok := svc.fetchSGPTokens(cli); tok.session != "" || tok.entitlements != "" {
		t.Fatalf("want empty tokens on failure, got %+v", tok)
	}
	first := hits.Load()
	if first != 2 {
		t.Fatalf("hits = %d, want 2", first)
	}
	// errTTL=0 → 立即过期，应重试而非命中失败缓存
	if tok := svc.fetchSGPTokens(cli); tok.session != "" {
		t.Fatalf("want empty tokens, got %+v", tok)
	}
	if hits.Load() <= first {
		t.Fatalf("hits = %d, want > %d（负缓存过期后应重试）", hits.Load(), first)
	}
}
