import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { ThemeProvider } from "next-themes";
import App from "./App";
import { LogWindow } from "./components/log-window";
import { FilterWindow } from "./components/filter-window";
import { applySystemTheme } from "./lib/theme";
import "./i18n";
import "./index.css";

// 接管右键：全局屏蔽 WebView2 默认菜单；主页卡片右键菜单由 radix
// ContextMenu 自行处理（触发区事件已自阻止默认行为）
window.addEventListener("contextmenu", (e) => e.preventDefault());

// 跟随系统深浅主题：WebView2 的 prefers-color-scheme 不保证随系统实时
// 更新，由后端轮询注册表并 emit system-theme 事件桥接。挂载后延迟一拍
// 应用初始值，避免被 next-themes 的挂载效果覆盖；此后仅事件驱动切换。
// 除 dark 类外还写内联 color-scheme，覆盖 wry 创建窗口时按"系统模式"
// 写入的内联值（否则浅色应用模式下滚动条等仍是深色）。
function SystemThemeSync() {
  useEffect(() => {
    const unlistenPromise = listen<"light" | "dark">("system-theme", (e) =>
      applySystemTheme(e.payload),
    );
    const timer = setTimeout(() => {
      void invoke<"light" | "dark">("system_theme").then(applySystemTheme);
    }, 50);
    return () => {
      clearTimeout(timer);
      void unlistenPromise.then((fn) => fn());
    };
  }, []);
  return null;
}

// 日志/筛选窗口与主窗口共用同一前端包，按窗口标签分流渲染
const label = (window as Record<string, any>).__TAURI_INTERNALS__?.metadata?.currentWindow
  ?.label as string | undefined;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>
      <SystemThemeSync />
      {label === "log" ? <LogWindow /> : label === "filter" ? <FilterWindow /> : <App />}
    </ThemeProvider>
  </React.StrictMode>,
);
