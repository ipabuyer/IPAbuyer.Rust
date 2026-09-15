import { useThemeStore } from "@/stores/theme";

/** 应用系统主题：切换 dark 类、写内联 color-scheme（覆盖 wry 建窗时写入
 * 的"系统模式"内联值）、更新镜像 store。SystemThemeSync 与测试共用。 */
export function applySystemTheme(theme: "light" | "dark") {
  document.documentElement.classList.toggle("dark", theme === "dark");
  document.documentElement.style.colorScheme = theme;
  useThemeStore.getState().setSystem(theme);
}
