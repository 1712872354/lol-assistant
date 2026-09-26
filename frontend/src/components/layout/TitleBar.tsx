import { useQueryClient } from "@tanstack/react-query";
import { Minus, RefreshCw, Settings, Square, X } from "lucide-react";
import { useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { ConnBadge } from "@/components/layout/ConnBadge";
import { Button } from "@/components/ui/button";
import { SidebarTrigger } from "@/components/ui/sidebar";
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
  const pathname = useLocation().pathname;
  const navigate = useNavigate();
  const offline = useAppStore((s) => s.conn.state !== "connected");
  const queryClient = useQueryClient();
  const [spinning, setSpinning] = useState(false);

  /** 只刷新当前页：战绩 → hist 查询；对局 → gameinfo 聚合；设置无需拉数 */
  const refreshCurrent = () => {
    if (spinning) return;
    setSpinning(true);
    if (pathname === "/history") {
      void queryClient.invalidateQueries({ queryKey: ["hist"] });
    } else if (pathname === "/gameinfo") {
      void useGameinfoStore.getState().refresh();
    }
    window.setTimeout(() => setSpinning(false), 500);
  };

  return (
    <header
      className="drag-region flex h-11 shrink-0 select-none items-center gap-2 border-b border-border/70 bg-card px-2.5"
      onDoubleClick={windowToggleMaximise}
    >
      <div className="no-drag">
        <SidebarTrigger className="-ml-1" />
      </div>
      <img
        src="/brand-ultra.svg"
        alt=""
        draggable={false}
        className="h-5 w-5 rounded-md"
      />
      <span className="text-[12.5px] font-semibold tracking-wide text-foreground">
        LOL助手
      </span>

      <ConnBadge />

      {/* 对局页筛选：阵营 + 近况口径 */}
      {pathname === "/gameinfo" ? (
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
          title={
            pathname === "/history"
              ? "刷新当前战绩数据"
              : pathname === "/gameinfo"
                ? "刷新当前对局数据"
                : "刷新当前页数据"
          }
          onClick={refreshCurrent}
          disabled={offline || pathname === "/settings"}
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
          onClick={() => navigate("/settings")}
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
