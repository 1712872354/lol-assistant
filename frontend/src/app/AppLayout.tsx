import { useEffect, useRef, useState } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { AppSidebar } from "@/components/layout/AppSidebar";
import { TitleBar } from "@/components/layout/TitleBar";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";

/** 路由 → 页面名（焦点迁移后的读屏播报） */
const ROUTE_TITLES: Record<string, string> = {
  "/history": "战绩",
  "/gameinfo": "对局信息",
  "/settings": "设置",
};

/** 应用壳：Tauri 窗口 chrome + shadcn SidebarProvider 骨架（dashboard-01 范式） */
export function AppLayout() {
  const { pathname } = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const firstRender = useRef(true);
  const [routeTitle, setRouteTitle] = useState("");

  // SPA 路由切换后焦点移至主内容区（WCAG 2.4.3）：后续 Tab 从新页面开始
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    mainRef.current?.focus();
    setRouteTitle(ROUTE_TITLES[pathname] ?? "");
  }, [pathname]);

  return (
    <SidebarProvider>
      <div className="flex h-screen w-full flex-col overflow-hidden">
        <TitleBar />
        <div className="flex min-h-0 flex-1">
          <AppSidebar />
          <SidebarInset
            ref={mainRef}
            tabIndex={-1}
            className="min-h-0 overflow-hidden focus:outline-none"
          >
            {/* 路由播报：焦点迁移的读屏配套 */}
            <p className="sr-only" role="status">
              {routeTitle ? `已进入${routeTitle}页` : ""}
            </p>
            <Outlet />
          </SidebarInset>
        </div>
      </div>
    </SidebarProvider>
  );
}
