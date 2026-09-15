import { beforeEach, describe, expect, it, vi } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { fireEvent, render, screen } from "@testing-library/react";
import { LogWindow } from "./log-window";
import { useLogs } from "@/stores/logs";
import type { LogEntry } from "@/lib/types";

const invokeMock = vi.fn<(cmd: string) => Promise<unknown>>();
const snapshotMock = vi.fn(async () => [] as LogEntry[]);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...(args as [string])),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock("@/lib/api", () => ({
  api: {
    logsSnapshot: () => snapshotMock(),
    logsClear: () => Promise.resolve(),
  },
}));

beforeEach(async () => {
  invokeMock.mockClear();
  snapshotMock.mockClear();
  snapshotMock.mockResolvedValue([]);
  useLogs.setState({ entries: [], listening: false });
  if (!i18next.isInitialized) {
    await i18next.use(initReactI18next).init({
      lng: "zh",
      resources: {
        zh: {
          translation: {
            "LogViewerWindow/TitleBar.Title": "日志",
            "LogViewerWindow/CopyButton.Content": "复制日志",
            "LogViewerWindow/ClearButton.Content": "清空日志",
            "LogViewerWindow/CloseButton.Content": "关闭",
            "Common/LogDialog/CopyEmptyLog": "日志为空",
            "Common/LogDialog/CopiedToClipboard": "已复制",
            "PurchaseSync/Log/Start": "开始同步已购买列表：{{0}}",
          },
        },
      },
      keySeparator: false,
      nsSeparator: false,
      interpolation: { escapeValue: false },
    });
  }
});

function entry(text: string): LogEntry {
  return { timestamp: "2026-09-15 10:00:00", level: "info", message: { type: "raw", text } };
}

describe("LogWindow", () => {
  it("renders store entries with timestamp and level tags", () => {
    useLogs.setState({ entries: [entry("第一条"), entry("第二条")] });
    render(<LogWindow />);
    expect(screen.getByText(/第一条/)).toBeTruthy();
    expect(screen.getByText(/第二条/)).toBeTruthy();
  });

  it("empty buffer renders the action bar only", () => {
    useLogs.setState({ entries: [] });
    render(<LogWindow />);
    expect(screen.getByText("清空日志")).toBeTruthy();
    expect(screen.queryByText(/INFO/)).toBeNull();
  });

  it("clear empties the store and notifies the backend", () => {
    useLogs.setState({ entries: [entry("将被打掉")] });
    render(<LogWindow />);
    fireEvent.click(screen.getByText("清空日志"));
    expect(useLogs.getState().entries.length).toBe(0);
    expect(screen.queryByText(/将被打掉/)).toBeNull();
  });

  it("close hides the standalone window via backend command", () => {
    render(<LogWindow />);
    fireEvent.click(screen.getByText("关闭"));
    expect(invokeMock).toHaveBeenCalledWith("logs_hide_window");
  });

  it("sets the document title to the localized log window name", () => {
    render(<LogWindow />);
    expect(document.title).toBe("日志");
  });
});
