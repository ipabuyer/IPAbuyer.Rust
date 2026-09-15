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
import { useSession } from "@/stores/session";

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

  it("logged-in state locks the form and offers logout", async () => {
    useSession.getState().setSession(true, "user@icloud.com", false);
    expect(await screen.findByRole("button", { name: "退出登录" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "登录" })).toBeNull();
    // 输入区随锁定蒙版禁用（jsdom 不传播 fieldset disabled 到子控件，断言 fieldset 本身）
    const fieldset = document.querySelector("fieldset") as HTMLFieldSetElement;
    expect(fieldset.disabled).toBe(true);
    expect((screen.getByLabelText(/电子邮箱地址/) as HTMLInputElement).value).toBe(
      "user@icloud.com",
    );
  });

  it("logout resets the session and refetches the rotated passphrase", async () => {
    useSession.getState().setSession(true, "user@icloud.com", false);
    let passphraseCalls = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "auth_logout")
        return Promise.resolve({ success: true, passphraseRotated: true, message: null });
      if (cmd === "settings_get_passphrase") {
        passphraseCalls += 1;
        return Promise.resolve(`rotated-${passphraseCalls}`);
      }
      return Promise.resolve(null);
    });

    fireEvent.click(await screen.findByRole("button", { name: "退出登录" }));
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith("auth_logout"));
    await vi.waitFor(() => expect(useSession.getState().loggedIn).toBe(false));
    // 密钥轮换开启 → 退出后重新拉取新密钥（挂载时的调用走旧实现不计入）
    await vi.waitFor(() => expect(passphraseCalls).toBe(1));
    expect(toastMock.info).toHaveBeenCalled();
  });

  it("two-factor flow: login asks for the code, verify completes the session", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "auth_login")
        return Promise.resolve({
          status: "RequiresTwoFactor",
          message: {
            type: "key",
            key: "LoginPage/Status/TwoFactorPromptFallback",
            args: [],
          },
          rawPayload: null,
        });
      if (cmd === "auth_verify_code")
        return Promise.resolve({
          status: "Success",
          message: { type: "key", key: "LoginPage/Status/LoginSuccess", args: [] },
          rawPayload: null,
        });
      if (cmd === "settings_get_passphrase") return Promise.resolve("abc123");
      return Promise.resolve(null);
    });

    fireEvent.change(screen.getByLabelText("电子邮箱地址"), {
      target: { value: "user@icloud.com" },
    });
    fireEvent.change(screen.getByLabelText("密码"), { target: { value: "pwd" } });
    fireEvent.click(screen.getByRole("button", { name: "登录" }));

    // 第一阶段：auth_login 触发验证码下发，按钮切到验证态并出现帮助文案
    await screen.findByText("正在验证...");
    expect(screen.getByText(/account.apple.com 获取验证码/)).toBeTruthy();

    // 第二阶段：填验证码后确认，走 auth_verify_code
    fireEvent.change(screen.getByLabelText(/双重验证码/), { target: { value: "123456" } });
    fireEvent.click(screen.getByRole("button", { name: "正在验证..." }));
    await vi.waitFor(() => expect(toastMock.success).toHaveBeenCalledWith("登录成功"));
    expect(invokeMock).toHaveBeenCalledWith("auth_verify_code", expect.anything());
    // 会话建立，出现退出登录入口
    expect(await screen.findByRole("button", { name: "退出登录" })).toBeTruthy();
  });
});
