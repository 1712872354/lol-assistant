import { create } from "zustand";
import { queueFilterArgs, type QueueFilterKey } from "@/features/gameinfo/queueOptions";
import { callApp } from "@/lib/backend";
import type {
  GameinfoPlayerSlot,
  GameinfoSideFilter,
  GameinfoTeamView,
  GameinfoViewState,
  GameflowPhase,
} from "@/lib/types";

const EMPTY_SLOT: GameinfoPlayerSlot = { filled: false, isSelf: false };

function emptySlots(): GameinfoPlayerSlot[] {
  return Array.from({ length: 5 }, () => ({ ...EMPTY_SLOT }));
}

function defaultTeam(key: "ally" | "enemy"): GameinfoTeamView {
  const ally = key === "ally";
  return {
    key,
    label: ally ? "我方" : "敌方",
    sideText: ally ? "蓝方·房间" : "红方",
    badge: ally ? "我方" : "敌方",
    playerCount: 0,
    winRate: 0,
    teamScore: 0,
    slots: emptySlots(),
  };
}

/** 空视图（未连接客户端 / gameflow=None 空闲） */
export function defaultView(phase: GameflowPhase = "None"): GameinfoViewState {
  return {
    phase,
    queueLabel: "",
    queueId: 0,
    teams: [defaultTeam("ally"), defaultTeam("enemy")],
  };
}

/** 后端视图归一：slots 恒 5、阶段文案兜底 */
function normalizeView(v: GameinfoViewState): GameinfoViewState {
  const phase = (v.phase ?? "None") as GameflowPhase;
  return {
    phase,
    queueLabel: v.queueLabel ?? "",
    queueId: v.queueId ?? 0,
    teams: (v.teams ?? []).slice(0, 2).map((t) => {
      const slots = (t.slots ?? [])
        .slice(0, 5)
        .map((s) => ({ ...EMPTY_SLOT, ...s, filled: s.filled === true }));
      while (slots.length < 5) slots.push({ ...EMPTY_SLOT });
      return { ...t, slots };
    }),
  };
}

interface GameinfoState {
  sideFilter: GameinfoSideFilter;
  setSideFilter: (v: GameinfoSideFilter) => void;
  /** 近况口径（对局类型下拉）："follow"=跟随当前对局；"all"=全部；其余=QUEUE_TYPE_OPTIONS key */
  queueKey: QueueFilterKey;
  /** 切换对局类型 → 立即按新口径自动刷新 */
  setQueueKey: (v: QueueFilterKey) => void;
  view: GameinfoViewState;
  loading: boolean;
  /** 最近一次刷新失败的用户可读原因；成功后清空 */
  error: string | null;
  /** WS 阶段事件即时反馈（状态徽章/表头先动），完整数据由 scheduleRefresh 补 */
  setPhase: (p: GameflowPhase) => void;
  /** 从 Go 侧 gameinfo 聚合服务拉整视图（携带当前对局类型口径）；失败/未连接回落空视图 */
  refresh: () => Promise<void>;
}

export const useGameinfoStore = create<GameinfoState>((set, get) => ({
  sideFilter: "all",
  setSideFilter: (sideFilter) => set({ sideFilter }),
  queueKey: "follow",
  setQueueKey: (queueKey) => {
    set({ queueKey });
    void get().refresh();
  },
  view: defaultView(),
  loading: false,
  error: null,

  setPhase: (phase) => set((s) => ({ view: { ...s.view, phase } })),

  refresh: async () => {
    // 请求序号：仅最新一次 refresh 可写回，防旧响应覆盖新数据
    const rid = ++refreshSeq;
    set({ loading: true });
    try {
      const v = await callApp<GameinfoViewState>(
        "GetGameflowState",
        queueFilterArgs(get().queueKey),
      );
      if (rid !== refreshSeq) return;
      if (v && Array.isArray(v.teams) && v.teams.length > 0) {
        const view = normalizeView(v);
        set({ view, loading: false, error: null });
        scheduleInGameRetry(view);
      } else if (v) {
        // 调用成功返回空/默认视图
        retryCount = 0;
        set({ view: normalizeView(v), loading: false, error: null });
      } else {
        // 调用失败（callApp 吞错返回 null）：保留上一帧数据，给出错误态
        set({ loading: false, error: "获取对局信息失败" });
      }
    } catch (e) {
      if (rid !== refreshSeq) return;
      // 刷新失败必须可见：保留上一帧数据 + 错误态
      const msg = e instanceof Error ? e.message : String(e);
      console.error("[gameinfo] 刷新失败:", msg);
      set({ loading: false, error: msg });
    }
  },
}));

/* ── WS 事件 → 防抖刷新 ───────────────────────────── */

let bound = false;
let timer: ReturnType<typeof setTimeout> | null = null;
let retryCount = 0;
let refreshSeq = 0;

/**
 * 游戏内数据未就绪（GameStart 初段花名册/Live 未齐）时 3s 自动重试；
 * 双队有填充即复位，阶段切换复位，单轮上限 40 次（≈2 分钟）。
 */
function scheduleInGameRetry(view: GameinfoViewState): void {
  const inGame =
    view.phase === "GameStart" || view.phase === "InProgress" || view.phase === "Reconnect";
  const filled = view.teams.some((t) => t.slots.some((s) => s.filled));
  if (filled || !inGame) {
    retryCount = 0;
    return;
  }
  if (retryCount >= 40) return;
  retryCount++;
  scheduleRefresh(3000);
}

/** 合并事件风暴（选人 hover/计时器高频），delay 窗口内仅触发一次刷新 */
function scheduleRefresh(delay: number): void {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    timer = null;
    void useGameinfoStore.getState().refresh();
  }, delay);
}

/**
 * 订阅 Go → JS gameinfo:update；仅关注 URI 变化时防抖刷新整视图：
 *   gameflow-phase → 即时改阶段 + 400ms 刷新；lobby/session → 600ms；champ-select → 800ms。
 * 返回取消函数。
 */
export function bindGameinfoEvents(
  on: (name: string, cb: (...data: unknown[]) => void) => () => void,
): () => void {
  if (bound) return () => {};
  bound = true;
  const off = on("gameinfo:update", (payload) => {
    const evt = payload as { uri?: string; data?: unknown } | undefined;
    const uri = evt?.uri ?? "";
    if (!uri) return;
    if (uri.includes("gameflow-phase")) {
      useGameinfoStore.getState().setPhase(String(evt?.data ?? "None") as GameflowPhase);
      scheduleRefresh(400);
    } else if (uri.includes("/lol-lobby/v2/lobby") || uri.includes("gameflow-session")) {
      scheduleRefresh(600);
    } else if (uri.includes("champ-select")) {
      scheduleRefresh(800);
    }
  });
  return () => {
    bound = false;
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    off();
  };
}
