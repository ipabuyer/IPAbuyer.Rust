import { beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen } from "@testing-library/react";
import { toast } from "sonner";
import zhHans from "@/locales/zh-Hans.json";

const purchaseMock = vi.fn();
const queueAddMock = vi.fn();
const queueStartMock = vi.fn();
const queueCancelMock = vi.fn();
const markMock = vi.fn();
const unmarkMock = vi.fn();
const filterShowMock = vi.fn();
const getSettingsMock = vi.fn(async (..._args: unknown[]) => ({
  countryCode: "cn",
  downloadDirectory: null,
  displayLanguage: "auto",
  detailedIpatoolLog: false,
  passphraseRotationEnabled: false,
  ipatoolFlavor: "main",
  customIpatoolPath: null,
  legacyDbImported: true,
}));
const searchMock = vi.fn(async () => []);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => null),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(async () => {}),
}));
vi.mock("sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), warning: vi.fn(), error: vi.fn() },
}));
vi.mock("@/lib/api", () => ({
  api: {
    getSettings: () => getSettingsMock(),
    search: () => searchMock(),
    purchase: (...a: unknown[]) => purchaseMock(...(a as unknown[])),
    mark: (...a: unknown[]) => markMock(...(a as unknown[])),
    unmark: (...a: unknown[]) => unmarkMock(...(a as unknown[])),
    queueAdd: (...a: unknown[]) => queueAddMock(...(a as unknown[])),
    queueStart: (...a: unknown[]) => queueStartMock(...(a as unknown[])),
    queueCancel: (...a: unknown[]) => queueCancelMock(...(a as unknown[])),
    queueStatus: vi.fn(async () => ({ running: false, items: [] })),
    logsSnapshot: vi.fn(async () => []),
    logsClear: vi.fn(async () => {}),
    syncLastTime: vi.fn(async () => null),
    filterGet: vi.fn(async () => ({ platform: "all", developer: "all", developers: [] })),
    filterSet: vi.fn(async () => {}),
    filterShow: (...a: unknown[]) => filterShowMock(...(a as unknown[])),
    filterHide: vi.fn(async () => {}),
  },
}));

import { HomePage } from "@/pages/home";
import { useLogs } from "@/stores/logs";
import { useQueue } from "@/stores/queue";
import { useSearch } from "@/stores/search";
import { useSession } from "@/stores/session";
import { useFilter } from "@/stores/filter";

const result = (bundleId: string, purchased: string, developer: string) => ({
  bundleId,
  id: "100",
  name: `应用-${bundleId}`,
  developer,
  artworkUrl: null,
  price: "free",
  version: "1.0",
  platform: "ios" as const,
  purchased,
});

beforeEach(async () => {
  purchaseMock.mockReset();
  queueAddMock.mockReset();
  queueStartMock.mockReset().mockResolvedValue(undefined);
  markMock.mockReset().mockResolvedValue(undefined);
  unmarkMock.mockReset().mockResolvedValue(undefined);
  getSettingsMock.mockClear();
  vi.mocked(toast.success).mockClear();
  vi.mocked(toast.info).mockClear();
  vi.mocked(toast.warning).mockClear();
  vi.mocked(toast.error).mockClear();

  useSession.getState().setSession(true, "user@icloud.com", false);
  useSearch.setState({
    query: "测试",
    searching: false,
    lastSearchEmpty: false,
    results: [
      result("com.purchased", "purchased", "Tencent"),
      result("com.free", "not_purchased", "Tencent"),
    ],
  });
  useQueue.setState({ running: false, items: [], listening: false });
  useLogs.setState({ entries: [], listening: false });
  useFilter.setState({
    platform: "all",
    developer: "all",
    developers: [],
    listening: false,
  });

  if (!i18next.isInitialized) {
    await i18next.use(initReactI18next).init({
      lng: "zh",
      resources: { zh: { translation: zhHans } },
      keySeparator: false,
      nsSeparator: false,
      interpolation: { escapeValue: false },
    });
  }
  render(<HomePage />);
});

