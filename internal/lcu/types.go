package lcu

// State LCU 连接状态机取值（与前端 ConnState 字面量保持一致）
type State string

const (
	// StateDisconnected 客户端未启动或已退出
	StateDisconnected State = "disconnected"
	// StateConnected 已连接且当前召唤师可用
	StateConnected State = "connected"
	// StateUnauthenticated 客户端进程在但尚未登录（current-summoner 404）
	StateUnauthenticated State = "unauthenticated"
)

// ConnStatus 连接状态快照，经 conn:status 事件推送前端
type ConnStatus struct {
	State         State  `json:"state"`
	GameName      string `json:"gameName,omitempty"`
	TagLine       string `json:"tagLine,omitempty"`
	PlatformId    string `json:"platformId,omitempty"`
	SummonerLevel int    `json:"summonerLevel,omitempty"`
	ProfileIconId int    `json:"profileIconId,omitempty"`
	Puuid         string `json:"puuid,omitempty"`
}
