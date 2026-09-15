import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { api } from "@/lib/api";
import type { LogEntry } from "@/lib/types";

// 全局日志（对应原 C# UiLogStore + LogViewerWindow，现为侧滑面板）
type LogsState = {
  entries: LogEntry[];
  open: boolean;
  listening: boolean;
  init: () => Promise<void>;
  setOpen: (open: boolean) => void;
  clear: () => void;
};

export const useLogs = create<LogsState>((set, get) => ({
  entries: [],
  open: false,
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
  setOpen: (open) => set({ open }),
  clear: () => {
    void api.logsClear();
    set({ entries: [] });
  },
}));
