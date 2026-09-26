import { createHashRouter, Navigate } from "react-router-dom";
import { AppLayout } from "@/app/AppLayout";
import { GameInfoView } from "@/features/gameinfo/GameInfoView";
import { HistoryView } from "@/features/history/HistoryView";
import { SettingsView } from "@/features/settings/SettingsView";

/** HashRouter：Tauri 生产环境走 tauri:// 协议，hash 路由免去 base path 适配 */
export const router = createHashRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <Navigate to="/history" replace /> },
      { path: "history", element: <HistoryView /> },
      { path: "gameinfo", element: <GameInfoView /> },
      { path: "settings", element: <SettingsView /> },
      { path: "*", element: <Navigate to="/history" replace /> },
    ],
  },
]);
