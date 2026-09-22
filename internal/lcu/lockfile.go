package lcu

// lockfile 检测通道（主通道）：
// LCU 启动时在安装目录写入 lockfile，格式 `name:pid:port:password:protocol`。
// 安装目录经注册表 HKLM\...\League of Legends\Location 查询，亦可由用户在设置中指定。
//
// 注意：本包仅面向 Windows 平台（项目约束），registry 依赖 golang.org/x/sys/windows。

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"golang.org/x/sys/windows/registry"
)

// LockInfo lockfile 解析结果
type LockInfo struct {
	Name     string // 通常为 LeagueClient
	PID      int32
	Port     uint16
	Password string // 即 LCU Basic 认证 token
	Protocol string // 通常为 https
}

// registryKeyPaths 注册表查询路径（amd64 进程读 64 位视图，WOW6432Node 需显式指定）
var registryKeyPaths = []string{
	`SOFTWARE\WOW6432Node\Riot Games, Inc.\League of Legends`,
	`SOFTWARE\Riot Games, Inc.\League of Legends`,
}

// ParseLockfile 解析 lockfile 内容（纯函数，可测）。
// 格式 name:pid:port:password:protocol；password 可能含冒号，故对前三段与末段精确切分。
// 字段不足 / pid、port 非法时返回 false（lockfile 未就绪的短暂竞态，由下轮轮询重试）。
func ParseLockfile(content string) (LockInfo, bool) {
	s := strings.TrimSpace(content)
	// name:pid:port:password[:protocol] — password 可能含 ':'，按前 3 个 + 可选末段 protocol 切分
	head := strings.SplitN(s, ":", 4)
	if len(head) < 4 {
		return LockInfo{}, false
	}
	name, pidStr, portStr, rest := head[0], head[1], head[2], head[3]
	password := rest
	protocol := ""
	// 若 rest 中还有 ':' 且末段是纯协议名，则最后冒号切 protocol
	if i := strings.LastIndex(rest, ":"); i >= 0 {
		maybeProto := rest[i+1:]
		if maybeProto != "" && !strings.ContainsAny(maybeProto, `/\`) {
			// protocol 通常为 http/https/wss；仅当末段无路径特征时切开
			if strings.EqualFold(maybeProto, "http") || strings.EqualFold(maybeProto, "https") ||
				strings.EqualFold(maybeProto, "wss") || strings.EqualFold(maybeProto, "ws") {
				password = rest[:i]
				protocol = maybeProto
			}
		}
	}
	pid64, err := strconv.ParseInt(pidStr, 10, 32)
	if err != nil || pid64 <= 0 {
		return LockInfo{}, false
	}
	port64, err := strconv.ParseUint(portStr, 10, 16) // 端口无符号 16 位（>32767 合法，实测 LCU 常见 5xxxx）
	if err != nil || port64 == 0 {
		return LockInfo{}, false
	}
	if password == "" {
		return LockInfo{}, false
	}
	return LockInfo{
		Name:     name,
		PID:      int32(pid64),
		Port:     uint16(port64),
		Password: password,
		Protocol: protocol,
	}, true
}

// registryLocations 查询注册表中的客户端安装目录列表
func registryLocations() []string {
	var out []string
	for _, keyPath := range registryKeyPaths {
		k, err := registry.OpenKey(registry.LOCAL_MACHINE, keyPath, registry.QUERY_VALUE)
		if err != nil {
			continue
		}
		loc, _, err := k.GetStringValue("Location")
		_ = k.Close()
		if err == nil && loc != "" {
			out = append(out, loc)
		}
	}
	return out
}

// LockfilePaths 组装候选 lockfile 路径：
// 注册表 Location 及其父目录（兼容安装层级差异）+ 用户配置的客户端目录。
func LockfilePaths(clientPath string) []string {
	var dirs []string
	dirs = append(dirs, registryLocations()...)
	if clientPath != "" {
		dirs = append(dirs, clientPath)
	}
	var paths []string
	for _, d := range dirs {
		d = filepath.Clean(d)
		paths = append(paths, filepath.Join(d, "lockfile"))
		if parent := filepath.Dir(d); parent != d {
			paths = append(paths, filepath.Join(parent, "lockfile"))
		}
	}
	return paths
}

// readLockfile 按候选路径读取并解析首个有效 lockfile。
// pid 对应进程必须存活——LOL 异常退出时 lockfile 残留，不校验会误判在线（Yuumi 回归点）。
func readLockfile(clientPath string) (string, LockInfo, bool) {
	for _, p := range LockfilePaths(clientPath) {
		data, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		info, ok := ParseLockfile(string(data))
		if !ok {
			continue
		}
		if !PidAlive(info.PID) {
			// 残留 lockfile：跳过而非删除（客户端可能正在重启写入）
			continue
		}
		return p, info, true
	}
	return "", LockInfo{}, false
}
