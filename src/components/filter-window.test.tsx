import { beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen } from "@testing-library/react";
import { FilterWindow } from "./filter-window";
import { useFilter } from "@/stores/filter";

const invokeMock = vi.fn<(cmd: string) => Promise<unknown>>();
const filterGetMock = vi.fn(async () => ({
  platform: "all",
  developer: "all",
  developers: ["Tencent", "NetEase"],
}));
const filterSetMock = vi.fn(async (_platform: string | null, _developer: string | null) => {});
const filterHideMock = vi.fn(async () => {});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...(args as [string])),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock("@/lib/api", () => ({
  api: {
    filterGet: () => filterGetMock(),
    filterSet: (platform: string | null, developer: string | null) =>
      filterSetMock(platform, developer),
    filterHide: () => filterHideMock(),
  },
}));

beforeEach(async () => {
  invokeMock.mockClear();
  filterGetMock.mockClear();
  filterSetMock.mockClear();
  filterHideMock.mockClear();
  useFilter.setState({
    platform: "all",
    developer: "all",
    developers: ["Tencent", "NetEase"],
    listening: false,
  });
  if (!i18next.isInitialized) {
    await i18next.use(initReactI18next).init({
      lng: "zh",
      resources: {
        zh: {
          translation: {
            "MainPage/Action/FilterButton.Content": "筛选",
            "MainPage/Filter/Platform": "平台",
            "MainPage/Filter/Developer": "开发者",
            "MainPage/Filter/AllPlatforms": "全部平台",
            "MainPage/Platform/Ios": "iOS",
            "MainPage/Platform/Ipad": "iPad",
            "MainPage/Platform/Macos": "Mac",
            "MainPage/DeveloperSelectorAllItem.Content": "所有开发者",
            "LogViewerWindow/CloseButton.Content": "关闭",
          },
        },
      },
      keySeparator: false,
      nsSeparator: false,
      interpolation: { escapeValue: false },
    });
  }
  render(<FilterWindow />);
});

describe("FilterWindow", () => {
  it("renders platform segments and developer options", () => {
    expect(screen.getByText("平台")).toBeTruthy();
    expect(screen.getByRole("button", { name: "全部平台" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "iOS" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "iPad" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Mac" })).toBeTruthy();
    expect(screen.getAllByText("所有开发者").length).toBeGreaterThan(0);
  });

  it("selecting a platform persists via filter_set", async () => {
    fireEvent.click(screen.getByRole("button", { name: "iPad" }));

    await vi.waitFor(() => expect(filterSetMock).toHaveBeenCalledWith("ipad", null));
    expect(useFilter.getState().platform).toBe("ipad");
  });

  it("close button hides the window", async () => {
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));

    await vi.waitFor(() => expect(filterHideMock).toHaveBeenCalled());
  });
});
