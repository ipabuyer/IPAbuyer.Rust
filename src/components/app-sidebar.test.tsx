import { beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen } from "@testing-library/react";
import { SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "./app-sidebar";

const openUrlMock = vi.fn<(url: string) => Promise<void>>(async () => {});
const onNavigateMock = vi.fn();

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...(args as [string])),
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));

beforeEach(async () => {
  openUrlMock.mockClear();
  onNavigateMock.mockClear();
  if (!i18next.isInitialized) {
    await i18next.use(initReactI18next).init({
      lng: "zh",
      resources: {
        zh: {
          translation: {
            "MainWindow/Nav/Main.Content": "主页",
            "MainWindow/Nav/Account.Content": "账户",
            "MainWindow/Nav/Ipatool.Content": "ipatool",
            "MainWindow/Nav/Settings.Content": "设置",
            "MainWindow/Nav/Faq.Content": "常见问题",
          },
        },
      },
      keySeparator: false,
      nsSeparator: false,
      interpolation: { escapeValue: false },
    });
  }
  render(
    <SidebarProvider>
      <AppSidebar active="main" onNavigate={onNavigateMock} />
    </SidebarProvider>,
  );
});

describe("AppSidebar", () => {
  it("renders the four in-app pages and the external FAQ entry", () => {
    for (const label of ["主页", "账户", "ipatool", "设置", "常见问题"]) {
      expect(screen.getByText(label)).toBeTruthy();
    }
  });

  it("marks only the external entry with the arrow indicator", () => {
    const faqButton = screen.getByText("常见问题").closest("button");
    expect(faqButton?.querySelector("svg.lucide-arrow-up-right")).toBeTruthy();
    for (const label of ["主页", "账户", "ipatool", "设置"]) {
      const button = screen.getByText(label).closest("button");
      expect(button?.querySelector("svg.lucide-arrow-up-right")).toBeNull();
    }
  });

  it("in-app entries route through onNavigate", () => {
    fireEvent.click(screen.getByText("设置"));
    expect(onNavigateMock).toHaveBeenCalledWith("settings");
    expect(openUrlMock).not.toHaveBeenCalled();
  });

  it("FAQ entry opens the external page", () => {
    fireEvent.click(screen.getByText("常见问题"));
    expect(openUrlMock).toHaveBeenCalledWith("https://ipa.blazesnow.com/faq.html");
    expect(onNavigateMock).not.toHaveBeenCalled();
  });

  it("marks the active page", () => {
    const mainButton = screen.getByText("主页").closest("button");
    expect(mainButton?.getAttribute("data-active")).toBe("true");
    const settingsButton = screen.getByText("设置").closest("button");
    expect(settingsButton?.getAttribute("data-active")).toBe("false");
  });
});
