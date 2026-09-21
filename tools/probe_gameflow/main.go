// Command probe_gameflow（只读诊断）：
//  1. /lol-gameflow/v1/gameflow-phase + /lol-gameflow/v1/session 原始 JSON 落盘
//  2. gameflow session 通用解码：顶层/gameData 键名、teamOne/teamTwo 真实条目形状、queueId 位置
//  3. Live Client :2999 playerlist 原始 JSON（对照花名册）
//
// 输出写入 E:\idea\LOL\_gameflow_probe.txt。
package main

import (
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"sort"
	"strings"
	"time"

	"github.com/1712872354/lol-assistant/internal/lcu"
)

const outPath = `E:\idea\LOL\_gameflow_probe.txt`

func main() {
	f, err := os.Create(outPath)
	if err != nil {
		return
	}
	defer f.Close()
	w := func(format string, a ...any) { fmt.Fprintf(f, format+"\n", a...) }

	scan, ok := lcu.ScanLeagueClientUx()
	if !ok {
		w("FATAL: cmdline scan failed")
		return
	}
	httpCli := &http.Client{
		Timeout: 10 * time.Second,
		Transport: &http.Transport{
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true}, //nolint:gosec
			Proxy:           nil,
		},
	}
	lcuBase := fmt.Sprintf("https://127.0.0.1:%d", scan.Port)
	get := func(base, path, auth string) (int, []byte) {
		req, _ := http.NewRequest(http.MethodGet, base+path, nil)
		if auth != "" {
			req.Header.Set("Authorization", auth)
		}
		req.Header.Set("Accept", "application/json")
		resp, err := httpCli.Do(req)
		if err != nil {
			w("  ERR %s: %v", path, err)
			return 0, nil
		}
		b, _ := io.ReadAll(resp.Body)
		_ = resp.Body.Close()
		return resp.StatusCode, b
	}
	getLCU := func(path string) (int, []byte) {
		return get(lcuBase, path, lcu.AuthHeader(scan.Token))
	}
	decode := func(b []byte) map[string]any {
		dec := json.NewDecoder(strings.NewReader(string(b)))
		dec.UseNumber()
		var m map[string]any
		_ = dec.Decode(&m)
		return m
	}
	asStr := func(v any) string {
		switch t := v.(type) {
		case nil:
			return "<nil>"
		case string:
			if t == "" {
				return `""`
			}
			return t
		case json.Number:
			return t.String()
		case bool:
			return fmt.Sprintf("%v", t)
		default:
			return fmt.Sprintf("%v", v)
		}
	}
	dumpKeys := func(title string, m map[string]any) {
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		w("%s keys(%d): %s", title, len(keys), strings.Join(keys, ", "))
	}
	dumpObj := func(title string, m map[string]any) {
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			v := m[k]
			s := asStr(v)
			if len(s) > 160 {
				s = s[:160] + "..."
			}
			kind := fmt.Sprintf("%T", v)
			w("    %-24s %-10s %s", k, kind, s)
		}
	}
	trunc := func(b []byte, n int) string {
		s := string(b)
		if len(s) > n {
			return s[:n] + fmt.Sprintf("...<truncated, total %d bytes>", len(b))
		}
		return s
	}

	w("== [phase] ==")
	st, b := getLCU(lcu.PathGameflowPhase)
	w("status=%d raw=%s", st, strings.TrimSpace(string(b)))

	w("\n== [gameflow session raw] ==")
	st, b = getLCU(lcu.PathGameflowSession)
	w("status=%d len=%d", st, len(b))
	w("raw: %s", trunc(b, 20000))

	sess := decode(b)
	w("\n== [session structure] ==")
	dumpKeys("top", sess)
	w("  queueId(top)=%v", asStr(sess["queueId"]))
	w("  queue(top)=%v", asStr(sess["queue"]))
	gd, _ := sess["gameData"].(map[string]any)
	if gd == nil {
		w("  gameData=<nil or not object>")
	} else {
		dumpKeys("  gameData", gd)
		for _, key := range []string{"queueId", "queueType", "gameMode", "gameType", "gameMutator", "teamIds"} {
			if v, ok := gd[key]; ok {
				w("  gameData.%s = %s", key, asStr(v))
			}
		}
		// 队列 id 全局搜刮（顶层 + gameData 下键名含 queue 的）
		for k, v := range gd {
			if strings.Contains(strings.ToLower(k), "queue") {
				w("  [queue-ish] gameData.%s = %s", k, asStr(v))
			}
		}
		for k, v := range sess {
			if strings.Contains(strings.ToLower(k), "queue") {
				w("  [queue-ish] top.%s = %s", k, asStr(v))
			}
		}
		for _, tk := range []string{"teamOne", "teamTwo"} {
			arr, _ := gd[tk].([]any)
			w("  %s: len=%d", tk, len(arr))
			for i, it := range arr {
				im, _ := it.(map[string]any)
				if im == nil {
					w("    [%d] <%T> %v", i, it, it)
					continue
				}
				w("    [%d] entry:", i)
				dumpObj("", im)
			}
		}
		// participants 万一是别的键名
		for k, v := range gd {
			if arr, ok := v.([]any); ok && len(arr) >= 8 && strings.Contains(strings.ToLower(k), "team") {
				w("  [big-array] gameData.%s len=%d", k, len(arr))
			}
		}
	}

	w("\n== [live playerlist] ==")
	st, b = get("http://127.0.0.1:2999", "/liveclientdata/playerlist", "")
	w("status=%d len=%d", st, len(b))
	w("raw: %s", trunc(b, 12000))
	list := decode(b)
	if arr, ok := list["playerlist"].([]any); ok {
		w("playerlist len=%d", len(arr))
	}
	_, nb := get("http://127.0.0.1:2999", "/liveclientdata/activeplayername", "")
	w("activeplayername: %s", strings.TrimSpace(string(nb)))

	w("\n== [champ-select session raw（若在选人）]==")
	st, b = getLCU(lcu.PathChampSelectSession)
	w("status=%d len=%d", st, len(b))
	w("raw: %s", trunc(b, 4000))

	w("\nDONE")
}
