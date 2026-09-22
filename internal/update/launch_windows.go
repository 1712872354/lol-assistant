//go:build windows

package update

import (
	"errors"
	"fmt"
	"path/filepath"

	"golang.org/x/sys/windows"
)

// launch 以管理员权限拉起安装包（ShellExecute runas → UAC 提示）。
// exec.Command 无法提权，NSIS 写 Program Files 时会报 requires elevation。
// installDir 非空时以 NSIS /D= 传入，作为安装目录默认值（必须为最后一参数且不含引号）。
func launch(path, installDir string) error {
	verb, err := windows.UTF16PtrFromString("runas")
	if err != nil {
		return err
	}
	file, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return err
	}
	dir, err := windows.UTF16PtrFromString(filepath.Dir(path))
	if err != nil {
		return err
	}
	var params *uint16
	if s := installerArgs(installDir); s != "" {
		if params, err = windows.UTF16PtrFromString(s); err != nil {
			return err
		}
	}
	err = windows.ShellExecute(0, verb, file, params, dir, windows.SW_SHOWNORMAL)
	if err != nil {
		if errors.Is(err, windows.ERROR_CANCELLED) {
			return fmt.Errorf("已取消管理员授权")
		}
		return fmt.Errorf("启动安装程序失败（需要管理员权限）: %w", err)
	}
	return nil
}
