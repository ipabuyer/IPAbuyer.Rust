import { beforeEach, describe, expect, it } from "vitest";
import { useThemeStore } from "./theme";

beforeEach(() => {
  useThemeStore.setState({ system: "light" });
});

describe("useThemeStore", () => {
  it("默认浅色", () => {
    expect(useThemeStore.getState().system).toBe("light");
  });

  it("setSystem 更新系统主题镜像", () => {
    useThemeStore.getState().setSystem("dark");
    expect(useThemeStore.getState().system).toBe("dark");

    useThemeStore.getState().setSystem("light");
    expect(useThemeStore.getState().system).toBe("light");
  });
});
