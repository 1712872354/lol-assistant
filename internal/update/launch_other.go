//go:build !windows

package update

import (
	"os/exec"
	"path/filepath"
)

// detectInstallDir 非 Windows 无注册表安装概念
func detectInstallDir() string { return "" }

// launch 非 Windows 平台直接 exec（无 UAC 概念）；installDir 忽略
func launch(path, installDir string) error {
	cmd := exec.Command(path)
	cmd.Dir = filepath.Dir(path)
	return cmd.Start()
}
