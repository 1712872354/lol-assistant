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
		// 顺序约束（假死根因之一）：菜单项 + 消费循环必须先于 SetIcon。
		// SetIcon 写临时文件（md5/Stat/WriteFile，杀软可秒级拖慢）；若排在前面，
		// 库的 nativeLoop 已在跑，右键只见空菜单且 ClickedCh 无接收者，
		// systrayMenuItemSelected 走 default 丢弃点击 → 永久假死。
		defer func() {
			if r := recover(); r != nil {
				slog.Error("tray onReady panic", "recover", r)
			}
		}()
		showItem := systray.AddMenuItem("显示主界面", "显示并聚焦主窗口")
		systray.AddSeparator()
		quitItem := systray.AddMenuItem("退出", "退出 LOL助手")
		go consumeClicks(showItem.ClickedCh, quitItem.ClickedCh, callShow, callQuit)
		systray.SetIcon(iconICO)
		systray.SetTooltip(title)
		slog.Info("tray started")
	}, func() {
		slog.Info("tray exit")
	})
}

// Stop 关闭托盘图标（应用退出前调用）。
// systray.Quit 仅 PostMessage(WM_CLOSE)（Windows 实现，非阻塞），quitOnce 保证幂等。
func Stop() {
	mu.Lock()
	defer mu.Unlock()
	if !started {
		return
	}
	started = false
	systray.Quit()
}

// consumeClicks 消费菜单点击：handler 异步执行，消费循环永不阻塞。
// 库侧 ClickedCh 非阻塞投递（select+default）：循环一卡或一 panic，
// 后续点击全部被丢弃 → 托盘“有菜单但点了没反应”。
func consumeClicks(show, quit <-chan struct{}, onShow, onQuit func()) {
	for {
		select {
		case <-show:
			safeDispatch(onShow)
		case <-quit:
			safeDispatch(onQuit)
			return
		}
	}
}

// safeDispatch 在独立 goroutine 执行 handler；panic 恢复，避免杀死消费循环。
func safeDispatch(fn func()) {
	if fn == nil {
		return
	}
	go func() {
		defer func() {
			if r := recover(); r != nil {
				slog.Error("tray handler panic", "recover", r)
			}
		}()
		fn()
	}()
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
