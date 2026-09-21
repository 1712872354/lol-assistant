package gameinfo

import (
	"encoding/json"
	"math"
	"strings"
	"time"

	"github.com/1712872354/lol-assistant/internal/lcu"
	"github.com/1712872354/lol-assistant/service/history"
)

// career 近况补数结果（hidden=true：生涯隐藏或无对局可查）
type career struct {
	recent []RecentMatch
	hidden bool
}

// splitName "gameName#tagLine" 拆分（无 # 时 tag 为空）
func splitName(name string) (game, tag string) {
	name = strings.TrimSpace(name)
	if i := strings.IndexByte(name, '#'); i >= 0 {
		return name[:i], name[i+1:]
	}
	return name, ""
}

// tagEqual tag 比对（任一为空视为一致，兼容无 tag 区与老客户端）
func tagEqual(a, b string) bool {
	if a == "" || b == "" {
		return true
	}
	return strings.EqualFold(a, b)
}

// pickRank 段位取值（未定级不占位，前端显示「未定级」灰标）
func pickRank(rk history.RankedInfo) (solo, flex string) {
	if s := strings.TrimSpace(rk.Solo); s != "" && s != "未定级" {
		solo = s
	}
	if f := strings.TrimSpace(rk.Flex); f != "" && f != "未定级" {
		flex = f
	}
	return solo, flex
}

// careerStats 近况统计。公式与旧前端 hydrateSelf 一致：
// 胜率=近 N 场胜场占比（保留 1 位）；avgKda=(K+A)/max(D,1) 均值（2 位）；rating=胜率/20+avgKda（1 位）。
func careerStats(recent []RecentMatch) (winRate float64, sample int, avgKda float64, rating float64) {
	sample = len(recent)
	if sample == 0 {
		return 0, 0, 0, 0
	}
	wins := 0
	kdaSum := 0.0
	for _, r := range recent {
		if r.Win {
			wins++
		}
		kdaSum += float64(r.Kills+r.Assists) / math.Max(float64(r.Deaths), 1)
	}
	winRate = math.Round(float64(wins)/float64(sample)*1000) / 10
	avgKda = math.Round(kdaSum/float64(sample)*100) / 100
	rating = math.Round((winRate/20+avgKda)*10) / 10
	return winRate, sample, avgKda, rating
}

// sumTeam 汇总队伍区块（slots 恒 5；统计取已填槽均值）
func sumTeam(key, label, sideText, badge, phaseLabel string, slots []PlayerSlot) TeamView {
	n := 0
	wrSum, rSum := 0.0, 0.0
	for _, s := range slots {
		if !s.Filled {
			continue
		}
		n++
		wrSum += s.WinRate
		rSum += s.Rating
	}
	tv := TeamView{
		Key: key, Label: label, SideText: sideText, Badge: badge,
		PlayerCount: n, PhaseLabel: phaseLabel, Slots: slots,
	}
	if n > 0 {
		tv.WinRate = math.Round(wrSum/float64(n)*10) / 10
		tv.CompScore = tv.WinRate
		tv.Rating = int(math.Round(rSum / float64(n) * 10))
	}
	return tv
}

// championIDByName 英文别名/显示名 → championId（champion-summary 索引，10min 缓存）
func (s *Service) championIDByName(cli lcuAPI, name string) int {
	name = strings.ToLower(strings.TrimSpace(name))
	if name == "" {
		return 0
	}
	return s.getChampIndex(cli)[name]
}

// getChampIndex 英雄索引（lower(alias|name) → id）；拉取失败不缓存，沿用旧索引
func (s *Service) getChampIndex(cli lcuAPI) map[string]int {
	s.champMu.Lock()
	defer s.champMu.Unlock()
	if s.champIndex != nil && time.Since(s.champAt) < champIndexTTL {
		return s.champIndex
	}
	m := map[string]int{}
	status, body, err := cli.Get(lcu.PathGDChampionSummary)
	if err == nil && status == 200 {
		var list []champEntry
		if json.Unmarshal(body, &list) == nil {
			for _, e := range list {
				if e.ID <= 0 {
					continue
				}
				if a := strings.ToLower(e.Alias); a != "" {
					m[a] = e.ID
				}
				if n := strings.ToLower(e.Name); n != "" {
					m[n] = e.ID
				}
			}
		}
	}
	if len(m) > 0 {
		s.champIndex = m
		s.champAt = time.Now()
	}
	return m
}
