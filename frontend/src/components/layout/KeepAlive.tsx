import { ErrorBoundary } from "@/components/ErrorBoundary";
import { GameInfoView } from "@/features/gameinfo/GameInfoView";
import { HistoryView } from "@/features/history/HistoryView";
import { SettingsView } from "@/features/settings/SettingsView";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";

/** KeepAlive：页面常驻 DOM，仅切换 hidden，保留滚动与 Query 缓存。
 *  每页独立 ErrorBoundary：单页渲染崩溃不拖垮其余常驻页。 */
export function KeepAlive() {
  const activeView = useAppStore((s) => s.activeView);

  return (
    <main className="relative min-w-0 flex-1 overflow-hidden bg-background">
      <div
        className={cn(
          "absolute inset-0 overflow-auto",
          activeView !== "history" && "hidden",
        )}
      >
        <ErrorBoundary>
          <HistoryView />
        </ErrorBoundary>
      </div>
      <div
        className={cn(
          "absolute inset-0 overflow-auto",
          activeView !== "gameinfo" && "hidden",
        )}
      >
        <ErrorBoundary>
          <GameInfoView />
        </ErrorBoundary>
      </div>
      <div
        className={cn(
          "absolute inset-0 overflow-auto",
          activeView !== "settings" && "hidden",
        )}
      >
        <ErrorBoundary>
          <SettingsView />
        </ErrorBoundary>
      </div>
    </main>
  );
}
