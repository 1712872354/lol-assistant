// Package logging 提供应用级 slog 初始化：文件日志 + 控制台双写、按天落盘、7 天保留。
// 日志目录与配置同级：%APPDATA%\LOLAssistant\logs\app-YYYYMMDD.log。
package logging

import (
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// retention 日志保留时长（开发方案 §6：7 天）
const retention = 7 * 24 * time.Hour

// Dir 返回日志目录（%APPDATA%\LOLAssistant\logs）
func Dir() string {
	base, err := os.UserConfigDir()
	if err != nil || base == "" {
		base = "."
	}
	return filepath.Join(base, "LOLAssistant", "logs")
}

// Init 初始化全局 slog：Info 级别，同时写入当日日志文件与 stderr。
// 返回的 close 函数用于释放文件句柄（进程退出前调用）。
// 文件打开失败时降级为仅 stderr，不阻塞启动。
func Init() (func(), error) {
	dir := Dir()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo})))
		return func() {}, err
	}
	pruneOld(dir)

	f, err := os.OpenFile(
		filepath.Join(dir, "app-"+time.Now().Format("20060102")+".log"),
		os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644,
	)
	if err != nil {
		slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo})))
		return func() {}, err
	}

	handler := slog.NewTextHandler(io.MultiWriter(f, os.Stderr), &slog.HandlerOptions{Level: slog.LevelInfo})
	slog.SetDefault(slog.New(handler))
	return func() { _ = f.Close() }, nil
}

// pruneOld 删除超过保留期的历史日志（启动时执行一次，失败静默）
func pruneOld(dir string) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	cutoff := time.Now().Add(-retention)
	for _, e := range entries {
		if e.IsDir() || !strings.HasPrefix(e.Name(), "app-") || !strings.HasSuffix(e.Name(), ".log") {
			continue
		}
		info, err := e.Info()
		if err != nil {
			continue
		}
		if info.ModTime().Before(cutoff) {
			_ = os.Remove(filepath.Join(dir, e.Name()))
		}
	}
}
