import { create } from "zustand";

// 系统明暗主题镜像（由 main.tsx 的 SystemThemeSync 从后端桥接维护：
// 启动查询 + system-theme 事件）。sonner 等依赖主题的组件从这里取值，
// 避免 WebView2 的 prefers-color-scheme 媒体查询不随系统实时更新的问题。
type ThemeState = {
  system: "light" | "dark";
  setSystem: (theme: "light" | "dark") => void;
};

export const useThemeStore = create<ThemeState>((set) => ({
  system: "light",
  setSystem: (system) => set({ system }),
}));
