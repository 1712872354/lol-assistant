package update

import (
	"regexp"
	"strings"
)

// reWinAbsPath Windows 绝对盘符路径（允许空格目录名，禁止 UNC / 相对 / 设备路径）
var reWinAbsPath = regexp.MustCompile(`^[A-Za-z]:\\[^<>|"?*\x00-\x1f]*$`)

// installerArgs 生成 NSIS 静默目录参数。
// /D= 必须为命令行最后一项，路径两侧不能加引号（空格路径可直接拼接）。
// 目录经 sanitizeInstallDir 净化：拒绝额外 token（如 " /S"）与非法字符，防参数注入。
func installerArgs(installDir string) string {
	installDir = sanitizeInstallDir(installDir)
	if installDir == "" {
		return ""
	}
	return "/D=" + installDir
}

// sanitizeInstallDir 净化安装目录：仅接受本地绝对路径，拒绝可被 NSIS 拆成开关的注入形态。
func sanitizeInstallDir(dir string) string {
	dir = strings.TrimSpace(strings.Trim(dir, `"`))
	dir = strings.TrimRight(dir, `\/`)
	if dir == "" {
		return ""
	}
	// 拒绝 UNC / 控制符 / 文件名非法字符
	if strings.ContainsAny(dir, "<>|\"?*\x00\r\n\t") {
		return ""
	}
	if strings.HasPrefix(dir, `\\`) || strings.HasPrefix(dir, `//`) {
		return ""
	}
	// 拒绝形如 "C:\evil /S" 的额外命令行 token（空格后接 / 或 -）
	if strings.Contains(dir, " /") || strings.Contains(dir, " -") {
		return ""
	}
	if !reWinAbsPath.MatchString(dir) {
		return ""
	}
	return dir
}
