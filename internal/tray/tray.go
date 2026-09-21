// Package tray 系统托盘（getlantern/systray）。
// 右键菜单：显示主界面 / 退出。
package tray

import (
	_ "embed"
	"log/slog"
	"sync"

	"github.com/getlantern/systray"
)

//go:embed tray.ico
var iconICO []byte

type Handlers struct {
	OnShow func() // 显示并聚焦主窗
	OnQuit func() // 彻底退出应用
}

var (
	started   bool
	mu        sync.Mutex
	handlerMu sync.RWMutex
	handlers  Handlers
)

// Start 启动托盘（幂等）。应在 wails.OnStartup 中调用；systray.Run 跑在独立 goroutine。
func Start(title string, h Handlers) {
	mu.Lock()
	defer mu.Unlock()
	handlerMu.Lock()
	handlers = h
	handlerMu.Unlock()
	if started {
		return
	}
	started = true

	go systray.Run(func() {
		systray.SetIcon(iconICO)
		systray.SetTooltip(title)

		showItem := systray.AddMenuItem("显示主界面", "显示并聚焦主窗口")
		systray.AddSeparator()
		quitItem := systray.AddMenuItem("退出", "退出 LOL助手")

		go func() {
			for {
				select {
				case <-showItem.ClickedCh:
					callShow()
				case <-quitItem.ClickedCh:
					callQuit()
					return
				}
			}
		}()
		slog.Info("tray started")
	}, func() {
		slog.Info("tray exit")
	})
}

// Stop 关闭托盘图标（应用退出前调用）
func Stop() {
	mu.Lock()
	defer mu.Unlock()
	if !started {
		return
	}
	started = false
	systray.Quit()
}

func callShow() {
	handlerMu.RLock()
	h := handlers
	handlerMu.RUnlock()
	if h.OnShow != nil {
		h.OnShow()
	}
}

func callQuit() {
	handlerMu.RLock()
	h := handlers
	handlerMu.RUnlock()
	if h.OnQuit != nil {
		h.OnQuit()
	}
}
