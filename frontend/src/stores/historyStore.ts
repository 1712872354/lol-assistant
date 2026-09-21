import { create } from "zustand";
import type { SummonerResult } from "@/lib/types";

/** 战绩页标签（浏览器式标签页，一个召唤师一个会话） */
export interface HistoryTab {
  id: string; // = puuid
  puuid: string;
  name: string; // "gameName#tagLine"
  iconId: number; // 头像
  self: boolean; // 是否"查看自己"
  summonerId?: string; // 段位查询用（LCU identities / by-puuid 补全）
}

interface HistoryState {
  tabs: HistoryTab[];
  activeTabId: string | null;
  /** puuid → 当前页码（从 0 起） */
  pages: Record<string, number>;
  /** puuid → 选中对局 gameId */
  selections: Record<string, number | null>;
  /** 模式筛选：all 或 queueId 字符串 */
  queueFilter: string;
  setQueueFilter: (v: string) => void;

  /** 打开/激活召唤师标签（同 puuid 去重仅激活） */
  openSummoner: (s: SummonerResult, self?: boolean) => void;
  closeTab: (id: string) => void;
  setActive: (id: string) => void;
  setPage: (puuid: string, page: number) => void;
  select: (puuid: string, gameId: number | null) => void;
}

export const useHistoryStore = create<HistoryState>((set) => ({
  tabs: [],
  activeTabId: null,
  pages: {},
  selections: {},
  queueFilter: "all",
  setQueueFilter: (queueFilter) => set({ queueFilter }),

  openSummoner: (s, self = false) =>
    set((st) => {
      const id = (s.puuid ?? "").trim();
      if (!id) return st;
      const exists = st.tabs.some((t) => t.id === id);
      if (exists) {
        return {
          ...st,
          activeTabId: id,
          tabs: st.tabs.map((t) =>
            t.id === id && s.summonerId ? { ...t, summonerId: s.summonerId } : t,
          ),
        };
      }
      const name =
        s.displayName ||
        (s.gameName ? `${s.gameName}${s.tagLine ? `#${s.tagLine}` : ""}` : id);
      return {
        ...st,
        tabs: [
          ...st.tabs,
          {
            id,
            puuid: id,
            name,
            iconId: s.profileIconId ?? 0,
            self,
            summonerId: s.summonerId || undefined,
          },
        ],
        activeTabId: id,
      };
    }),

  closeTab: (id) =>
    set((st) => {
      const idx = st.tabs.findIndex((t) => t.id === id);
      if (idx < 0) return st;
      const tabs = st.tabs.filter((t) => t.id !== id);
      let { activeTabId } = st;
      if (st.activeTabId === id) {
        activeTabId = tabs.length > 0 ? tabs[Math.min(idx, tabs.length - 1)].id : null;
      }
      return { ...st, tabs, activeTabId };
    }),

  setActive: (id) => set({ activeTabId: id }),

  setPage: (puuid, page) =>
    set((st) => ({ pages: { ...st.pages, [puuid]: Math.max(0, page) } })),

  select: (puuid, gameId) =>
    set((st) => ({ selections: { ...st.selections, [puuid]: gameId } })),
}));
