import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { toast } from "sonner";
import zhHans from "@/locales/zh-Hans.json";

const defaultConfig = (overrides: Record<string, unknown> = {}) => ({
  countryCode: "cn",
  downloadDirectory: null,
  displayLanguage: "auto",
  detailedIpatoolLog: false,
  passphraseRotationEnabled: true,
  ipatoolFlavor: "main",
  customIpatoolPath: null,
  legacyDbImported: true,
  ...overrides,
});

const getSettingsMock = vi.fn();
const defaultDirMock = vi.fn();
const syncLastTimeMock = vi.fn();
const legacyDbExistsMock = vi.fn();
const syncStartMock = vi.fn();
const setDisplayLanguageMock = vi.fn();
const setCountryCodeMock = vi.fn();
const setPassphraseRotationMock = vi.fn();
const resetDownloadDirMock = vi.fn();
const legacyDbImportMock = vi.fn();
const listStorefrontsMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "2026.9.16") }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => {}) }));
vi.mock("sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), warning: vi.fn(), error: vi.fn() },
}));
vi.mock("@/lib/api", () => ({
  api: {
    getSettings: (...a: unknown[]) => getSettingsMock(...(a as [])),
    defaultDownloadDirectory: (...a: unknown[]) => defaultDirMock(...(a as [])),
    syncLastTime: (...a: unknown[]) => syncLastTimeMock(...(a as [])),
    legacyDbExists: (...a: unknown[]) => legacyDbExistsMock(...(a as [])),
    syncStart: (...a: unknown[]) => syncStartMock(...(a as [])),
    setDisplayLanguage: (...a: unknown[]) => setDisplayLanguageMock(...(a as [string])),
    setCountryCode: (...a: unknown[]) => setCountryCodeMock(...(a as [string])),
    setPassphraseRotation: (...a: unknown[]) =>
      setPassphraseRotationMock(...(a as [boolean])),
    resetDownloadDirectory: (...a: unknown[]) => resetDownloadDirMock(...(a as [])),
    legacyDbImport: (...a: unknown[]) => legacyDbImportMock(...(a as [])),
    listStorefronts: (...a: unknown[]) => listStorefrontsMock(...(a as [])),
  },
}));

import { SettingsPage } from "@/pages/settings";
import { useSearch } from "@/stores/search";
import { useSession } from "@/stores/session";

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
  getSettingsMock.mockReset().mockResolvedValue(defaultConfig());
  defaultDirMock.mockReset().mockResolvedValue("C:\\Users\\t\\Downloads");
  syncLastTimeMock.mockReset().mockResolvedValue(null);
  legacyDbExistsMock.mockReset().mockResolvedValue(false);
  syncStartMock.mockReset();
  setDisplayLanguageMock.mockReset().mockResolvedValue(defaultConfig());
  setCountryCodeMock.mockReset();
  setPassphraseRotationMock.mockReset().mockResolvedValue(defaultConfig());
  resetDownloadDirMock.mockReset().mockResolvedValue(defaultConfig());
  legacyDbImportMock.mockReset().mockResolvedValue(undefined);
  listStorefrontsMock.mockReset().mockResolvedValue([]);
  useSearch.setState({ query: "", results: [], searching: false, lastSearchEmpty: false });
  useSession.getState().reset();
});

async function renderPage() {
  const view = render(<SettingsPage />);
  // 配置加载完成后各设置卡片出现
  await screen.findByText("国家/地区代码");
  return view;
}

