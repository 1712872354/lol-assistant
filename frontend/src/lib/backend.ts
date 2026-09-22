import type {
  AppConfig,
  AssetResult,
  ConnStatus,
  GameinfoViewState,
  MatchDetail,
  MatchPage,
  RankedInfo,
  SummonerResult,
} from "@/lib/types";

/** Wails v2 运行时注入对象（最小声明；生成 wailsjs 后可整体替换） */
interface WailsRuntime {
  WindowMinimise(): void;
  WindowToggleMaximise(): void;
  WindowUnmaximise?(): void;
  WindowClose(): void;
  Quit(): void;
  WindowShow?(): void;
  EventsOn(name: string, callback: (...data: unknown[]) => void): void;
  EventsOff?(name: string): void;
}

/** Go app 绑定层（wails build/dev 自动生成到 window.go.main.App） */
interface AppBindings {
  GetConnStatus(): Promise<ConnStatus>;
  GetConfig(): Promise<AppConfig>;
  SetConfig(cfg: AppConfig): Promise<void>;
  GetAppVersion(): Promise<string>;
  CheckUpdate(): Promise<import("@/stores/updateStore").UpdateInfo>;
  DownloadAndInstallUpdate(setupUrl: string, sha256: string): Promise<string>;
  // M2 历史战绩绑定（app.go 导出方法，camelCase 一致）
  GetMatches(puuid: string, page: number): Promise<MatchPage>;
  GetMatchDetail(gameId: number, selfPuuid: string): Promise<MatchDetail>;
  SearchSummoner(name: string): Promise<SummonerResult>;
  GetSelfSummoner(): Promise<SummonerResult>;
  GetPlayersRanked(summonerIds: string[]): Promise<RankedInfo[]>;
  GetMatchAsset(kind: string, id: number): Promise<AssetResult>;
  // M3 对局信息绑定（按客户端阶段聚合双队视图；queueFilter：[]=全部，[-1]=跟随当前对局，其余=队列 id 集）
  GetGameflowState(queueFilter: number[]): Promise<GameinfoViewState>;
  WindowMinimise(): Promise<void>;
  WindowToggleMaximise(): Promise<void>;
  WindowClose(): Promise<void>;
}

export function getRuntime(): WailsRuntime | null {
  return (window as unknown as { runtime?: WailsRuntime }).runtime ?? null;
}

export function getApp(): AppBindings | null {
  const w = window as unknown as {
    go?: { main?: { App?: AppBindings }; app?: { App?: AppBindings } };
  };
  // Go 绑定按包名注入：App 位于 package main → window.go.main.App
  return w.go?.main?.App ?? w.go?.app?.App ?? null;
}

/** 统一绑定调用：Wails 环境外（纯浏览器开发）返回 null，不抛错 */
export async function callApp<T>(
  fn: keyof AppBindings,
  ...args: unknown[]
): Promise<T | null> {
  const app = getApp();
  if (!app) return null;
  try {
    const result = await (
      app[fn] as (...a: unknown[]) => Promise<T>
    )(...args);
    return result ?? null;
  } catch (e) {
    console.error(`[backend] ${String(fn)} 调用失败:`, e);
    return null;
  }
}

/** 严格绑定调用：抛出错误供 React Query 捕获（战绩页数据通路使用） */
export async function callAppStrict<T>(
  fn: keyof AppBindings,
  ...args: unknown[]
): Promise<T> {
  const app = getApp();
  if (!app) throw new Error("后端未就绪（非 Wails 运行环境）");
  return (app[fn] as (...a: unknown[]) => Promise<T>)(...args);
}

/** 按事件名维护的监听器集合（Wails EventsOff(name) 会拆掉同名全部监听，需自管） */
const eventListeners = new Map<string, Set<(...data: unknown[]) => void>>();

/** 订阅 Go → JS 事件；cleanup 只移除自身，不影响同名其它监听器 */
export function onAppEvent(
  name: string,
  callback: (...data: unknown[]) => void,
): () => void {
  const rt = getRuntime();
  if (!rt) return () => {};
  let set = eventListeners.get(name);
  if (!set) {
    set = new Set();
    eventListeners.set(name, set);
    rt.EventsOn(name, (...data: unknown[]) => {
      for (const cb of eventListeners.get(name) ?? []) {
        try {
          cb(...data);
        } catch (e) {
          console.error(`[backend] listener ${name} error:`, e);
        }
      }
    });
  }
  set.add(callback);
  return () => {
    const s = eventListeners.get(name);
    if (!s) return;
    s.delete(callback);
    if (s.size === 0) {
      eventListeners.delete(name);
      rt.EventsOff?.(name);
    }
  };
}

/* ── 窗口控制（自绘标题栏按钮） ─────────────────────────────────── */

function viaRuntimeOrApp(
  runtimeFn: keyof WailsRuntime,
  appFn: keyof AppBindings,
): void {
  const rt = getRuntime();
  if (rt) {
    const fn = rt[runtimeFn];
    if (typeof fn === "function") {
      (fn as () => void).call(rt);
      return;
    }
  }
  void callApp(appFn);
}

export const windowMinimise = () => viaRuntimeOrApp("WindowMinimise", "WindowMinimise");
export const windowToggleMaximise = () => viaRuntimeOrApp("WindowToggleMaximise", "WindowToggleMaximise");
export const windowClose = () => viaRuntimeOrApp("WindowClose", "WindowClose");
