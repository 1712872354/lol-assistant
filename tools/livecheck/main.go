// Package main 段位链路 live 复核工具（开发自检，不参与发行包）。
// 与前端绑定同路径调用 service 层：GetSelfSummoner → GetMatches → GetMatchDetail
// → GetPlayersRanked（双键入参），验证明细页段位补数在真实客户端下的输出。
// 客户端未连接时输出提示并正常退出（逻辑验收以单测为准）。
// 用法：客户端登录后 go run ./tools/livecheck ；结果同时落盘 E:\idea\LOL\_livecheck.txt
package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/1712872354/lol-assistant/internal/lcu"
	"github.com/1712872354/lol-assistant/internal/sgp"
	"github.com/1712872354/lol-assistant/service/history"
)

const reportPath = "E:\\idea\\LOL\\_livecheck.txt"

func main() {
	out := &strings.Builder{}
	defer func() {
		_ = os.WriteFile(reportPath, []byte(out.String()), 0o644)
		fmt.Print(out.String())
	}()

	mon := lcu.NewMonitor()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	mon.Start(ctx, func(lcu.ConnStatus) {}, func(lcu.LcuEvent) {})
	defer mon.Stop()

	deadline := time.Now().Add(25 * time.Second)
	for time.Now().Before(deadline) && mon.Status().State != lcu.StateConnected {
		time.Sleep(500 * time.Millisecond)
	}
	st := mon.Status()
	fmt.Fprintf(out, "conn: state=%s platform=%s summoner=%s#%s puuid=%s\n",
		st.State, st.PlatformId, st.GameName, st.TagLine, st.Puuid)
	if st.State != lcu.StateConnected {
		fmt.Fprintln(out, "SKIP: LCU 未连接/未登录，live 复核跳过（逻辑验收以单测为准）")
		return
	}

	svc := history.New(mon, 20, 4)
	svc.SetSGPEnabled(true) // 与 config.sgpEnabled 默认值一致

	self, err := svc.GetSelfSummoner()
	if err != nil {
		fmt.Fprintf(out, "FAIL self: %v\n", err)
		return
	}
	fmt.Fprintf(out, "self: %s summonerId=%s puuid=%s\n", self.DisplayName, self.SummonerID, self.Puuid)

	page, err := svc.GetMatches(self.Puuid, 0)
	if err != nil || len(page.Summaries) == 0 {
		fmt.Fprintf(out, "FAIL matches: err=%v count=%d\n", err, len(page.Summaries))
		return
	}
	gameID := page.Summaries[0].GameID
	fmt.Fprintf(out, "latest match: gameId=%d queue=%s\n", gameID, page.Summaries[0].QueueShort)

	detail, err := svc.GetMatchDetail(gameID, self.Puuid)
	if err != nil {
		fmt.Fprintf(out, "FAIL detail: %v\n", err)
		return
	}

	// 与前端 MatchDetailPanel rankedIds 同策略：每人双键（summonerId + puuid）
	var ids []string
	for _, team := range detail.Teams {
		for _, p := range team.Players {
			if p.SummonerID != "" {
				ids = append(ids, p.SummonerID)
			}
			if p.Puuid != "" {
				ids = append(ids, p.Puuid)
			}
		}
	}

	ranked, err := svc.GetPlayersRanked(ids)
	if err != nil {
		fmt.Fprintf(out, "FAIL ranked: %v\n", err)
		return
	}
	fmt.Fprintf(out, "input ids=%d, ranked entries=%d\n", len(ids), len(ranked))

	filled, unrankedCnt := 0, 0
	for _, r := range ranked {
		real := (r.Solo != "" && r.Solo != "未定级") || (r.Flex != "" && r.Flex != "未定级")
		if real {
			filled++
		} else {
			unrankedCnt++
		}
		fmt.Fprintf(out, "  summonerId=%-14s puuid=%-38s solo=%-12s flex=%s\n",
			r.SummonerID, r.Puuid, r.Solo, r.Flex)
	}
	fmt.Fprintf(out, "result: 真实段位条目=%d, 未定级条目=%d（明细页段位补全验证）\n", filled, unrankedCnt)

	// ── SGP 原始诊断：抽 2 个"未定级"他人直查 leagues-ledge，
	// 区分「玩家真无排位数据」与「SGP 补数通路失败」──
	cli, ok := mon.Client()
	if !ok {
		fmt.Fprintln(out, "sgp diag: client unavailable")
		return
	}
	host := sgp.Host(st.PlatformId)
	tok := ""
	if t, err := sgp.FetchLeagueSessionToken(cli.Get); err == nil {
		tok = t
	}
	if tok == "" {
		if t, err := sgp.FetchEntitlementsToken(cli.Get); err == nil {
			tok = t
		}
	}
	fmt.Fprintf(out, "sgp diag: host=%s tokenLen=%d\n", host, len(tok))
	probed := 0
	seenPuuid := map[string]bool{}
	for _, r := range ranked {
		if probed >= 2 || r.Puuid == "" || r.Puuid == self.Puuid || seenPuuid[r.Puuid] {
			continue
		}
		if (r.Solo == "" || r.Solo == "未定级") && (r.Flex == "" || r.Flex == "未定级") {
			seenPuuid[r.Puuid] = true
			stats, err := sgp.FetchRankedStats(host, r.Puuid, tok)
			if err != nil {
				fmt.Fprintf(out, "sgp diag %s: ERR %v\n", r.Puuid, err)
				probed++
				continue
			}
			fmt.Fprintf(out, "sgp diag %s: queues=%d\n", r.Puuid, len(stats.Queues))
			for _, q := range stats.Queues {
				fmt.Fprintf(out, "    %s tier=%s rank=%s lp=%d\n", q.QueueType, q.Tier, q.Rank, q.LeaguePoints)
			}
			probed++
		}
	}
}
