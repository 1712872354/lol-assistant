import { useQueryClient } from "@tanstack/react-query";
import { Minus, RefreshCw, Settings, Square, X } from "lucide-react";
import { useState } from "react";
import { ConnBadge } from "@/components/layout/ConnBadge";
import { Button } from "@/components/ui/button";
import { GameinfoViewControls } from "@/features/gameinfo/ViewControls";
import {
  windowClose,
  windowMinimise,
  windowToggleMaximise,
} from "@/lib/backend";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useGameinfoStore } from "@/stores/gameinfoStore";

const controlCls =
  "no-drag h-8 w-8 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground";

export function TitleBar() {
  const activeView = useAppStore((s) => s.activeView);
  const setActiveView = useAppStore((s) => s.setActiveView);
  const offline = useAppStore((s) => s.conn.state !== "connected");
  const queryClient = useQueryClient();
  const [spinning, setSpinning] = useState(false);

  /** 全局刷新：对局聚合 + 战绩查询一并失效 */
  const refreshAll = () => {
    if (spinning) return;
    setSpinning(true);
    void useGameinfoStore.getState().refresh();
    void queryClient.invalidateQueries({ queryKey: ["hist"] });
    window.setTimeout(() => setSpinning(false), 500);
  };

  return (
    <header
      className="drag-region flex h-11 shrink-0 select-none items-center gap-3 border-b border-border bg-card px-3"
      onDoubleClick={windowToggleMaximise}
    >
      <img
        src="/brand-ultra.svg"
        alt=""
        draggable={false}
        className="h-6 w-6 rounded-md"
      />
      <span className="text-[13px] font-semibold tracking-wide text-foreground">
        LOL助手
      </span>

      <ConnBadge />

      {/* 对局页筛选：阵营 + 近况口径（从内容区上移） */}
      {activeView === "gameinfo" ? (
        <div className="no-drag ml-2 flex min-w-0 items-center">
          <GameinfoViewControls disabled={offline} />
        </div>
      ) : null}

      <div className="no-drag ml-auto flex items-center gap-0.5">
        <Button
          variant="ghost"
          size="icon"
          className={cn(controlCls, "press-scale")}
          aria-label="刷新"
          title="刷新对局与战绩数据"
          onClick={refreshAll}
        >
          <RefreshCw
            className={cn(
              "h-4 w-4 transition-transform duration-200",
              spinning && "animate-spin",
            )}
          />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className={controlCls}
          aria-label="设置"
          onClick={() => setActiveView("settings")}
        >
          <Settings className="h-4 w-4" />
        </Button>
        <div className="mx-1 h-4 w-px bg-border" />
        <Button
          variant="ghost"
          size="icon"
          className={controlCls}
          onClick={windowMinimise}
          aria-label="最小化"
        >
          <Minus className="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className={controlCls}
          onClick={windowToggleMaximise}
          aria-label="最大化"
        >
          <Square className="h-3 w-3" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="no-drag h-8 w-8 rounded-md text-muted-foreground hover:bg-destructive hover:text-destructive-foreground"
          onClick={windowClose}
          aria-label="关闭"
        >
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>
    </header>
  );
}
