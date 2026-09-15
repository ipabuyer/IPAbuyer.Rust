import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { api } from "@/lib/api";
import type { QueueItem } from "@/lib/types";

// 下载队列镜像（对应原 C# ObservableCollection 镜像 + 200ms 轮询）
type QueueState = {
  running: boolean;
  items: QueueItem[];
  listening: boolean;
  init: () => Promise<void>;
  refresh: () => Promise<void>;
};

export const useQueue = create<QueueState>((set, get) => ({
  running: false,
  items: [],
  listening: false,
  init: async () => {
    if (get().listening) return;
    set({ listening: true });
    await listen<{ running: boolean; items: QueueItem[] }>("queue-status", (event) => {
      set({ running: event.payload.running, items: event.payload.items });
    });
    await listen("queue-finished", () => {
      void get().refresh();
    });
    await get().refresh();
  },
  refresh: async () => {
    const status = await api.queueStatus().catch(() => null);
    if (status) set({ running: status.running, items: status.items });
  },
}));