describe("SettingsPage", () => {
  it("配置未加载时显示加载动画", () => {
    getSettingsMock.mockReturnValue(new Promise(() => {}));
    const { container } = render(<SettingsPage />);

    expect(container.querySelector(".animate-spin")).toBeTruthy();
    expect(screen.queryByText("国家/地区代码")).toBeNull();
  });

  it("加载后渲染全部设置卡片与版本号", async () => {
    syncLastTimeMock.mockResolvedValue("2026-09-20T10:00:00+08:00");
    await renderPage();

    for (const header of [
      "显示语言",
      "国家/地区代码",
      "下载目录",
      "加密密钥轮换",
      "刷新已购买列表",
      "开发者网站",
      "项目仓库",
      "软件版本",
    ]) {
      expect(screen.getByText(header)).toBeTruthy();
    }
    expect(screen.getByText("2026.9.16")).toBeTruthy();
    expect(screen.getByText("当前: CN")).toBeTruthy();
    expect(screen.getByText("上次同步：", { exact: false })).toBeTruthy();
  });

  it("未登录点击同步提示登录且不调用 syncStart", async () => {
    await renderPage();
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));

    await waitFor(() =>
      expect(toast.warning).toHaveBeenCalledWith("尚未登录", {
        description: "请先在账户页登录 Apple 账户。",
      }),
    );
    expect(syncStartMock).not.toHaveBeenCalled();
  });

  it("测试账户点击同步提示不执行同步", async () => {
    useSession.getState().setSession(true, "test", true);
    await renderPage();
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));

    await waitFor(() =>
      expect(toast.info).toHaveBeenCalledWith("测试账户", {
        description: "测试账户不执行同步。",
      }),
    );
    expect(syncStartMock).not.toHaveBeenCalled();
  });

  it("已登录同步：开日志窗口、按结果提示并刷新上次同步时间", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    syncStartMock.mockResolvedValue({ outcome: "Completed", synced: 2, total: 5, message: null });
    await renderPage();
    useSession.getState().setSession(true, "user@icloud.com", false);

    fireEvent.click(screen.getByRole("button", { name: "刷新" }));

    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith("同步中… 2/5"),
    );
    expect(syncStartMock).toHaveBeenCalledTimes(1);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("logs_show_window");
    await waitFor(() => expect(syncLastTimeMock).toHaveBeenCalledTimes(2));
  });

  it("同步失败提示错误详情", async () => {
    syncStartMock.mockRejectedValue("network down");
    await renderPage();
    useSession.getState().setSession(true, "user@icloud.com", false);

    fireEvent.click(screen.getByRole("button", { name: "刷新" }));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("同步未完成，请稍后重试；详细信息见日志。", {
        description: "network down",
      }),
    );
  });

  it("密钥轮换开关保存配置", async () => {
    await renderPage();
    const sw = screen.getByRole("switch");
    expect((sw as HTMLButtonElement).getAttribute("data-state")).toBe("checked");

    fireEvent.click(sw);

    await waitFor(() => expect(setPassphraseRotationMock).toHaveBeenCalledWith(false));
  });

  it("国家码对话框：列出商店、选择后保存并清空主页搜索", async () => {
    setCountryCodeMock.mockResolvedValue(defaultConfig({ countryCode: "jp" }));
    listStorefrontsMock.mockResolvedValue([
      ["JP", "Japan"],
      ["US", "United States"],
    ]);
    useSearch.setState({ query: "wechat", results: [], searching: false, lastSearchEmpty: false });
    await renderPage();

    fireEvent.click(screen.getByRole("button", { name: "修改" }));
    expect(await screen.findByText("选择国家/地区")).toBeTruthy();
    expect(listStorefrontsMock).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("JP")).toBeTruthy();

    // 国名经 Intl.DisplayNames 按界面语言解析（zh → 日本），点击所在行按钮
    fireEvent.click(screen.getByText("JP").closest("button")!);

    await waitFor(() => expect(setCountryCodeMock).toHaveBeenCalledWith("jp"));
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith("国家/地区代码已更新为 jp"));
    // 国家码变更后旧商店的搜索结果已失效
    await waitFor(() => expect(useSearch.getState().query).toBe(""));
  });

  it("国家码搜索无匹配时显示空状态", async () => {
    listStorefrontsMock.mockResolvedValue([["JP", "Japan"]]);
    await renderPage();

    fireEvent.click(screen.getByRole("button", { name: "修改" }));
    await screen.findByText("选择国家/地区");
    fireEvent.change(screen.getByPlaceholderText("搜索国家、地区或代码"), {
      target: { value: "zzz" },
    });

    expect(await screen.findByText("未找到匹配的国家或地区")).toBeTruthy();
  });

  it("检测到旧版数据库且未导入时显示导入卡片，导入成功后隐藏", async () => {
    legacyDbExistsMock.mockResolvedValue(true);
    getSettingsMock.mockResolvedValue(defaultConfig({ legacyDbImported: false }));
    await renderPage();

    expect(await screen.findByText("导入旧版数据")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "导入" }));

    await waitFor(() => expect(toast.success).toHaveBeenCalledWith("已导入旧版 IPAbuyer 的已购记录。"));
    await waitFor(() => expect(screen.queryByText("导入旧版数据")).toBeNull());
  });

  it("已导入过旧版数据库时不显示导入卡片", async () => {
    legacyDbExistsMock.mockResolvedValue(true);
    getSettingsMock.mockResolvedValue(defaultConfig({ legacyDbImported: true }));
    await renderPage();

    expect(screen.queryByText("导入旧版数据")).toBeNull();
  });
});
