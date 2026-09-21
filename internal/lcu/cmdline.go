package lcu

// 命令行检测通道（兜底，覆盖 WeGame 等注册表不可靠场景）：
// 枚举 LeagueClientUx 进程，从命令行提取 --app-port / --remoting-auth-token / --rso-platform-id。

import (
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"

	"github.com/shirou/gopsutil/v4/process"
)

// 预编译正则（命令行高频解析，避免重复编译）。
// token/platform 值允许紧跟引号（"--remoting-auth-token=xxx" 形态），捕获组跳过引号取值。
var (
	reAppPort     = regexp.MustCompile(`--app-port=(\d+)`)
	reAuthToken   = regexp.MustCompile(`--remoting-auth-token="?([^\s"']+)`)
	rePlatformID  = regexp.MustCompile(`--rso_platform_id="?([^\s"']+)`)
	leagueProcSet = map[string]bool{"leagueclientux.exe": true, "leagueclientux": true}
)

// ProcessScan 进程扫描结果
type ProcessScan struct {
	PID        int32
	Port       uint16
	Token      string
	PlatformID string
	ExeDir     string
}

// ParseCmdline 从 LeagueClientUx 命令行提取连接参数（纯函数，可测）。
// ok=true 表示 port 与 token 均提取成功；platformID 可为空。
func ParseCmdline(cmd string) (port uint16, token string, platformID string, ok bool) {
	m := reAppPort.FindStringSubmatch(cmd)
	if m == nil {
		return
	}
	p64, err := strconv.ParseUint(m[1], 10, 16)
	if err != nil || p64 == 0 {
		return
	}
	t := reAuthToken.FindStringSubmatch(cmd)
	if t == nil || t[1] == "" {
		return
	}
	port = uint16(p64)
	token = t[1]
	if s := rePlatformID.FindStringSubmatch(cmd); s != nil {
		platformID = s[1]
	}
	ok = true
	return
}

// SanitizeCmdline 日志脱敏：token 打码后输出（凭据不得进日志）
func SanitizeCmdline(cmd string) string {
	return reAuthToken.ReplaceAllString(cmd, "--remoting-auth-token=***")
}

// ScanLeagueClientUx 枚举 LeagueClientUx 进程，返回首个凭据齐备的结果。
// 同名但无凭据的进程（僵尸/无权限）跳过继续扫描，不提前退出。
func ScanLeagueClientUx() (*ProcessScan, bool) {
	pids, err := process.Pids()
	if err != nil {
		return nil, false
	}
	for _, pid := range pids {
		p, err := process.NewProcess(pid)
		if err != nil {
			continue
		}
		name, err := p.Name()
		if err != nil || !leagueProcSet[strings.ToLower(name)] {
			continue
		}
		cmd, _ := p.Cmdline()
		if cmd == "" {
			continue
		}
		port, token, platformID, ok := ParseCmdline(cmd)
		if !ok {
			continue
		}
		scan := &ProcessScan{PID: pid, Port: port, Token: token, PlatformID: platformID}
		if exe, err := p.Exe(); err == nil {
			scan.ExeDir = filepath.Dir(exe)
		}
		return scan, true
	}
	return nil, false
}

// PidAlive 进程存活检查（lockfile 残留校验）
func PidAlive(pid int32) bool {
	ok, err := process.PidExists(pid)
	return err == nil && ok
}

// 大区标识缓存：lockfile 通道不含 --rso_platform_id，需命令行补充；
// 按 PID 缓存避免每轮轮询全量扫描进程。
var (
	platCacheMu  sync.Mutex
	platCachePID int32
	platCacheID  string
)

// platformIDForPID 返回指定客户端进程的大区标识（带缓存；找不到返回空串）
func platformIDForPID(pid int32) string {
	platCacheMu.Lock()
	if platCachePID == pid && platCacheID != "" {
		id := platCacheID
		platCacheMu.Unlock()
		return id
	}
	platCacheMu.Unlock()

	id := scanPlatformID()
	if id != "" {
		platCacheMu.Lock()
		platCachePID, platCacheID = pid, id
		platCacheMu.Unlock()
	}
	return id
}

// scanPlatformID 从 LeagueClientUx 命令行提取 --rso_platform_id（登录大区标识）
func scanPlatformID() string {
	pids, err := process.Pids()
	if err != nil {
		return ""
	}
	for _, pid := range pids {
		p, err := process.NewProcess(pid)
		if err != nil {
			continue
		}
		name, err := p.Name()
		if err != nil || !leagueProcSet[strings.ToLower(name)] {
			continue
		}
		cmd, _ := p.Cmdline()
		if m := rePlatformID.FindStringSubmatch(cmd); m != nil {
			return m[1]
		}
	}
	return ""
}
