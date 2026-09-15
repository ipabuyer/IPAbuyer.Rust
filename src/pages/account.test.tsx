import { beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen } from "@testing-library/react";
import zhHans from "@/locales/zh-Hans.json";

// vi.mock 会提升到文件顶部，mock 函数须用 vi.hoisted 创建
const { invokeMock, openUrlMock, toastMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(cmd: string, args?: unknown) => Promise<unknown>>(),
  openUrlMock: vi.fn<(url: string) => Promise<void>>(async () => {}),
  toastMock: { warning: vi.fn(), success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...(args as [string])),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...(args as [string])),
}));
vi.mock("sonner", () => ({ toast: toastMock }));

import { AccountPage } from "@/pages/account";

beforeEach(async () => {
  invokeMock.mockClear();
  openUrlMock.mockClear();
  toastMock.warning.mockClear();
  toastMock.success.mockClear();
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "settings_get_passphrase") return Promise.resolve("abc123");
    if (cmd === "logs_show_window" || cmd === "logs_hide_window") return Promise.resolve(null);
    return Promise.resolve(null);
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
  render(<AccountPage />);
});

describe("AccountPage", () => {
  it("renders login form fields and action buttons", () => {
    expect(screen.getByLabelText("电子邮箱地址")).toBeTruthy();
    expect(screen.getByLabelText("密码")).toBeTruthy();
    expect(screen.getByLabelText("双重验证码")).toBeTruthy();
    expect(screen.getByLabelText("加密密钥")).toBeTruthy();
    expect(screen.getByRole("button", { name: "登录" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "查询登录状态" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "日志" })).toBeTruthy();
  });

  it("warns and skips login when required fields are empty", () => {
    fireEvent.click(screen.getByRole("button", { name: "登录" }));
    expect(toastMock.warning).toHaveBeenCalledWith("邮箱、密码和加密密钥不能为空");
    expect(invokeMock).not.toHaveBeenCalledWith("auth_login", expect.anything());
  });

  it("opens the standalone log window from the log button", () => {
    fireEvent.click(screen.getByRole("button", { name: "日志" }));
    expect(invokeMock).toHaveBeenCalledWith("logs_show_window");
  });

  it("opens the apple account site via opener", () => {
    fireEvent.click(screen.getByRole("button", { name: /打开苹果账户官网/ }));
    expect(openUrlMock).toHaveBeenCalledWith("https://account.apple.com");
  });
});
