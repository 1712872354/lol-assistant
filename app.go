package main

import (
	"context"
	"log/slog"

	"github.com/1712872354/lol-assistant/internal/config"
	"github.com/1712872354/lol-assistant/internal/lcu"
	"github.com/1712872354/lol-assistant/internal/liveclient"
	"github.com/1712872354/lol-assistant/internal/parser"
	"github.com/1712872354/lol-assistant/internal/tray"
	"github.com/1712872354/lol-assistant/internal/update"
	"github.com/1712872354/lol-assistant/service/gameinfo"
	"github.com/1712872354/lol-assistant/service/history"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

// App 是前端绑定层：参数校验 + 调用服务 + 事件桥接，不承载业务逻辑。
type App struct {
	ctx       context.Context
	cfg       *config.Store
	monitor   *lcu.Monitor
	hist      *history.Service   // M2 历史战绩服务（startup 构造，注入 pageSize/apiConcurrency）
	live      *liveclient.Client // M3 Live Client 数据端 :2999（fail-soft，仅游戏中可达）
	game      *gameinfo.Service  // M3 对局信息聚合服务
	version   string             // 构建注入（ldflags -X main.version=）
	forceQuit bool               // 强制退出：更新安装/托盘退出时绕过 closeToTray 拦截
}

// NewApp 构造应用实例（wails.Run 前调用，ctx 于 startup 注入）
func NewApp(version string) *App {
	return &App{
		cfg:     config.NewStore(),
		monitor: lcu.NewMonitor(),
		version: version,
	}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	a.cfg.Load()
	a.fitWindowToScreen()
	a.monitor.SetClientPath(a.cfg.Get().ClientPath) // lockfile 通道候选路径
	a.hist = history.New(a.monitor, a.cfg.Get().PageSize, a.cfg.Get().ApiConcurrency)
	a.hist.SetSGPEnabled(a.cfg.Get().SgpEnabled) // SGP 云端数据源开关（段位/战绩优先 SGP，可禁用回退 LCU）
	a.hist.SetConcurrency(a.cfg.Get().ApiConcurrency)
	a.live = liveclient.New()
	a.game = gameinfo.New(a.monitor, a.hist, a.live, a.cfg.Get().ApiConcurrency)
	a.game.SetCareerLimit(a.cfg.Get().PageSize)
	a.monitor.Start(ctx, a.onConnChange, a.onLcuEvent)
	a.startTray()
	slog.Info("app started", "configPath", a.cfg.Path())
}

// startTray 系统托盘：显示主界面 / 退出；配合 closeToTray 控制关窗行为
func (a *App) startTray() {
	tray.Start("LOL助手", tray.Handlers{
		OnShow: a.showMainWindow,
		OnQuit: a.quitFromTray,
	})
}

// showMainWindow 托盘唤回：显示并置于前台
func (a *App) showMainWindow() {
	if a.ctx == nil {
		return
	}
	runtime.WindowShow(a.ctx)
	runtime.WindowUnminimise(a.ctx)
	runtime.WindowSetAlwaysOnTop(a.ctx, true) // 短暂置顶抢焦点
	runtime.WindowSetAlwaysOnTop(a.ctx, false)
}

// quitFromTray 托盘「退出」：收起托盘后结束应用
func (a *App) quitFromTray() {
	a.forceQuit = true
	tray.Stop()
	if a.ctx != nil {
		runtime.Quit(a.ctx)
	}
}

// beforeClose 拦截关窗：closeToTray=true 时隐藏到托盘并阻止退出；
// forceQuit=true（更新安装/主动退出）时不拦截，保证进程真正退出以释放文件锁。
func (a *App) beforeClose(ctx context.Context) (prevent bool) {
	if a.forceQuit {
		return false
	}
	if a.cfg.Get().CloseToTray {
		runtime.WindowHide(ctx)
		slog.Info("window hidden to tray")
		return true
	}
	return false
}

// fitWindowToScreen 小屏时把默认窗口收到工作区约 92%，避免溢出；大屏保持 main.go 默认铺开尺寸
func (a *App) fitWindowToScreen() {
	const (
		wantW, wantH = 1680, 980
		minW, minH   = 1200, 780
	)
	screens, err := runtime.ScreenGetAll(a.ctx)
	if err != nil || len(screens) == 0 {
		runtime.WindowCenter(a.ctx)
		return
	}
	scr := screens[0]
	for _, s := range screens {
		if s.IsCurrent {
			scr = s
			break
		}
	}
	maxW := int(float64(scr.Size.Width) * 0.92)
	maxH := int(float64(scr.Size.Height) * 0.92)
	w, h := wantW, wantH
	if w > maxW {
		w = maxW
	}
	if h > maxH {
		h = maxH
	}
	if w < minW {
		w = minW
	}
	if h < minH {
		h = minH
	}
	runtime.WindowSetSize(a.ctx, w, h)
	runtime.WindowCenter(a.ctx)
	slog.Info("window sized", "screen", scr.Size, "window", [2]int{w, h})
}

func (a *App) shutdown(context.Context) {
	a.monitor.Stop()
	tray.Stop()
	if err := a.cfg.Save(); err != nil {
		slog.Error("config save failed", "err", err)
	}
}

// onConnChange 连接状态变化 → 前端标题栏徽章（conn:status 事件）
func (a *App) onConnChange(st lcu.ConnStatus) {
	if a.ctx != nil {
		runtime.EventsEmit(a.ctx, "conn:status", st)
	}
}

// onLcuEvent LCU WS 事件 → 对局信息页（gameinfo:update 事件，前端按 uri 分发）
func (a *App) onLcuEvent(evt lcu.LcuEvent) {
	if a.ctx != nil {
		runtime.EventsEmit(a.ctx, "gameinfo:update", evt)
	}
}

// histService 战绩服务访问器（防御 startup 未完成时的绑定调用）
func (a *App) histService() *history.Service {
	if a.hist == nil {
		a.hist = history.New(a.monitor, a.cfg.Get().PageSize, a.cfg.Get().ApiConcurrency)
		a.hist.SetSGPEnabled(a.cfg.Get().SgpEnabled)
	}
	return a.hist
}

// gameService 对局信息服务访问器（防御 startup 未完成时的绑定调用）
func (a *App) gameService() *gameinfo.Service {
	if a.game == nil {
		if a.live == nil {
			a.live = liveclient.New()
		}
		a.game = gameinfo.New(a.monitor, a.histService(), a.live, a.cfg.Get().ApiConcurrency)
	}
	return a.game
}

/* ── 绑定方法（Go → TS 自动生成，契约见开发方案 §6.3） ────────── */

// GetConnStatus 当前 LCU 连接状态快照
func (a *App) GetConnStatus() lcu.ConnStatus {
	return a.monitor.Status()
}

// GetConfig 读取应用配置
func (a *App) GetConfig() config.Config {
	return a.cfg.Get()
}

// SetConfig 校验并持久化应用配置，并热更新各服务生效项
func (a *App) SetConfig(c config.Config) error {
	if err := a.cfg.Set(c); err != nil {
		return err
	}
	a.monitor.SetClientPath(a.cfg.Get().ClientPath) // 客户端目录变更影响 lockfile 检测
	a.histService().SetPageSize(a.cfg.Get().PageSize)
	a.histService().SetSGPEnabled(a.cfg.Get().SgpEnabled) // SGP 数据源开关热更新
	a.histService().SetConcurrency(a.cfg.Get().ApiConcurrency)
	a.gameService().SetCareerLimit(a.cfg.Get().PageSize) // 对局页近况场数 = pageSize
	a.gameService().SetConcurrency(a.cfg.Get().ApiConcurrency)
	// closeToTray 托盘未落地前仅持久化，见 WindowClose 注释
	return nil
}

/* ── M2 历史战绩绑定 ──────────────────────────────────────────── */

// GetMatches 指定 puuid 的战绩分页（page 从 0 起）
func (a *App) GetMatches(puuid string, page int) (history.MatchPage, error) {
	return a.histService().GetMatches(puuid, page)
}

// GetMatchDetail 单局明细（selfPuuid 标记本人行并置顶所在队伍）
func (a *App) GetMatchDetail(gameID int64, selfPuuid string) (*parser.MatchDetail, error) {
	return a.histService().GetMatchDetail(gameID, selfPuuid)
}

// SearchSummoner 按 Riot ID / 召唤师名查询
func (a *App) SearchSummoner(name string) (history.SummonerResult, error) {
	return a.histService().SearchSummoner(name)
}

// GetSelfSummoner 当前登录召唤师（"查看自己"与默认标签页）
func (a *App) GetSelfSummoner() (history.SummonerResult, error) {
	return a.histService().GetSelfSummoner()
}

// GetPlayersRanked 批量查询段位（明细页段位列，尽力而为）
func (a *App) GetPlayersRanked(summonerIDs []string) ([]history.RankedInfo, error) {
	return a.histService().GetPlayersRanked(summonerIDs)
}

// GetMatchAsset 游戏资源图标（base64；kind: champion/profile/item/spell/perk/augment）
func (a *App) GetMatchAsset(kind string, id int) (history.AssetResult, error) {
	return a.histService().GetAsset(kind, id)
}

/* ── M3 对局信息绑定 ──────────────────────────────────────────── */

// GetGameflowState 按客户端阶段聚合对局视图（双队卡槽 + 段位/近况补数）。
// queueFilter 近况口径：空 = 全部对局；[-1] = 跟随当前对局队列；其余 = 队列 id 集（前端"对局类型"下拉）。
func (a *App) GetGameflowState(queueFilter []int) (gameinfo.ViewState, error) {
	return a.gameService().GetGameflowState(queueFilter)
}

/* ── 更新（GitHub Releases API） ──────────────────────────────── */

// GetAppVersion 当前构建版本（ldflags 注入）
func (a *App) GetAppVersion() string {
	if a.version == "" {
		return "dev"
	}
	return a.version
}

// CheckUpdate 检查 GitHub 最新 Release
func (a *App) CheckUpdate() (*update.Info, error) {
	c := update.New(a.GetAppVersion())
	return c.Check()
}

// DownloadAndInstallUpdate 下载 Setup 并拉起安装（过程经 update:progress 事件推送）
func (a *App) DownloadAndInstallUpdate(setupURL, sha256Hex string) (string, error) {
	c := update.New(a.GetAppVersion())
	c.OnProgress = func(p update.Progress) {
		if a.ctx != nil {
			runtime.EventsEmit(a.ctx, "update:progress", p)
		}
	}
	path, err := c.DownloadAndInstall(setupURL, sha256Hex)
	if err != nil {
		return "", err
	}
	// 安装器启动后强制退出本进程（绕过 closeToTray），便于覆盖写入
	go func() {
		a.forceQuit = true
		tray.Stop()
		if a.ctx != nil {
			runtime.Quit(a.ctx)
		}
	}()
	return path, nil
}

/* ── 窗口控制（自绘标题栏按钮） ──────────────────────────────── */

// WindowMinimise 最小化窗口
func (a *App) WindowMinimise() {
	if a.ctx != nil {
		runtime.WindowMinimise(a.ctx)
	}
}

// WindowToggleMaximise 最大化 / 还原
func (a *App) WindowToggleMaximise() {
	if a.ctx != nil {
		runtime.WindowToggleMaximise(a.ctx)
	}
}

// WindowClose 关闭窗口（自绘标题栏 X）。
// closeToTray=true 时隐藏到托盘；否则直接退出。
func (a *App) WindowClose() {
	if a.ctx == nil {
		return
	}
	if !a.forceQuit && a.cfg.Get().CloseToTray {
		runtime.WindowHide(a.ctx)
		slog.Info("window hidden to tray")
		return
	}
	a.forceQuit = true
	tray.Stop()
	runtime.Quit(a.ctx)
}
