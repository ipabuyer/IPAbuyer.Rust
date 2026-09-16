import { create } from "zustand";
import i18next from "i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { api } from "@/lib/api";
import type { LogEntry } from "@/lib/types";

// 全局日志（对应原 C# UiLogStore + LogViewerWindow 独立窗口）。
// setOpen 控制日志窗口显隐（Rust logs_show_window/logs_hide_window），
// 主窗口与日志窗口各自的 store 实例都经 log-append 事件接收增量。
type LogsState = {
  entries: LogEntry[];
  listening: boolean;
  init: () => Promise<void>;
  setOpen: (open: boolean) => void;
  clear: () => void;
};

export const useLogs = create<LogsState>((set, get) => ({
  entries: [],
  listening: false,
  init: async () => {
    if (get().listening) return;
    set({ listening: true });
    const snapshot = await api.logsSnapshot().catch(() => []);
    set({ entries: snapshot });
    await listen<LogEntry[]>("log-append", (event) => {
      set((s) => ({ entries: [...s.entries, ...event.payload].slice(-1000) }));
    });
  },
  setOpen: (open) => {
    // 创建窗口时传入当前语言的标题（原生标题栏无法使用前端 i18n 资源）；
    // 用 i18next 单例而非 @/i18n（后者导入即初始化，会覆盖测试环境的语言设置）
    if (open) {
      void invoke("logs_show_window", { title: i18next.t("LogViewerWindow/TitleBar.Title") });
    } else {
      void invoke("logs_hide_window");
    }
  },
  clear: () => {
    void api.logsClear();
    set({ entries: [] });
  },
}));
