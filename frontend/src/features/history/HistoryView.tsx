import { useQueryClient } from "@tanstack/react-query";
import { WifiOff } from "lucide-react";
import { useEffect, useRef } from "react";
import { EmptyState } from "@/components/EmptyState";
import { useAppStore } from "@/stores/appStore";
import { useHistoryStore } from "@/stores/historyStore";
import { fetchSelfSummoner } from "./api";
import { MatchDetailPanel } from "./MatchDetailPanel";
import { MatchList } from "./MatchList";
import { SummonerTabs } from "./SummonerTabs";

/**
 * 历史战绩页：主从结构，对齐亮色 shadcn 参考截图。
 *   顶部：召唤师标签 + 动作区（搜索/刷新/自己/模式筛选）
 *   左侧：战绩列表卡片
 *   右侧：选中对局明细
 */
export function HistoryView() {
  const offline = useAppStore((s) => s.conn.state !== "connected");
  const tabs = useHistoryStore((s) => s.tabs);
  const activeTabId = useHistoryStore((s) => s.activeTabId);
  const activeTab = tabs.find((t) => t.id === activeTabId) ?? null;
  const queryClient = useQueryClient();
  const autoRef = useRef(false);

  useEffect(() => {
    if (offline) {
      autoRef.current = false;
      return;
    }
    void queryClient.invalidateQueries({ queryKey: ["hist"] });
    if (autoRef.current) return;
    const st = useHistoryStore.getState();
    if (st.tabs.length > 0) {
      autoRef.current = true;
      return;
    }
    autoRef.current = true;
    fetchSelfSummoner()
      .then((s) => st.openSummoner(s, true))
      .catch(() => {
        autoRef.current = false;
      });
  }, [offline, queryClient]);

  if (offline && tabs.length === 0) {
    return (
      <div className="flex h-full flex-col">
        <EmptyState
          className="m-4 flex-1"
          icon={WifiOff}
          title="未连接英雄联盟客户端"
          desc="请启动英雄联盟客户端并登录账号。连接建立后本页自动恢复，无需重启助手。"
        />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <SummonerTabs
        onRefresh={() => {
          void queryClient.invalidateQueries({ queryKey: ["hist"] });
        }}
      />
      {activeTab ? (
        <div className="flex min-h-0 flex-1 overflow-hidden">
          <aside className="m-2 mr-0 flex h-full min-h-0 w-[280px] shrink-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
            <MatchList tab={activeTab} />
          </aside>
          <section className="m-2 flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border border-border bg-card">
            <MatchDetailPanel tab={activeTab} />
          </section>
        </div>
      ) : (
        <EmptyState
          className="m-4 flex-1"
          title="查询召唤师战绩"
          desc="点击右上角搜索图标输入 Riot ID（昵称#TAG），或点击用户图标查看自己的历史战绩。可同时开启多个标签页对比查看。"
        />
      )}
    </div>
  );
}
