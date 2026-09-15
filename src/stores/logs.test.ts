import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LogEntry } from "@/lib/types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));

const snapshotMock = vi.fn(async () => [] as LogEntry[]);
vi.mock("@/lib/api", () => ({
  api: {
    logsSnapshot: (...args: unknown[]) => snapshotMock(...(args as [])),
    logsClear: vi.fn(async () => {}),
  },
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useLogs } from "./logs";

const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

function logs(count: number): LogEntry[] {
  return Array.from({ length: count }, (_, i) => ({
    timestamp: `t${i}`,
    level: "info" as const,
    message: { type: "raw" as const, text: `m${i}` },
  }));
}

function appendedHandler() {
  return listenMock.mock.calls[0][1] as unknown as (event: { payload: LogEntry[] }) => void;
}

beforeEach(() => {
  invokeMock.mockClear();
  listenMock.mockClear();
  snapshotMock.mockClear();
  snapshotMock.mockResolvedValue([]);
  useLogs.setState({ entries: [], listening: false });
});

describe("useLogs", () => {
  it("init backfills entries from snapshot and registers the listener once", async () => {
    snapshotMock.mockResolvedValue(logs(2));
    await useLogs.getState().init();
    await useLogs.getState().init();

    expect(snapshotMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledWith("log-append", expect.any(Function));
    expect(useLogs.getState().entries.length).toBe(2);
  });

  it("appends entries delivered by log-append events", async () => {
    await useLogs.getState().init();
    const handler = appendedHandler();

    handler({ payload: logs(3) });
    expect(useLogs.getState().entries.map((e) => e.timestamp)).toEqual(["t0", "t1", "t2"]);

    handler({ payload: logs(1) });
    expect(useLogs.getState().entries.length).toBe(4);
  });

  it("caps retained entries at 1000", async () => {
    await useLogs.getState().init();
    appendedHandler()({ payload: logs(1200) });
    expect(useLogs.getState().entries.length).toBe(1000);
  });

  it("setOpen toggles the standalone log window commands", async () => {
    await useLogs.getState().setOpen(true);
    expect(invokeMock).toHaveBeenCalledWith("logs_show_window");

    await useLogs.getState().setOpen(false);
    expect(invokeMock).toHaveBeenCalledWith("logs_hide_window");
  });

  it("clear empties local entries and invokes backend clear", async () => {
    await useLogs.getState().init();
    appendedHandler()({ payload: logs(2) });
    useLogs.getState().clear();
    expect(useLogs.getState().entries.length).toBe(0);
  });
});
