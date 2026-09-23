import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
import type { UpdateInfo } from "@/stores/updateStore";

/** Wails PascalCase 方法名 → Tauri 命令 + 具名参数打包（键用 camelCase，对齐 Tauri 2 默认） */
type WailsFn =
  | "GetConnStatus"
  | "GetConfig"
  | "SetConfig"
  | "GetAppVersion"
  | "CheckUpdate"
  | "DownloadAndInstallUpdate"
  | "IsPortable"
  | "GetMatches"
  | "GetMatchDetail"
  | "SearchSummoner"
  | "GetSelfSummoner"
  | "GetPlayersRanked"
  | "GetMatchAsset"
  | "GetGameflowState"
  | "WindowMinimise"
  | "WindowToggleMaximise"
  | "WindowClose";

interface CmdSpec {
  cmd: string;
  pack?: (args: unknown[]) => Record<string, unknown>;
}

const CMD_MAP: Record<WailsFn, CmdSpec> = {
  GetConnStatus: { cmd: "get_conn_status" },
  GetConfig: { cmd: "get_config" },
  SetConfig: { cmd: "set_config", pack: (a) => ({ cfg: a[0] }) },
  GetAppVersion: { cmd: "get_app_version" },
  CheckUpdate: { cmd: "check_update" },
  DownloadAndInstallUpdate: { cmd: "download_and_install_update" },
  IsPortable: { cmd: "is_portable" },
  GetMatches: { cmd: "get_matches", pack: (a) => ({ puuid: a[0], page: a[1] }) },
  // Tauri 2 期望 camelCase 参数键（Rust snake_case 会自动转换）
  GetMatchDetail: {
    cmd: "get_match_detail",
    pack: (a) => ({ gameId: a[0], selfPuuid: a[1] }),
  },
  SearchSummoner: { cmd: "search_summoner", pack: (a) => ({ name: a[0] }) },
  GetSelfSummoner: { cmd: "get_self_summoner" },
  GetPlayersRanked: {
    cmd: "get_players_ranked",
    pack: (a) => ({ summonerIds: a[0] }),
  },
  GetMatchAsset: { cmd: "get_match_asset", pack: (a) => ({ kind: a[0], id: a[1] }) },
  GetGameflowState: {
    cmd: "get_gameflow_state",
    pack: (a) => ({ queueFilter: a[0] }),
  },
  WindowMinimise: { cmd: "window_minimise" },
  WindowToggleMaximise: { cmd: "window_toggle_maximise" },
  WindowClose: { cmd: "window_close" },
};

/** 是否运行在 Tauri 宿主中（纯浏览器 dev 返回 false） */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** 统一绑定调用：Tauri 环境外返回 null，不抛错 */
export async function callApp<T>(
  fn: WailsFn,
  ...args: unknown[]
): Promise<T | null> {
  if (!isTauri()) return null;
  try {
    const spec = CMD_MAP[fn];
    if (!spec) return null;
    const payload = spec.pack ? spec.pack(args) : {};
    const result = await invoke<T>(spec.cmd, payload);
    return result ?? null;
  } catch (e) {
    console.error(`[backend] ${String(fn)} 调用失败:`, e);
    return null;
  }
}

/** 严格绑定调用：抛出错误供 React Query 捕获 */
export async function callAppStrict<T>(
  fn: WailsFn,
  ...args: unknown[]
): Promise<T> {
  if (!isTauri()) throw new Error("后端未就绪（非 Tauri 运行环境）");
  const spec = CMD_MAP[fn];
  if (!spec) throw new Error(`未知命令: ${String(fn)}`);
  const payload = spec.pack ? spec.pack(args) : {};
  return invoke<T>(spec.cmd, payload);
}

/** 订阅 Tauri → JS 事件；callback 收 payload（对齐原 Wails 回调签名） */
export function onAppEvent(
  name: string,
  callback: (...data: unknown[]) => void,
): () => void {
  if (!isTauri()) return () => {};
  let unlisten: (() => void) | null = null;
  let disposed = false;
  void listen(name, (event) => {
    try {
      callback(event.payload);
    } catch (e) {
      console.error(`[backend] listener ${name} error:`, e);
    }
  })
    .then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    })
    .catch((e) => {
      console.error(`[backend] listen ${name} failed:`, e);
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

/* ── 窗口控制（自绘标题栏按钮） ─────────────────────────────────── */

export const windowMinimise = (): void => {
  void callApp("WindowMinimise");
};
export const windowToggleMaximise = (): void => {
  void callApp("WindowToggleMaximise");
};
export const windowClose = (): void => {
  void callApp("WindowClose");
};

/** 类型再导出，避免调用方改动 */
export type { AppConfig, ConnStatus, MatchPage, UpdateInfo };
export type { MatchDetail, SummonerResult, RankedInfo, AssetResult, GameinfoViewState };
