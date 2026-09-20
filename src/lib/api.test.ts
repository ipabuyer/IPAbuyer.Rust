import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));

import { invoke } from "@tauri-apps/api/core";
import { api } from "./api";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockClear();
});

describe("api 封装", () => {
  it("设置命令透传命令名与参数", async () => {
    await api.getSettings();
    expect(invokeMock).toHaveBeenCalledWith("settings_get");

    await api.setCountryCode("jp");
    expect(invokeMock).toHaveBeenCalledWith("settings_set_country_code", { code: "jp" });

    await api.setDetailedLog(true);
    expect(invokeMock).toHaveBeenCalledWith("settings_set_detailed_log", { enabled: true });

    await api.setDisplayLanguage("zh-Hans");
    expect(invokeMock).toHaveBeenCalledWith("settings_set_display_language", {
      language: "zh-Hans",
    });

    await api.setPassphraseRotation(false);
    expect(invokeMock).toHaveBeenCalledWith("settings_set_passphrase_rotation", {
      enabled: false,
    });
  });

  it("login：空密钥归一为 null，非空 trim 后透传", async () => {
    await api.login(" user@icloud.com ", "pw", "   ");
    expect(invokeMock).toHaveBeenCalledWith("auth_login", {
      account: " user@icloud.com ",
      password: "pw",
      passphrase: null,
    });

    await api.login("a", "b", "  key  ");
    expect(invokeMock).toHaveBeenCalledWith("auth_login", {
      account: "a",
      password: "b",
      passphrase: "key",
    });
  });

  it("verifyAuthCode 同样归一空密钥", async () => {
    await api.verifyAuthCode("a", "b", "123456", "");
    expect(invokeMock).toHaveBeenCalledWith("auth_verify_code", {
      account: "a",
      password: "b",
      authCode: "123456",
      passphrase: null,
    });
  });

  it("购买 / 标记命令透传平台维度", async () => {
    await api.purchase("com.a", "free", "not_purchased", "ios");
    expect(invokeMock).toHaveBeenCalledWith("purchase", {
      bundleId: "com.a",
      price: "free",
      purchased: "not_purchased",
      platform: "ios",
    });

    await api.mark("com.a", "purchased", "macos");
    expect(invokeMock).toHaveBeenCalledWith("purchases_mark", {
      bundleId: "com.a",
      status: "purchased",
      platform: "macos",
    });

    await api.unmark("com.a", "ipad");
    expect(invokeMock).toHaveBeenCalledWith("purchases_unmark", {
      bundleId: "com.a",
      platform: "ipad",
    });
  });

  it("queueAdd 透传完整队列条目", async () => {
    const item = {
      bundleId: "com.a",
      platform: "ios" as const,
      appId: "1",
      name: "A",
      developer: "Dev",
      version: "1.0",
      price: "free",
      artworkUrl: null,
    };
    await api.queueAdd(item);
    expect(invokeMock).toHaveBeenCalledWith("queue_add", item);
  });

  it("队列与同步命令使用约定命令名", async () => {
    await api.queueStart();
    await api.queueCancel();
    await api.syncStart();
    await api.syncCancel();
    await api.syncLastTime();

    expect(invokeMock).toHaveBeenCalledWith("queue_start");
    expect(invokeMock).toHaveBeenCalledWith("queue_cancel_current");
    expect(invokeMock).toHaveBeenCalledWith("sync_start");
    expect(invokeMock).toHaveBeenCalledWith("sync_cancel");
    expect(invokeMock).toHaveBeenCalledWith("sync_last_time");
  });
});
