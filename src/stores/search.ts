import { create } from "zustand";
import { api } from "@/lib/api";
import type { SearchResultItem } from "@/lib/types";

// 搜索状态：标题栏搜索框与主页结果列表共享
type SearchState = {
  query: string;
  results: SearchResultItem[];
  searching: boolean;
  lastSearchEmpty: boolean;
  setQuery: (query: string) => void;
  search: () => Promise<void>;
};

export const useSearch = create<SearchState>((set, get) => ({
  query: "",
  results: [],
  searching: false,
  lastSearchEmpty: false,
  setQuery: (query) => set({ query }),
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
