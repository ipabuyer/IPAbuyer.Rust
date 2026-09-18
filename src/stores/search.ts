import { create } from "zustand";
import { api } from "@/lib/api";
import type { SearchResultItem } from "@/lib/types";
// @ts-ignore 临时诊断：统计模块实例化次数
(window as any).__searchInstances = ((window as any).__searchInstances ?? 0) + 1;

// 搜索状态：标题栏搜索框与主页结果列表共享
type SearchState = {
  query: string;
  results: SearchResultItem[];
  searching: boolean;
  lastSearchEmpty: boolean;
  setQuery: (query: string) => void;
  search: () => Promise<void>;
  /** 清空全部搜索状态（切换国家/地区后旧商店结果已失效）。 */
  reset: () => void;
};

export const useSearch = create<SearchState>((set, get) => ({
  query: "",
  results: [],
  searching: false,
  lastSearchEmpty: false,
  setQuery: (query) => set({ query }),
  reset: () => set({ query: "", results: [], searching: false, lastSearchEmpty: false }),
  search: async () => {
    const query = get().query.trim();
    if (!query || get().searching) return;
    set({ searching: true });
    try {
      const results = await api.search(query);
      set({ results, lastSearchEmpty: results.length === 0 });
    } finally {
      set({ searching: false });
    }
  },
}));
