import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import zhHans from "@/locales/zh-Hans.json";

const { windowMock, searchMock } = vi.hoisted(() => ({
  windowMock: { minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn() },
  searchMock: vi.fn(async (..._args: unknown[]) => []),
}));

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => windowMock }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock("@/lib/api", () => ({
  api: {
    search: (...args: unknown[]) => searchMock(...(args as [string])),
  },
}));

import { SidebarProvider } from "@/components/ui/sidebar";
import { TitleBar } from "./title-bar";
import { useSearch } from "@/stores/search";
import { useSession } from "@/stores/session";

function windowButton(icon: string): HTMLButtonElement {
  const svg = document.querySelector(`svg.lucide-${icon}`);
  expect(svg).toBeTruthy();
  return (svg as Element).closest("button") as HTMLButtonElement;
}

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
  windowMock.minimize.mockClear();
  windowMock.toggleMaximize.mockClear();
  windowMock.close.mockClear();
  searchMock.mockClear();
  useSearch.setState({ query: "", results: [], searching: false, lastSearchEmpty: false });
  useSession.getState().reset();
});

describe("TitleBar", () => {
  it("渲染应用名与窗口控制按钮", () => {
    const { container } = render(
      <SidebarProvider>
        <TitleBar active="main" />
      </SidebarProvider>,
    );

    expect(screen.getByText("IPAbuyer")).toBeTruthy();
    expect(container.querySelector("svg.lucide-minus")).toBeTruthy();
    expect(container.querySelector("svg.lucide-square")).toBeTruthy();
    expect(container.querySelector("svg.lucide-x")).toBeTruthy();
  });

  it("搜索框仅在主页显示", () => {
    const { rerender } = render(
      <SidebarProvider>
        <TitleBar active="main" />
      </SidebarProvider>,
    );
    expect(screen.getByPlaceholderText("搜索 App 名称")).toBeTruthy();

    rerender(
      <SidebarProvider>
        <TitleBar active="settings" />
      </SidebarProvider>,
    );
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("输入更新查询，回车触发搜索", async () => {
    render(
      <SidebarProvider>
        <TitleBar active="main" />
      </SidebarProvider>,
    );
    const input = screen.getByPlaceholderText("搜索 App 名称");

    fireEvent.change(input, { target: { value: "wechat" } });
    expect(useSearch.getState().query).toBe("wechat");

    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(searchMock).toHaveBeenCalledWith("wechat"));
  });

  it("已登录头像显示邮箱首字母（大写）与绿色边框", () => {
    useSession.getState().setSession(true, "user@icloud.com", false);
    const { container } = render(
      <SidebarProvider>
        <TitleBar active="main" />
      </SidebarProvider>,
    );

    expect(screen.getByText("U")).toBeTruthy();
    const avatar = container.querySelector("[data-slot=avatar]");
    expect(avatar?.className).toContain("border-green-600");
  });

  it("未登录头像显示登出图标与红色边框", () => {
    const { container } = render(
      <SidebarProvider>
        <TitleBar active="main" />
      </SidebarProvider>,
    );

    expect(container.querySelector("svg.lucide-log-out")).toBeTruthy();
    const avatar = container.querySelector("[data-slot=avatar]");
    expect(avatar?.className).toContain("border-red-500");
  });

  it("窗口按钮分别触发最小化 / 最大化还原 / 关闭", () => {
    render(
      <SidebarProvider>
        <TitleBar active="main" />
      </SidebarProvider>,
    );

    fireEvent.click(windowButton("minus"));
    fireEvent.click(windowButton("square"));
    fireEvent.click(windowButton("x"));

    expect(windowMock.minimize).toHaveBeenCalledTimes(1);
    expect(windowMock.toggleMaximize).toHaveBeenCalledTimes(1);
    expect(windowMock.close).toHaveBeenCalledTimes(1);
  });
});
