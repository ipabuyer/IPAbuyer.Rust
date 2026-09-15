import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "next-themes";
import App from "./App";
import { LogWindow } from "./components/log-window";
import "./i18n";
import "./index.css";

// 日志窗口与主窗口共用同一前端包，按窗口标签分流渲染
const label = (window as Record<string, any>).__TAURI_INTERNALS__?.metadata?.currentWindow
  ?.label as string | undefined;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThemeProvider>{label === "log" ? <LogWindow /> : <App />}</ThemeProvider>
  </React.StrictMode>,
);
