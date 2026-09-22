import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { KeepAlive } from "@/components/layout/KeepAlive";
import { Sidebar } from "@/components/layout/Sidebar";
import { TitleBar } from "@/components/layout/TitleBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { UpdateDialog } from "@/components/UpdateDialog";
import { callApp, onAppEvent } from "@/lib/backend";
import type { AppConfig, ConnStatus } from "@/lib/types";
import { applyTheme, useAppStore } from "@/stores/appStore";
import { bindGameinfoEvents, useGameinfoStore } from "@/stores/gameinfoStore";
import { bindUpdateAutoload } from "@/stores/updateStore";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnMount: false,
      staleTime: 30_000,
      gcTime: 5 * 60_000,
    },
  },
});

export default function App() {
  const setConn = useAppStore((s) => s.setConn);
  const setConfig = useAppStore((s) => s.setConfig);

  useEffect(() => {
    applyTheme(useAppStore.getState().config.theme);

    void callApp<ConnStatus>("GetConnStatus").then((c) => c && setConn(c));
    void callApp<AppConfig>("GetConfig").then((cfg) => cfg && setConfig(cfg));
    bindUpdateAutoload();

    const offConn = onAppEvent("conn:status", (data) => {
      if (data && typeof data === "object") {
        const c = data as ConnStatus;
        setConn(c);
        void useGameinfoStore.getState().refresh();
      }
    });
    const offGame = bindGameinfoEvents(onAppEvent);
    void useGameinfoStore.getState().refresh();

    return () => {
      offConn();
      offGame();
    };
  }, [setConn, setConfig]);

  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider delayDuration={200}>
        <ErrorBoundary>
          <div className="flex h-screen flex-col overflow-hidden">
            <TitleBar />
            <div className="flex min-h-0 flex-1">
              <Sidebar />
              <KeepAlive />
            </div>
            <UpdateDialog />
          </div>
        </ErrorBoundary>
      </TooltipProvider>
    </QueryClientProvider>
  );
}
