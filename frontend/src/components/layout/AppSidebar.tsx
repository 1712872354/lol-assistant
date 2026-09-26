import { Activity, ChartNoAxesCombined, RefreshCw, Settings } from "lucide-react";
import * as React from "react";
import { NavLink } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  useSidebar,
} from "@/components/ui/sidebar";
import { cn } from "@/lib/utils";
import { useUpdateStore } from "@/stores/updateStore";

const NAV = [
  { to: "/history", label: "战绩", icon: ChartNoAxesCombined },
  { to: "/gameinfo", label: "对局", icon: Activity },
  { to: "/settings", label: "设置", icon: Settings },
] as const;

/** 侧栏（sidebar-07 范式：collapsible="icon"）：导航 + 底部版本/更新区 */
export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar
      collapsible="icon"
      // fixed 容器默认 inset-y-0 覆盖全窗口，会让出顶部 TitleBar（h-11）
      className="top-11 h-[calc(100svh-2.75rem)]"
      {...props}
    >
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {NAV.map((item) => (
                <SidebarMenuItem key={item.to}>
                  <SidebarMenuButton asChild tooltip={item.label}>
                    <NavLink
                      to={item.to}
                      className={({ isActive }) =>
                        cn(
                          isActive &&
                            "bg-sidebar-accent font-medium text-sidebar-accent-foreground",
                        )
                      }
                    >
                      <item.icon />
                      <span>{item.label}</span>
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              size="sm"
              className="pointer-events-none h-auto py-1"
              tooltip="当前版本"
            >
              <VersionInfo />
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton asChild tooltip="检查更新">
              <UpdateButtons />
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}

function VersionInfo() {
  const appVersion = useUpdateStore((s) => s.appVersion);
  const info = useUpdateStore((s) => s.info);
  const hasUpdate = !!info?.hasUpdate;

  return (
    <div className="flex w-full min-w-0 items-center gap-1.5">
      <span className="shrink-0 text-[11px] text-muted-foreground">v</span>
      <span className="tnum truncate text-[11px] font-medium text-foreground/75">
        {appVersion}
      </span>
      {hasUpdate ? (
        <Badge className="ml-auto shrink-0 border-transparent bg-win-bg px-1 py-0 text-[10px] text-win-fg">
          可更新
        </Badge>
      ) : null}
    </div>
  );
}

function UpdateButtons() {
  const { isMobile, toggleSidebar } = useSidebar();
  const checking = useUpdateStore((s) => s.checking);
  const installing = useUpdateStore((s) => s.installing);
  const error = useUpdateStore((s) => s.error);
  const info = useUpdateStore((s) => s.info);
  const check = useUpdateStore((s) => s.check);
  const setDialogOpen = useUpdateStore((s) => s.setDialogOpen);
  const hasUpdate = !!info?.hasUpdate;

  return (
    <div
      className="flex w-full flex-col gap-1"
      onClick={() => isMobile && toggleSidebar()}
    >
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
          onClick={(e) => {
            e.preventDefault();
            void check();
          }}
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
          onClick={(e) => {
            e.preventDefault();
            setDialogOpen(true);
          }}
        >
          {installing ? "安装中" : "更新"}
        </Button>
      </div>
    </div>
  );
}
