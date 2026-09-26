import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { RouterProvider } from "react-router-dom";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { UpdateDialog } from "@/components/UpdateDialog";
import { TooltipProvider } from "@/components/ui/tooltip";
import { router } from "@/app/router";
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
          <RouterProvider router={router} />
          <UpdateDialog />
        </ErrorBoundary>
      </TooltipProvider>
    </QueryClientProvider>
  );
}
