import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { api } from "@/lib/api";
import type { FilterSelection } from "@/lib/types";
import type { PlatformFilter } from "@/lib/status";

// 主页筛选（对应独立筛选窗口）：选择保存在后端，经 filter-changed 事件
// 同步筛选窗口与主窗口（两个 WebView 的 zustand 实例不共享）。
type FilterState = {
  platform: PlatformFilter;
  developer: string;
  developers: string[];
  listening: boolean;
  init: () => Promise<void>;
  setPlatform: (platform: PlatformFilter) => void;
  setDeveloper: (developer: string) => void;
};

export const useFilter = create<FilterState>((set, get) => ({
  platform: "all",
  developer: "all",
  developers: [],
  listening: false,
  init: async () => {
    if (get().listening) return;
    set({ listening: true });
    const current = await api.filterGet().catch(() => null);
    if (current) set(current);
    await listen<FilterSelection>("filter-changed", (event) => {
      set(event.payload);
    }).catch(() => {});
  },
  setPlatform: (platform) => {
    set({ platform });
    void api.filterSet(platform, null);
  },
  setDeveloper: (developer) => {
    set({ developer });
    void api.filterSet(null, developer);
  },
}));
