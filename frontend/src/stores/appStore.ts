import { create } from "zustand";
import { callAppStrict } from "@/lib/backend";
import {
  DEFAULT_CONFIG,
  type AppConfig,
  type ConnStatus,
  type ThemeMode,
  type ViewKey,
} from "@/lib/types";

function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode !== "system") return mode;
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

/** 主题经 data-theme 属性挂 html（shadcn CSS 变量 + 深色自定义 variant） */
export function applyTheme(mode: ThemeMode): void {
  document.documentElement.dataset.theme = resolveTheme(mode);
}

interface AppState {
  /** KeepAlive 双页切换：仅改此值，组件不卸载 */
  activeView: ViewKey;
  setActiveView: (v: ViewKey) => void;
  conn: ConnStatus;
  setConn: (c: ConnStatus) => void;
  config: AppConfig;
  setConfig: (c: AppConfig) => void;
  patchConfig: (p: Partial<AppConfig>) => void;
}

/** SetConfig 串行队列：防连点主题/分页时后发先至覆盖 */
let configWriteChain: Promise<void> = Promise.resolve();

export const useAppStore = create<AppState>((set) => ({
  activeView: "history",
  setActiveView: (activeView) => set({ activeView }),
  conn: { state: "disconnected" },
  setConn: (conn) => set({ conn }),
  config: DEFAULT_CONFIG,
  setConfig: (config) => {
    applyTheme(config.theme);
    set({ config });
  },
  patchConfig: (p) => {
    set((s) => {
      const config = { ...s.config, ...p };
      applyTheme(config.theme);
      return { config };
    });
    const next = useAppStore.getState().config;
    configWriteChain = configWriteChain
      .then(() => callAppStrict("SetConfig", next))
      .then(() => undefined)
      .catch((e) => {
        console.error("[appStore] SetConfig failed:", e);
      });
  },
}));

/** 跟随系统主题变化 */
window
  .matchMedia?.("(prefers-color-scheme: dark)")
  .addEventListener("change", () => {
    const { config } = useAppStore.getState();
    if (config.theme === "system") applyTheme("system");
  });
