import { Activity, ChartNoAxesCombined, RefreshCw, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ViewKey } from "@/lib/types";
import { useAppStore } from "@/stores/appStore";
import { useUpdateStore } from "@/stores/updateStore";

const NAV: Array<{ key: ViewKey; label: string; icon: typeof Settings }> = [
  { key: "history", label: "战绩", icon: ChartNoAxesCombined },
  { key: "gameinfo", label: "对局", icon: Activity },
  { key: "settings", label: "设置", icon: Settings },
];

export function Sidebar() {
  const activeView = useAppStore((s) => s.activeView);
  const setActiveView = useAppStore((s) => s.setActiveView);
  const appVersion = useUpdateStore((s) => s.appVersion);
  const info = useUpdateStore((s) => s.info);
  const checking = useUpdateStore((s) => s.checking);
  const installing = useUpdateStore((s) => s.installing);
  const error = useUpdateStore((s) => s.error);
  const check = useUpdateStore((s) => s.check);
  const setDialogOpen = useUpdateStore((s) => s.setDialogOpen);

  const hasUpdate = !!info?.hasUpdate;

  return (
    <aside className="flex w-[200px] shrink-0 flex-col border-r border-sidebar-border bg-sidebar">
      <nav className="flex flex-col gap-0.5 p-2">
        {NAV.map(({ key, label, icon: Icon }) => {
          const active = activeView === key;
          return (
            <button
              key={key}
              type="button"
              onClick={() => setActiveView(key)}
              className={cn(
                "flex items-center gap-2.5 rounded-lg px-3 py-2.5 text-[13px] transition-colors",
                active
                  ? "bg-sidebar-accent font-medium text-sidebar-accent-foreground"
                  : "text-sidebar-foreground/70 hover:bg-sidebar-accent/40 hover:text-sidebar-foreground",
              )}
            >
              <Icon
                className={cn(
                  "h-4 w-4",
                  active ? "text-sidebar-accent-foreground" : "opacity-70",
                )}
              />
              {label}
            </button>
          );
        })}
      </nav>

      <div className="mt-auto space-y-2 border-t border-sidebar-border p-3">
        <div className="flex items-baseline justify-between text-[11px] text-muted-foreground">
          <span>当前版本</span>
          <span className="tnum font-medium text-foreground/80">
            {appVersion}
            {hasUpdate ? (
              <span className="ml-1 text-[10px] font-semibold text-win-fg">可更新</span>
            ) : null}
          </span>
        </div>
        {error ? (
          <p className="text-[10px] leading-snug text-destructive/90">{error}</p>
        ) : null}
        <div className="flex gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="press-scale h-7 flex-1 gap-1 px-2 text-[11px]"
            type="button"
            disabled={checking || installing}
            onClick={() => void check()}
          >
            <RefreshCw className={cn("h-3 w-3", checking && "animate-spin")} />
            检查
          </Button>
          <Button
            variant={hasUpdate ? "default" : "outline"}
            size="sm"
            className="press-scale h-7 flex-1 px-2 text-[11px]"
            type="button"
            disabled={!hasUpdate || installing}
            onClick={() => setDialogOpen(true)}
          >
            {installing ? "安装中" : "更新"}
          </Button>
        </div>
      </div>
    </aside>
  );
}
