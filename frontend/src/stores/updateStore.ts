import { create } from "zustand";
import { callApp, callAppStrict, onAppEvent } from "@/lib/backend";

/** Release 说明 → 弹窗可读纯文本（后端 formatNotes 已清洗；此处兜底再压一遍） */
export function formatReleaseNotes(md: string): string {
  if (!md) return "";
  let s = md.replace(/\r\n/g, "\n");
  // 弹窗只看变更说明：去掉安装包表格与校验示例
  s = s.replace(/^###[ \t]*安装校验[ \t]*\n[\s\S]*?(?=\n#{1,3}[ \t]|\n---|$)/gm, "");
  s = s.replace(/^###[ \t]*安装[ \t]*\n[\s\S]*?(?=\n#{1,3}[ \t]|\n---|$)/gm, "");
  // 代码块：保留内容为缩进行，去掉围栏
  s = s.replace(/```[\w-]*\n([\s\S]*?)```/g, (_m, body: string) =>
    body
      .split("\n")
      .map((l) => (l.trim() ? "    " + l.trim() : ""))
      .join("\n"),
  );
  // 表格：去分隔行，单元格用 · 拼接
  s = s.replace(/^\|[\s:|-]+\|$/gm, "");
  s = s.replace(/^\|(.+)\|$/gm, (_m, row: string) =>
    row
      .split("|")
      .map((c) => c.trim())
      .filter(Boolean)
      .join(" · "),
  );
  // 标题 / 列表 / 强调 / 行内代码
  s = s.replace(/^#{1,6}[ \t]+/gm, "");
  s = s.replace(/^\s*[-*+][ \t]+/gm, "• ");
  s = s.replace(/\*\*([^*]+)\*\*/g, "$1");
  s = s.replace(/`([^`]+)`/g, "$1");
  s = s.replace(/^---+[ \t]*$/gm, "");
  // 收紧空行
  s = s.replace(/\n{3,}/g, "\n\n").trim();
  return s || "本次更新内容暂无说明。";
}

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
    if (!info?.hasUpdate || get().installing) return;
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
      // 严格调用：失败必须进 catch，不能把 null 当成功
      // tauri-plugin-updater 流程：后端自行 check + download + install，无需 setupUrl/sha256
      await callAppStrict<void>("DownloadAndInstallUpdate");
      set({
        installing: false,
        progress: { stage: "done", percent: 100, message: "安装程序已启动，即将退出" },
      });
      // 约 2s 内未退出则提示手动处理（后端另有 os.Exit 兜底）
      window.setTimeout(() => {
        const s = useUpdateStore.getState();
        if (s.progress?.stage === "done") {
          set({
            quitHint: "应用未自动退出。请关闭本程序后，运行安装包完成更新。",
          });
        }
      }, 2500);
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
