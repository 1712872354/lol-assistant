import { create } from "zustand";
import { callApp, onAppEvent } from "@/lib/backend";

export interface UpdateInfo {
  hasUpdate: boolean;
  currentVersion: string;
  version: string;
  notes: string;
  pubDate: string;
  setupUrl: string;
  portableUrl: string;
  sha256: string;
  releaseUrl: string;
}

export interface UpdateProgress {
  stage: string;
  percent: number;
  message: string;
}

interface UpdateState {
  appVersion: string;
  info: UpdateInfo | null;
  checking: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  error: string | null;
  dialogOpen: boolean;
  /** done/error 后 2 秒仍未退出时的提示（安装包需手动运行） */
  quitHint: string | null;
  setDialogOpen: (v: boolean) => void;
  loadVersion: () => Promise<void>;
  check: (opts?: { silent?: boolean }) => Promise<UpdateInfo | null>;
  downloadAndInstall: () => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  appVersion: "dev",
  info: null,
  checking: false,
  installing: false,
  progress: null,
  error: null,
  dialogOpen: false,
  quitHint: null,

  setDialogOpen: (v) => set({ dialogOpen: v }),

  loadVersion: async () => {
    try {
      const v = await callApp<string>("GetAppVersion");
      if (v) set({ appVersion: v });
    } catch {
      /* keep dev */
    }
  },

  check: async (opts) => {
    if (get().checking) return get().info;
    set({ checking: true, error: null });
    try {
      const info = await callApp<UpdateInfo>("CheckUpdate");
      set({ info, checking: false });
      if (info?.hasUpdate && !opts?.silent) {
        set({ dialogOpen: true });
      }
      return info ?? null;
    } catch (e) {
      set({
        checking: false,
        error: e instanceof Error ? e.message : String(e),
      });
      return null;
    }
  },

  downloadAndInstall: async () => {
    const info = get().info;
    if (!info?.setupUrl || get().installing) return;
    set({
      installing: true,
      error: null,
      progress: { stage: "downloading", percent: 0, message: "准备下载" },
    });

    const off = onAppEvent("update:progress", (data) => {
      if (data && typeof data === "object") {
        set({ progress: data as UpdateProgress });
      }
    });

    try {
      await callApp<string>("DownloadAndInstallUpdate", info.setupUrl, info.sha256 ?? "");
      set({
        installing: false,
        progress: { stage: "done", percent: 100, message: "安装程序已启动，即将退出" },
      });
      // 2 秒内未退出则提示手动运行安装包；弹窗保持可关闭
      window.setTimeout(() => {
        const s = useUpdateStore.getState();
        if (s.progress?.stage === "done") {
          set({
            quitHint: "应用未自动退出。请关闭本程序后，运行安装包完成更新。",
          });
        }
      }, 2000);
    } catch (e) {
      set({
        installing: false,
        error: e instanceof Error ? e.message : String(e),
        progress: { stage: "error", percent: 0, message: "安装启动失败" },
      });
    } finally {
      off();
    }
  },
}));

/** 启动后延迟检查（有新版弹对话框） */
export function bindUpdateAutoload() {
  void useUpdateStore.getState().loadVersion();
  window.setTimeout(() => {
    void useUpdateStore.getState().check({ silent: false });
  }, 4000);
}
