import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));

const filterGetMock = vi.fn();
const filterSetMock = vi.fn();
vi.mock("@/lib/api", () => ({
  api: {
    filterGet: (...args: unknown[]) => filterGetMock(...(args as [])),
    filterSet: (...args: unknown[]) => filterSetMock(...(args as unknown[])),
  },
}));

import { listen } from "@tauri-apps/api/event";
import { useFilter } from "./filter";

const listenMock = vi.mocked(listen);

function changedHandler() {
  return listenMock.mock.calls[0][1] as unknown as (event: { payload: unknown }) => void;
}

beforeEach(() => {
  filterGetMock.mockReset().mockResolvedValue({
    platform: "all",
    developer: "all",
    developers: [],
  });
  filterSetMock.mockReset().mockResolvedValue(undefined);
  listenMock.mockClear();
  useFilter.setState({ platform: "all", developer: "all", developers: [], listening: false });
});

describe("useFilter", () => {
  it("init 拉取后端筛选并注册 filter-changed 监听（幂等）", async () => {
    filterGetMock.mockResolvedValue({
      platform: "ios",
      developer: "Apple",
      developers: ["Apple"],
    });
    await useFilter.getState().init();
    await useFilter.getState().init();

    expect(filterGetMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledWith("filter-changed", expect.any(Function));
    expect(useFilter.getState().platform).toBe("ios");
    expect(useFilter.getState().developer).toBe("Apple");
    expect(useFilter.getState().developers).toEqual(["Apple"]);
  });

  it("init 拉取失败时保留本地状态并照常监听", async () => {
    filterGetMock.mockRejectedValue("boom");
    await useFilter.getState().init();

    expect(useFilter.getState().platform).toBe("all");
    expect(listenMock).toHaveBeenCalledTimes(1);
  });

  it("filter-changed 事件同步两个窗口共享的筛选选择", async () => {
    await useFilter.getState().init();
    changedHandler()({
      payload: { platform: "macos", developer: "NetEase", developers: ["NetEase", "Tencent"] },
    });

    expect(useFilter.getState().platform).toBe("macos");
    expect(useFilter.getState().developer).toBe("NetEase");
    expect(useFilter.getState().developers).toEqual(["NetEase", "Tencent"]);
  });

  it("setPlatform 更新本地并只回传平台维度", () => {
    useFilter.getState().setPlatform("ipad");

    expect(useFilter.getState().platform).toBe("ipad");
    expect(filterSetMock).toHaveBeenCalledWith("ipad", null);
  });

  it("setDeveloper 更新本地并只回传开发者维度", () => {
    useFilter.getState().setDeveloper("Apple");

    expect(useFilter.getState().developer).toBe("Apple");
    expect(filterSetMock).toHaveBeenCalledWith(null, "Apple");
  });
});
