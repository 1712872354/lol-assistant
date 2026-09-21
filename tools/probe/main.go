// Command probe v11（只读）：
//  1. SGP leagues-ledge 原始 queues JSON 深挖（自WOOD 段位的神秘队列）
//  2. LCU /lol-summoner/v1/summoners/{summonerId} 对他人是否可反查 puuid（决定后端双键合并可行性）
// 输出写入 E:\idea\LOL\_ranked_probe2.txt。
package main

import (
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/1712872354/lol-assistant/internal/lcu"
)

const outPath = `E:\idea\LOL\_ranked_probe2.txt`

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
	getLCU := func(path string) (int, []byte) {
		req, _ := http.NewRequest(http.MethodGet, lcuBase+path, nil)
		req.Header.Set("Authorization", lcu.AuthHeader(scan.Token))
		req.Header.Set("Accept", "application/json")
		resp, err := httpCli.Do(req)
		if err != nil {
			return 0, nil
		}
		b, _ := io.ReadAll(resp.Body)
		_ = resp.Body.Close()
		return resp.StatusCode, b
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
			return ""
		case string:
			return t
		case json.Number:
			return t.String()
		default:
			return fmt.Sprintf("%v", v)
		}
	}

	_, b := getLCU("/lol-summoner/v1/current-summoner")
	cur := decode(b)
	selfPuuid := asStr(cur["puuid"])
	w("[self] puuid=%s platformId=%s", selfPuuid, scan.PlatformID)

	// 最近一局 identities：name / puuid / summonerId
	type person struct{ name, puuid, sid string }
	var people []person
	mh := decode(func() []byte { _, bb := getLCU("/lol-match-history/v1/products/lol/" + selfPuuid + "/matches?begIndex=0&endIndex=9"); return bb }())
	gameID := ""
	if games, _ := mh["games"].(map[string]any); games != nil {
		if arr, _ := games["games"].([]any); len(arr) > 0 {
			g0, _ := arr[0].(map[string]any)
			gameID = asStr(g0["gameId"])
		}
	}
	if gameID != "" {
		_, bb := getLCU("/lol-match-history/v1/games/" + gameID)
		det := decode(bb)
		if idents, _ := det["participantIdentities"].([]any); idents != nil {
			for _, it := range idents {
				im, _ := it.(map[string]any)
				pl, _ := im["player"].(map[string]any)
				if pl == nil {
					continue
				}
				people = append(people, person{
					name:  asStr(pl["gameName"]),
					puuid: asStr(pl["puuid"]),
					sid:   asStr(pl["summonerId"]),
				})
			}
		}
	}
	w("[match] gameId=%s players=%d", gameID, len(people))

	// ── 实验 1：LCU 按他人 summonerId 反查 puuid ──
	w("\n==[exp1] LCU /lol-summoner/v1/summoners/{sid} 他人反查 ==")
	for i, p := range people {
		if p.sid == "" {
			w("  p%-2d %-18s sid=<空> 跳过", i, p.name)
			continue
		}
		st, bb := getLCU("/lol-summoner/v1/summoners/" + p.sid)
		m := decode(bb)
		gotPuuid := asStr(m["puuid"])
		match := ""
		if gotPuuid != "" {
			if strings.EqualFold(gotPuuid, p.puuid) {
				match = " [puuid 一致✓]"
			} else {
				match = " [puuid 不一致✗ got=" + gotPuuid + "]"
			}
		}
		display := asStr(m["displayName"])
		if display == "" {
			display = asStr(m["gameName"])
		}
		w("  p%-2d %-18s sid=%-12s status=%-3d displayName=%s%s", i, p.name, p.sid, st, display, match)
	}

	// ── 实验 2：SGP leagues-ledge 原始 queues JSON（自 + 有段位者）──
	w("\n==[exp2] SGP leagues-ledge 原始 queues ==")
	_, sb := getLCU("/lol-league-session/v1/league-session-token")
	sessionToken := strings.Trim(strings.TrimSpace(string(sb)), `"`)
	if sessionToken == "" {
		_, eb := getLCU("/entitlements/v1/token")
		sessionToken = asStr(decode(eb)["accessToken"])
	}
	w("[token] len=%d", len(sessionToken))

	host := "https://" + strings.ToLower(scan.PlatformID) + "-sgp.lol.qq.com:21019"
	sgpGet := func(puuid string) []byte {
		req, _ := http.NewRequest(http.MethodGet, host+"/leagues-ledge/v2/rankedStats/puuid/"+puuid, nil)
		req.Header.Set("Accept", "application/json")
		req.Header.Set("Authorization", "Bearer "+sessionToken)
		req.Header.Set("X-Riot-ClientPlatform", "ew0KCSJwbGF0Zm9ybVR5cGUiOiAiUEMiDQp9")
		resp, err := httpCli.Do(req)
		if err != nil {
			w("  ERR %s: %v", puuid, err)
			return nil
		}
		bb, _ := io.ReadAll(resp.Body)
		_ = resp.Body.Close()
		return bb
	}
	for i, p := range people {
		if i != 6 && i != 7 && i != 9 && !strings.EqualFold(p.puuid, selfPuuid) {
			continue // 只深挖自 + p7(WOOD) + p9(SILVER)
		}
		raw := sgpGet(p.puuid)
		if raw == nil {
			continue
		}
		w("\n  --- p%d %s puuid=%s ---", i, p.name, p.puuid)
		// 原始 JSON 原样落盘（截断保护）
		s := string(raw)
		if len(s) > 3000 {
			s = s[:3000] + "...<truncated>"
		}
		w("  raw: %s", s)
		// 结构化 queues 摘要
		m := decode(raw)
		if qs, ok := m["queues"].([]any); ok {
			for qi, q := range qs {
				qm, _ := q.(map[string]any)
				w("  q%d queueType=%q tier=%q rank=%q division=%q lp=%v wins=%v losses=%v",
					qi, asStr(qm["queueType"]), asStr(qm["tier"]), asStr(qm["rank"]),
					asStr(qm["division"]), qm["leaguePoints"], qm["wins"], qm["losses"])
			}
		}
		// 其余顶层字段键名
		keys := make([]string, 0, len(m))
		for k := range m {
			keys = append(keys, k)
		}
		w("  top-level keys: %v", keys)
	}

	// ── 实验 3：LCU ranked-stats/{sid} 与 /{puuid} 双通道对自的结果 ──
	w("\n==[exp3] LCU ranked-stats 双通道（自）==")
	if len(people) > 6 {
		self := people[6]
		for _, id := range []string{self.sid, self.puuid} {
			st, bb := getLCU("/lol-ranked/v1/ranked-stats/" + id)
			m := decode(bb)
			qm, _ := m["queueMap"].(map[string]any)
			solo, _ := qm["RANKED_SOLO_5x5"].(map[string]any)
			w("  id=%-20s status=%d soloTier=%v soloDiv=%v lp=%v  bodyLen=%d",
				id, st, solo["tier"], solo["division"], solo["leaguePoints"], len(bb))
		}
	}
	w("\nDONE")
}
