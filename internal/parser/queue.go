package parser

import "fmt"

// QueueInfo 队列展示信息（映射移植自 Yuumi match_parser.rs 国服实测表 + 战绩卡片短名）
type QueueInfo struct {
	ID    int    `json:"id"`
	Name  string `json:"name"`  // 完整模式名（明细页头部）
	Short string `json:"short"` // 战绩卡片短名（截图左列样式）
	Map   string `json:"map"`
	Arena bool   `json:"arena"` // 竞技场类：对局按 subteamPlacement 分组而非 teamId
}

// queueTable 队列映射表（国服优先；未收录 ID 走 fallback）
var queueTable = map[int]QueueInfo{
	// 召唤师峡谷
	400: {400, "征召模式", "征召", "召唤师峡谷", false},
	420: {420, "排位单双排", "单双", "召唤师峡谷", false},
	430: {430, "匹配模式", "匹配", "召唤师峡谷", false},
	440: {440, "排位灵活组排", "灵活", "召唤师峡谷", false},
	480: {480, "快速模式", "快速", "召唤师峡谷", false},
	490: {490, "快速模式", "快速", "召唤师峡谷", false},
	// 嚎哭深渊 / 海克斯大乱斗（国服短名"海斗"）
	450:  {450, "极地大乱斗", "大乱斗", "嚎哭深渊", false},
	2400: {2400, "海克斯大乱斗", "海斗", "嚎哭深渊", false},
	2450: {2450, "经典海斗", "海斗", "嚎哭深渊", false},
	// 人机
	800: {800, "人机对战", "人机", "召唤师峡谷", false},
	810: {810, "人机对战", "人机", "召唤师峡谷", false},
	820: {820, "人机对战", "人机", "嚎哭深渊", false},
	830: {830, "人机对战", "人机", "召唤师峡谷", false},
	840: {840, "人机对战", "人机", "召唤师峡谷", false},
	850: {850, "人机对战", "人机", "召唤师峡谷", false},
	// 限时 / 特殊模式
	900:  {900, "无限火力", "火力", "召唤师峡谷", false},
	1010: {1010, "随机无限火力", "火力", "嚎哭深渊", false},
	1020: {1020, "克隆模式", "克隆", "召唤师峡谷", false},
	1300: {1300, "极限闪击", "闪击", "极限闪击", false},
	// 斗魂竞技场：多小队淘汰制，按 subteamPlacement 分组
	1700: {1700, "斗魂竞技场", "竞技场", "斗魂竞技场", true},
	1710: {1710, "斗魂竞技场", "竞技场", "斗魂竞技场", true},
	// 捉鬼模式（Swarm）
	1810: {1810, "捉鬼模式", "捉鬼", "捉鬼模式", false},
	1820: {1820, "捉鬼模式", "捉鬼", "捉鬼模式", false},
	1830: {1830, "捉鬼模式", "捉鬼", "捉鬼模式", false},
	1840: {1840, "捉鬼模式", "捉鬼", "捉鬼模式", false},
	// 经典模式 / 自定义
	4300: {4300, "经典模式", "经典", "召唤师峡谷", false},
	4310: {4310, "经典模式", "经典", "召唤师峡谷", false},
	0:    {0, "自定义模式", "自定义", "自定义", false},
}

// LookupQueue 队列 ID → 展示信息（ok=false 表示本地表未收录，调用方可改用外部官方名兜底）
func LookupQueue(queueID int) (QueueInfo, bool) {
	q, ok := queueTable[queueID]
	return q, ok
}

// QueueInfoFor 队列 ID → 展示信息；未收录返回兜底项（短名带 ID 便于反馈修正）
func QueueInfoFor(queueID int) QueueInfo {
	if q, ok := LookupQueue(queueID); ok {
		return q
	}
	return QueueInfo{
		ID:    queueID,
		Name:  fmt.Sprintf("其他模式 %d", queueID),
		Short: fmt.Sprintf("%d", queueID),
		Map:   "未知",
		Arena: false,
	}
}
