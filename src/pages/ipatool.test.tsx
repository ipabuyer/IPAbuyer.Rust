import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { toast } from "sonner";
import zhHans from "@/locales/zh-Hans.json";

const defaultInfo = (overrides: Record<string, unknown> = {}) => ({
  flavor: "main",
  customPath: null,
  builtinVersion: "2.6.0",
  activePath: "C:\\app\\ipatool.exe",
  builtinAvailable: true,
  dataDirectory: "C:\\Users\\t\\.ipatool",
  ...overrides,
});

const infoMock = vi.fn();
const getSettingsMock = vi.fn();
const defaultDirMock = vi.fn();
const setDetailedLogMock = vi.fn();
const exportMock = vi.fn();
const clearDataMock = vi.fn();
const setFlavorMock = vi.fn();
const setCustomPathMock = vi.fn();
const deleteCustomMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => {}) }));
vi.mock("sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), warning: vi.fn(), error: vi.fn() },
}));
vi.mock("@/lib/api", () => ({
  api: {
    ipatoolInfo: (...a: unknown[]) => infoMock(...(a as [])),
    getSettings: (...a: unknown[]) => getSettingsMock(...(a as [])),
    defaultDownloadDirectory: (...a: unknown[]) => defaultDirMock(...(a as [])),
    setDetailedLog: (...a: unknown[]) => setDetailedLogMock(...(a as [boolean])),
    ipatoolExport: (...a: unknown[]) => exportMock(...(a as [])),
    ipatoolClearData: (...a: unknown[]) => clearDataMock(...(a as [])),
    ipatoolSetFlavor: (...a: unknown[]) => setFlavorMock(...(a as [string])),
    ipatoolSetCustomPath: (...a: unknown[]) => setCustomPathMock(...(a as [string])),
    ipatoolDeleteCustom: (...a: unknown[]) => deleteCustomMock(...(a as [])),
  },
}));

import { IpatoolPage } from "@/pages/ipatool";

beforeAll(async () => {
  if (!i18next.isInitialized) {
    await i18next.use(initReactI18next).init({
      lng: "zh",
      resources: { zh: { translation: zhHans } },
      keySeparator: false,
      nsSeparator: false,
      interpolation: { escapeValue: false },
    });
  }
});

beforeEach(() => {
  infoMock.mockReset().mockResolvedValue(defaultInfo());
  getSettingsMock.mockReset().mockResolvedValue({
    countryCode: "cn",
    downloadDirectory: null,
    displayLanguage: "auto",
    detailedIpatoolLog: false,
    passphraseRotationEnabled: true,
    ipatoolFlavor: "main",
    customIpatoolPath: null,
    legacyDbImported: true,
  });
  defaultDirMock.mockReset().mockResolvedValue("C:\\Users\\t\\Downloads");
  setDetailedLogMock.mockReset().mockResolvedValue(undefined);
  exportMock.mockReset().mockResolvedValue("C:\\Users\\t\\Downloads\\ipatool.exe");
  clearDataMock.mockReset().mockResolvedValue(undefined);
  setFlavorMock.mockReset().mockResolvedValue(undefined);
  setCustomPathMock.mockReset().mockResolvedValue(undefined);
  deleteCustomMock.mockReset().mockResolvedValue(undefined);
});

async function renderPage() {
  render(<IpatoolPage />);
  await screen.findByText("内置 ipatool");
}

describe("IpatoolPage", () => {
  it("信息未加载时显示加载动画", () => {
    infoMock.mockReturnValue(new Promise(() => {}));
    const { container } = render(<IpatoolPage />);

    expect(container.querySelector(".animate-spin")).toBeTruthy();
    expect(screen.queryByText("内置 ipatool")).toBeNull();
  });

  it("加载后渲染全部卡片与内置版本号", async () => {
    await renderPage();

    for (const header of [
      "内置 ipatool",
      "自定义 ipatool.exe",
      "ipatool 版本要求",
      "显示详细日志",
      "清空 ipatool 数据",
      "ipatool 仓库",
    ]) {
      expect(screen.getByText(header)).toBeTruthy();
    }
    expect(screen.getByText("release@2.6.0")).toBeTruthy();
    expect(screen.getByText("≥ 2.5.0")).toBeTruthy();
  });

  it("使用内置版时内置卡片显示当前使用徽章", async () => {
    const { container } = render(<IpatoolPage />);
    await screen.findByText("release@2.6.0");

    expect(screen.getAllByText("当前使用").length).toBe(1);
    // 未设置自定义路径时不显示"使用"按钮
    expect(screen.queryByRole("button", { name: "使用" })).toBeNull();
    expect(container.textContent).not.toContain("D:\\tools");
  });

  it("使用自定义版时自定义卡片显示徽章与路径", async () => {
    infoMock.mockResolvedValue(
      defaultInfo({ flavor: "custom", customPath: "D:\\tools\\ipatool.exe" }),
    );
    await renderPage();

    expect(screen.getAllByText("当前使用").length).toBe(1);
    expect(screen.getByText("D:\\tools\\ipatool.exe")).toBeTruthy();
    expect(screen.getByRole("button", { name: "使用" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "更换" })).toBeTruthy();
  });

  it("导出经确认对话框执行并提示成功", async () => {
    await renderPage();
    fireEvent.click(screen.getByRole("button", { name: "导出" }));

    expect(await screen.findByText("确认导出")).toBeTruthy();
    // 确认对话框内同样有"导出"按钮（触发器 + 主按钮），取对话框中的那个
    const exportButtons = screen.getAllByRole("button", { name: "导出" });
    fireEvent.click(exportButtons[exportButtons.length - 1]);

    await waitFor(() => expect(exportMock).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        "内置正式版 ipatool 已导出到：C:\\Users\\t\\Downloads\\ipatool.exe",
      ),
    );
  });

  it("清空数据经确认对话框执行并提示成功", async () => {
    await renderPage();
    fireEvent.click(screen.getByRole("button", { name: "清空数据" }));

    expect(await screen.findByText("此操作不可恢复", { exact: false })).toBeTruthy();
    const clearButtons = screen.getAllByRole("button", { name: "清空数据" });
    fireEvent.click(clearButtons[clearButtons.length - 1]);

    await waitFor(() => expect(clearDataMock).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith("ipatool 数据目录已清空。"),
    );
  });

  it("详细日志开关立即保存", async () => {
    await renderPage();
    const sw = screen.getByRole("switch");
    expect((sw as HTMLButtonElement).getAttribute("data-state")).toBe("unchecked");

    fireEvent.click(sw);

    expect((sw as HTMLButtonElement).getAttribute("data-state")).toBe("checked");
    await waitFor(() => expect(setDetailedLogMock).toHaveBeenCalledWith(true));
  });

  it("导出失败提示错误", async () => {
    exportMock.mockRejectedValue("boom");
    await renderPage();
    fireEvent.click(screen.getByRole("button", { name: "导出" }));
    const exportButtons = await screen.findAllByRole("button", { name: "导出" });
    fireEvent.click(exportButtons[exportButtons.length - 1]);

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("导出 ipatool.exe 失败：boom"),
    );
  });
});
