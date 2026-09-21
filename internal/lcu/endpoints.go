package lcu

import "strings"

// 端点契约（开发方案 §6）：M2 战绩页 / M3 对局信息页引用的 LCU 路径与 WS 事件 URI。
// 路径常量集中在此，服务层不得内联拼写，防止白名单与调用点漂移。
const (
	// PathBuildInfo LCU HTTP 就绪探测端点
	PathBuildInfo = "/system/v1/builds"
	// PathCurrentSummoner 当前登录召唤师（登录态判定：200=已登录 / 404=未登录）
	PathCurrentSummoner = "/lol-summoner/v1/current-summoner"
	// PathGameflowPhase 游戏流程阶段（None/Lobby/Matchmaking/ChampSelect/InProgress/EndOfGame...）
	PathGameflowPhase = "/lol-gameflow/v1/gameflow-phase"
	// PathGameflowSession 游戏流程会话详情
	PathGameflowSession = "/lol-gameflow/v1/session"
	// PathChampSelectSession 选人会话（对局信息页阵容数据源）
	PathChampSelectSession = "/lol-champ-select/v1/session"
	// PathMatchHistory 指定 puuid 战绩列表（%s = puuid）
	PathMatchHistory = "/lol-match-history/v1/products/lol/%s/matches"
	// PathMatchHistorySelf 当前登录者战绩列表
	PathMatchHistorySelf = "/lol-match-history/v1/products/lol/current-summoner/matches"
	// PathMatchGameDetail 指定对局完整明细（%d = gameId；返回含 participants + participantIdentities）
	PathMatchGameDetail = "/lol-match-history/v1/games/%d"
	// PathSummonersByName 按 Riot ID / 召唤师名查询（服务层追加 ?name=<escaped>）
	PathSummonersByName = "/lol-summoner/v1/summoners"
	// PathSummonerByPuuid 按 puuid 查询召唤师详情（%s = puuid）
	PathSummonerByPuuid = "/lol-summoner/v1/summoners/by-puuid/%s"
	// PathRankedStats 当前登录者段位数据
	PathRankedStats = "/lol-ranked/v1/current-ranked-stats"
	// PathRankedStatsBySummoner 指定召唤师段位数据（%s = summonerId，战绩明细页段位列数据源）
	PathRankedStatsBySummoner = "/lol-ranked/v1/ranked-stats/%s"
	// PathSummonerByID 按 summonerId 查询召唤师详情（%s = summonerId；SGP 补数时 summonerId → puuid 解析）
	PathSummonerByID = "/lol-summoner/v1/summoners/%s"
	// PathLeagueSessionToken SGP 认证凭据主源（league-session-token，响应为 JSON 字符串）
	PathLeagueSessionToken = "/lol-league-session/v1/league-session-token"
	// PathEntitlementsToken SGP 认证凭据兜底（entitlements accessToken）
	PathEntitlementsToken = "/entitlements/v1/token"
	// PathReadyCheck 匹配确认状态
	PathReadyCheck = "/lol-matchmaking/v1/ready-check"
	// PathLobby 房间会话（Lobby/Matchmaking/ReadyCheck 阶段的成员数据源）
	PathLobby = "/lol-lobby/v2/lobby"

	// ── LCU 游戏资源表与图标（/lol-game-data/ 前缀已在客户端白名单内）──
	// PathGDItems 物品表（[{id, iconPath}]）
	PathGDItems = "/lol-game-data/assets/v1/items.json"
	// PathGDSpells 召唤师技能表（[{id, iconPath}]）
	PathGDSpells = "/lol-game-data/assets/v1/summoner-spells.json"
	// PathGDPerks 符文表（[{id, iconPath}]）
	PathGDPerks = "/lol-game-data/assets/v1/perks.json"
	// PathGDAugments 海克斯强化表（[{id, iconPath, augmentSmallImagePath, ...}]）
	PathGDAugments = "/lol-game-data/assets/v1/cherry-augments.json"
	// PathGDProfileIcon 召唤师头像图标（%d = profileIconId）
	PathGDProfileIcon = "/lol-game-data/assets/v1/profile-icons/%d.jpg"
	// PathGDChampionIcon 英雄头像图标（%d = championId）
	PathGDChampionIcon = "/lol-game-data/assets/v1/champion-icons/%d.png"
	// PathGDItemIcon 物品图标直链（%d = itemId，免查 items.json）
	PathGDItemIcon = "/lol-game-data/assets/v1/items/icons2d/%d.png"
	// PathGDChampionSummary 英雄总表（[{id, alias, name}]，live client 英文名 → id 映射）
	PathGDChampionSummary = "/lol-game-data/assets/v1/champion-summary.json"
)

// Live Client Data API（仅游戏进程中可达，127.0.0.1:2999；M3 对局信息页使用）
const (
	// LiveClientBase Live Client Data API 基址
	LiveClientBase = "https://127.0.0.1:2999"
	// PathLiveActivePlayer 当前玩家完整数据
	PathLiveActivePlayer = "/liveclientdata/activeplayer"
	// PathLivePlayerList 本局 10 名玩家数据
	PathLivePlayerList = "/liveclientdata/playerlist"
	// PathLiveGameStats 对局统计
	PathLiveGameStats = "/liveclientdata/gamestats"
	// PathLiveEvents 对局事件流（击杀/推塔等）
	PathLiveEvents = "/liveclientdata/eventdata"
	// PathLiveActivePlayerName 当前玩家名称
	PathLiveActivePlayerName = "/liveclientdata/activeplayername"
)

// watchedURIs 前端关心的 WS 事件 URI 前缀：
// 对局信息页阶段驱动（gameflow/champ-select/ready-check）+ 登录态刷新（current-summoner）。
var watchedURIs = []string{
	PathGameflowPhase,
	PathGameflowSession,
	PathChampSelectSession,
	PathCurrentSummoner,
	PathReadyCheck,
	PathLobby,
}

// uriWatched 判断 WS 事件 URI 是否命中关注列表（前缀匹配，纯函数可测）
func uriWatched(uri string) bool {
	for _, p := range watchedURIs {
		if strings.HasPrefix(uri, p) {
			return true
		}
	}
	return false
}