describe("HomePage", () => {
  it("renders a card per search result with purchase/download action", () => {
    expect(screen.getByText("应用-com.purchased")).toBeTruthy();
    expect(screen.getByText("应用-com.free")).toBeTruthy();
    // 已购卡片 → 下载；未购卡片 → 购买
    expect(screen.getByRole("button", { name: "下载" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "购买" })).toBeTruthy();
  });

  it("purchased filter hides not-purchased cards", () => {
    // "已购买" 文本同时出现在卡片状态中，用 role 精确命中筛选按钮
    fireEvent.click(screen.getByRole("button", { name: "已购买" }));
    expect(screen.queryByText("应用-com.free")).toBeNull();
    expect(screen.getByText("应用-com.purchased")).toBeTruthy();
  });

  it("purchase button invokes the purchase command", async () => {
    purchaseMock.mockResolvedValue({ bundleId: "com.free", outcome: "Purchased", detail: null });
    fireEvent.click(screen.getByRole("button", { name: "购买" }));
    await vi.waitFor(() =>
      expect(purchaseMock).toHaveBeenCalledWith("com.free", "free", "not_purchased", "ios"),
    );
  });

  it("purchase does not auto-open the log window", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const invokeMock = vi.mocked(invoke);
    purchaseMock.mockResolvedValue({ bundleId: "com.free", outcome: "Purchased", detail: null });
    fireEvent.click(screen.getByRole("button", { name: "购买" }));
    await vi.waitFor(() => expect(purchaseMock).toHaveBeenCalled());
    const calls = invokeMock.mock.calls.map((c) => c[0]);
    expect(calls).not.toContain("logs_show_window");
  });

  it("purchased card queues a download", async () => {
    queueAddMock.mockResolvedValue("Added");
    fireEvent.click(screen.getByRole("button", { name: "下载" }));
    await vi.waitFor(() => expect(queueAddMock).toHaveBeenCalled());
  });

  it("successful purchase flips the card to the download action", async () => {
    purchaseMock.mockResolvedValue({ bundleId: "com.free", outcome: "Purchased", detail: null });
    fireEvent.click(screen.getByRole("button", { name: "购买" }));
    // 本地状态更新后，原"购买"卡片变为"下载"（两处下载按钮：已购 + 刚转为已购）
    await vi.waitFor(() =>
      expect(screen.getAllByRole("button", { name: "下载" }).length).toBe(2),
    );
  });

  it("purchase toast interpolates the app name instead of {{0}}", async () => {
    purchaseMock.mockResolvedValue({ bundleId: "com.free", outcome: "Purchased", detail: null });
    fireEvent.click(screen.getByRole("button", { name: "购买" }));
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith("购买成功: 应用-com.free"),
    );
  });

  it("purchase failure toast fills name and reason", async () => {
    purchaseMock.mockResolvedValue({ bundleId: "com.free", outcome: "Failed", detail: "boom" });
    fireEvent.click(screen.getByRole("button", { name: "购买" }));
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("购买失败: 应用-com.free - boom"),
    );
  });

  it("three-dot menu marks a not-purchased card as purchased", async () => {
    markMock.mockResolvedValue(undefined);
    const freeCard = screen.getByText("应用-com.free").closest("[data-slot=card]")!;
    const ellipsis = [...freeCard.querySelectorAll("button")].find((b) =>
      b.querySelector("svg.lucide-ellipsis"),
    );
    expect(ellipsis).toBeTruthy();
    fireEvent.pointerDown(ellipsis!);
    fireEvent.click(ellipsis!);

    const item = await screen.findByText("标记为已购买");
    fireEvent.click(item);
    await vi.waitFor(() =>
      expect(markMock).toHaveBeenCalledWith("com.free", "purchased", "ios"),
    );
  });

  it("right-click on a card opens the same menu", async () => {
    markMock.mockResolvedValue(undefined);
    const freeCard = screen.getByText("应用-com.free").closest("[data-slot=card]")!;
    fireEvent.contextMenu(freeCard);

    const item = await screen.findByText("标记为已购买");
    fireEvent.click(item);
    await vi.waitFor(() => expect(markMock).toHaveBeenCalledWith("com.free", "purchased", "ios"));
  });

  it("macOS and iPad cards show platform badges", async () => {
    const macResult = {
      ...result("com.mac", "not_purchased", "Apple"),
      platform: "macos" as const,
    };
    const ipadResult = {
      ...result("com.ipad", "not_purchased", "Apple"),
      platform: "ipad" as const,
    };
    useSearch.setState({
      query: "测试",
      searching: false,
      lastSearchEmpty: false,
      results: [...useSearch.getState().results, macResult, ipadResult],
    });

    expect(await screen.findByText("Mac")).toBeTruthy();
    expect(screen.getByText("iPad")).toBeTruthy();
  });

  it("paid app (blocked) shows 无法购买 with a disabled purchase action", async () => {
    // 回归：core 的无法购买 token 是 "blocked"（status_policy），不是 "purchase_blocked"
    const paidResult = { ...result("com.paid", "blocked", "NetEase"), price: "6.00" };
    useSearch.setState({
      query: "测试",
      searching: false,
      lastSearchEmpty: false,
      results: [...useSearch.getState().results, paidResult],
    });

    const card = await screen.findByText("应用-com.paid");
    expect(screen.getByText("无法购买")).toBeTruthy();
    const cardEl = card.closest("[data-slot=card]")!;
    const purchaseButton = [...cardEl.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("购买"),
    ) as HTMLButtonElement;
    // 购买按钮保留但禁用（保持与其它卡片排版一致），信息图标悬停查看原因
    expect(purchaseButton).toBeTruthy();
    expect(purchaseButton.disabled).toBe(true);
    expect(cardEl.querySelector("svg.lucide-info")).toBeTruthy();
    expect(cardEl.querySelectorAll("svg.lucide-ellipsis").length).toBe(1);
  });

  it("paid app marked then unmarked returns to blocked, not purchasable", async () => {
    // 回归：付费 App 标记已购再取消，应按价格恢复"无法购买"，而不是卡出购买按钮
    markMock.mockResolvedValue(undefined);
    unmarkMock.mockResolvedValue(undefined);
    const paidResult = { ...result("com.paid", "blocked", "NetEase"), price: "6.00" };
    useSearch.setState({
      query: "测试",
      searching: false,
      lastSearchEmpty: false,
      results: [...useSearch.getState().results, paidResult],
    });
    await screen.findByText("应用-com.paid");

    function paidCard() {
      return screen.getByText("应用-com.paid").closest("[data-slot=card]") as HTMLElement;
    }
    async function menuItemClick(label: string) {
      const card = paidCard();
      const ellipsis = [...card.querySelectorAll("button")].find((b) =>
        b.querySelector("svg.lucide-ellipsis"),
      );
      fireEvent.pointerDown(ellipsis!);
      fireEvent.click(ellipsis!);
      fireEvent.click(await screen.findByText(label));
    }

    await menuItemClick("标记为已购买");
    await vi.waitFor(() => expect(markMock).toHaveBeenCalledWith("com.paid", "purchased", "ios"));
    await vi.waitFor(() => expect(paidCard().textContent).toContain("已购买"));

    await menuItemClick("标记为未购买");
    await vi.waitFor(() => expect(unmarkMock).toHaveBeenCalledWith("com.paid", "ios"));
    await vi.waitFor(() => expect(paidCard().textContent).toContain("无法购买"));
    const purchaseButton = [...paidCard().querySelectorAll("button")].find((b) =>
      b.textContent?.includes("购买"),
    ) as HTMLButtonElement;
    expect(purchaseButton.disabled).toBe(true);
  });

  it("parallel purchases spin all involved buttons", async () => {
    // 回归：busy 状态曾按单个 bundleId 存储，多项购买并行时只有最后一个转圈
    useSearch.setState({
      query: "测试",
      searching: false,
      lastSearchEmpty: false,
      results: [
        ...useSearch.getState().results,
        { ...result("com.free2", "not_purchased", "Tencent"), name: "应用-com.free2" },
      ],
    });
    const resolvers: Array<() => void> = [];
    purchaseMock.mockImplementation(
      () => new Promise((resolve) => resolvers.push(() => resolve({ bundleId: "", outcome: "Purchased", detail: null }))),
    );

    await screen.findByText("应用-com.free2");
    const buyButtons = screen.getAllByRole("button", { name: "购买" });
    expect(buyButtons.length).toBe(2);
    fireEvent.click(buyButtons[0]);
    fireEvent.click(buyButtons[1]);

    await vi.waitFor(() => expect(purchaseMock).toHaveBeenCalledTimes(2));
    // 两个购买中的按钮都在转圈
    await vi.waitFor(() =>
      expect(document.querySelectorAll("svg.animate-spin").length).toBe(2),
    );

    resolvers.forEach((resolve) => resolve());
    await vi.waitFor(() =>
      expect(document.querySelectorAll("svg.animate-spin").length).toBe(0),
    );
  });

  it("filter button opens the independent filter window", async () => {
    fireEvent.click(screen.getByRole("button", { name: "筛选" }));
    await vi.waitFor(() => expect(filterShowMock).toHaveBeenCalled());
  });

  it("filter store drives the platform filter", async () => {
    const macResult = {
      ...result("com.mac", "not_purchased", "Apple"),
      platform: "macos" as const,
    };
    useSearch.setState({
      query: "测试",
      searching: false,
      lastSearchEmpty: false,
      results: [...useSearch.getState().results, macResult],
    });
    const { useFilter } = await import("@/stores/filter");
    useFilter.setState({ platform: "macos" });

    expect(await screen.findByText("应用-com.mac")).toBeTruthy();
    expect(screen.queryByText("应用-com.free")).toBeNull();
    expect(screen.queryByText("应用-com.purchased")).toBeNull();
    useFilter.setState({ platform: "all" });
  });
});
