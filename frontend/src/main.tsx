import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { applyTheme, useAppStore } from "@/stores/appStore";

// 首帧前确定主题，避免闪白/闪黑
applyTheme(useAppStore.getState().config.theme);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
