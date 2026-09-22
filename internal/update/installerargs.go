package update

import "strings"

// installerArgs 生成 NSIS 静默目录参数。
// /D= 必须为命令行最后一项，且路径两侧不能加引号（空格路径可直接拼接）。
func installerArgs(installDir string) string {
	installDir = strings.TrimSpace(installDir)
	if installDir == "" {
		return ""
	}
	// 去掉引号与末尾分隔符，避免 NSIS 解析失败
	installDir = strings.Trim(installDir, `"`)
	installDir = strings.TrimRight(installDir, `\/`)
	if installDir == "" {
		return ""
	}
	return "/D=" + installDir
}
