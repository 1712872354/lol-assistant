package main

import (
	"embed"
	"log"
	"os"
	"path/filepath"

	"github.com/1712872354/lol-assistant/internal/logging"
	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
)

//go:embed all:frontend/dist
var assets embed.FS

// version 构建注入：-ldflags "-X main.version=vX.Y.Z"
var version = "dev"

// webviewDataDir WebView2 用户数据目录（ASCII 路径）。
// wails 默认取 %APPDATA%\[exe名]，而 exe 名含中文时 WebView2 浏览器进程
// 创建失败（实测 WebVie2wProcess failed kind 6/4/0），故显式指定。
func webviewDataDir() string {
	base := os.Getenv("LOCALAPPDATA") // WebView2 数据属缓存性质，放 Local
	if base == "" {
		base, _ = os.UserConfigDir()
	}
	if base == "" {
		base = "."
	}
	dir := filepath.Join(base, "LolAssistant", "EBWebView")
	_ = os.MkdirAll(dir, 0o755)
	return dir
}

func main() {
	// 文件日志先行（%APPDATA%\LOLAssistant\logs\app-YYYYMMDD.log，7 天保留），
	// 连接状态机与 WS 事件流均有 slog 记录，WebView2 异常时可离线诊断
	closeLog, _ := logging.Init()
	defer closeLog()

	app := NewApp(version)

	err := wails.Run(&options.App{
		Title:  "LOL助手",
		// 对局页 5 列玩家卡需要足够宽度；默认铺开，小屏在 startup 里收缩
		Width:     1680,
		Height:    980,
		MinWidth:  1200,
		MinHeight: 780,
		Frameless: true,                                // 自绘标题栏（CSS --wails-draggable: drag）
		BackgroundColour: options.NewRGBA(246, 247, 249, 255), // 与亮色主题背景一致，抑制启动白闪
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		Windows: &windows.Options{
			Theme:               windows.SystemDefault,
			WebviewUserDataPath: webviewDataDir(),
			// 终端安全软件注入/拦截 WebView2 渲染进程时（实测 kind 6/4→0 崩溃循环），
			// 关闭渲染进程代码完整性校验以恢复运行
			WebviewDisableRendererCodeIntegrity: true,
		},
		OnStartup:  app.startup,
		OnShutdown: app.shutdown,
		OnBeforeClose: app.beforeClose,
		Bind: []interface{}{
			app,
		},
	})
	if err != nil {
		log.Fatal("wails.Run:", err)
	}
}
