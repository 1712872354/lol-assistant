import { cn } from "@/lib/utils";
import type { ConnState } from "@/lib/types";
import { phaseLabelCN } from "@/lib/phase";
import { useAppStore } from "@/stores/appStore";
import { useGameinfoStore } from "@/stores/gameinfoStore";

const STATE_UI: Record<ConnState, { dot: string; text: string }> = {
  disconnected: { dot: "bg-zinc-400", text: "未连接客户端" },
  unauthenticated: { dot: "bg-amber-400", text: "客户端未登录" },
  connected: { dot: "bg-emerald-500", text: "已连接" },
};

/**
 * 标题栏状态徽章：已连接时显示英雄联盟客户端状态（大厅中/房间内/游戏中…），
 * 召唤师名收进 title 提示；未连接/未登录显示连接态文案。
 */
export function ConnBadge() {
  const conn = useAppStore((s) => s.conn);
  const phase = useGameinfoStore((s) => s.view.phase);
  const ui = STATE_UI[conn.state] ?? STATE_UI.disconnected;
  const stateText = conn.state === "connected" ? phaseLabelCN(phase) : ui.text;
  const title =
    conn.state === "connected" && conn.gameName
      ? `已连接 · ${conn.gameName}${conn.tagLine ? `#${conn.tagLine}` : ""} · ${stateText}`
      : stateText;

  return (
    <span
      title={title}
      className={cn(
        "inline-flex max-w-[240px] items-center gap-1.5 rounded-full border border-border",
        "bg-secondary px-2 py-0.5 text-[11px] text-muted-foreground",
      )}
    >
      <span className={cn("h-1.5 w-1.5 shrink-0 rounded-full", ui.dot)} />
      <span className="truncate">{stateText}</span>
    </span>
  );
}
