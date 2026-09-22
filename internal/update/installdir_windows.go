//go:build windows

package update

import (
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sys/windows/registry"
)

const uninstKeyPath = `Software\Microsoft\Windows\CurrentVersion\Uninstall\LOL助手`

// detectInstallDir 自动识别已安装目录（更新时作为 NSIS /D= 默认路径）。
// 顺序：注册表 InstallLocation → DisplayIcon/UninstallString 父目录 → 当前 exe 所在目录。
func detectInstallDir() string {
	if dir := fromRegistry(registry.LOCAL_MACHINE); dir != "" {
		return dir
	}
	if dir := fromRegistry(registry.CURRENT_USER); dir != "" {
		return dir
	}
	return fromExecutable()
}

func fromRegistry(root registry.Key) string {
	k, err := registry.OpenKey(root, uninstKeyPath, registry.QUERY_VALUE)
	if err != nil {
		return ""
	}
	defer k.Close()

	if dir, _, err := k.GetStringValue("InstallLocation"); err == nil {
		if d := cleanDir(dir); d != "" {
			return d
		}
	}
	// DisplayIcon: ...\LOL助手.exe 或 ...\LOL助手.exe,0
	if icon, _, err := k.GetStringValue("DisplayIcon"); err == nil {
		if d := cleanDir(parentOf(icon)); d != "" {
			return d
		}
	}
	// UninstallString: "...\uninstall.exe"（可含引号）
	if s, _, err := k.GetStringValue("UninstallString"); err == nil {
		if d := cleanDir(parentOf(s)); d != "" {
			return d
		}
	}
	return ""
}

func fromExecutable() string {
	exe, err := os.Executable()
	if err != nil {
		return ""
	}
	if resolved, err := filepath.EvalSymlinks(exe); err == nil {
		exe = resolved
	}
	return cleanDir(filepath.Dir(exe))
}

// parentOf 从带引号/附带参数的 exe/uninstall 字符串取父目录（仅解析，不校验存在）。
// 支持："C:\Program Files\app.exe" /S 、C:\Program Files\app.exe,0
func parentOf(s string) string {
	s = strings.TrimSpace(s)
	// 引号包裹：只取引号内路径，丢弃后续参数
	if strings.HasPrefix(s, `"`) {
		rest := s[1:]
		if end := strings.Index(rest, `"`); end >= 0 {
			s = rest[:end]
		} else {
			s = strings.Trim(rest, `"`)
		}
	} else if i := strings.Index(strings.ToLower(s), ".exe"); i >= 0 {
		// 无引号但含 .exe：截到 .exe 结束（保留空格目录名，丢弃后续参数）
		s = s[:i+4]
	} else if i := strings.IndexAny(s, " \t"); i >= 0 {
		s = s[:i]
	}
	// DisplayIcon 可能带 ,0 图标索引
	if i := strings.LastIndex(strings.ToLower(s), ".exe,"); i >= 0 {
		s = s[:i+4]
	}
	if s == "" {
		return ""
	}
	return strings.TrimRight(filepath.Dir(s), `\/`)
}

func cleanDir(dir string) string {
	dir = sanitizeInstallDir(dir)
	if dir == "" {
		return ""
	}
	if st, err := os.Stat(dir); err != nil || !st.IsDir() {
		return ""
	}
	return dir
}
