import { beforeEach, describe, expect, it } from "vitest";
import { applySystemTheme } from "./theme";
import { useThemeStore } from "@/stores/theme";

beforeEach(() => {
  document.documentElement.classList.remove("dark");
  document.documentElement.style.colorScheme = "";
  useThemeStore.getState().setSystem("light");
});

describe("applySystemTheme", () => {
  it("dark: toggles the class, inline color-scheme and the mirror store", () => {
    applySystemTheme("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(useThemeStore.getState().system).toBe("dark");
  });

  it("light: removes the class and restores inline color-scheme", () => {
    applySystemTheme("dark");
    applySystemTheme("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe("light");
    expect(useThemeStore.getState().system).toBe("light");
  });

  it("overrides the inline value wry writes at window creation", () => {
    // wry 建窗时按"系统模式"写入的内联值（可能与应用模式相反）
    document.documentElement.style.colorScheme = "dark";
    applySystemTheme("light");
    // 内联被覆盖，滚动条等原生控件回到浅色
    expect(document.documentElement.style.colorScheme).toBe("light");
  });
});
