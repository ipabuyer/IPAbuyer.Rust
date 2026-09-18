import { beforeEach, describe, expect, it, vi } from "vitest";
import type { QueueItem } from "@/lib/types";

type StatusHandler = (event: { payload: { running: boolean; items: QueueItem[] } }) => void;
const listenMock = vi.fn<(event: string, handler: (e: never) => void) => Promise<() => void>>(
  async () => () => {},
);
const queueStatusMock = vi.fn(async () => ({ running: false, items: [] as QueueItem[] }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...(args as [string, (e: never) => void])),
}));
vi.mock("@/lib/api", () => ({
  api: {
    queueStatus: () => queueStatusMock(),
  },
}));

import { useQueue } from "./queue";

const items: QueueItem[] = [
  {
    bundleId: "com.a",
    platform: "ios",
    appId: "1",
    name: "A",
    developer: "dev",
    version: "1.0",
    price: "free",
    artworkUrl: "",
    status: "Downloading",
    lastMessage: "",
  },
];

beforeEach(() => {
  listenMock.mockClear();
  queueStatusMock.mockReset();
  queueStatusMock.mockResolvedValue({ running: false, items: [] });
  useQueue.setState({ running: false, items: [], listening: false });
});

describe("useQueue", () => {
  it("init registers status and finished listeners then refreshes once", async () => {
    queueStatusMock.mockResolvedValue({ running: true, items });
    await useQueue.getState().init();

    expect(listenMock).toHaveBeenCalledTimes(2);
    expect(listenMock).toHaveBeenCalledWith("queue-status", expect.any(Function));
    expect(listenMock).toHaveBeenCalledWith("queue-finished", expect.any(Function));
    expect(useQueue.getState().running).toBe(true);
    expect(useQueue.getState().items).toEqual(items);
  });

  it("init is idempotent via the listening guard", async () => {
    await useQueue.getState().init();
    await useQueue.getState().init();
    expect(listenMock).toHaveBeenCalledTimes(2);
  });

  it("queue-status events update running state and items", async () => {
    await useQueue.getState().init();
    const handler = listenMock.mock.calls[0][1] as StatusHandler;

    handler({ payload: { running: false, items: [] } });
    expect(useQueue.getState().running).toBe(false);
    expect(useQueue.getState().items).toEqual([]);
  });

  it("queue-finished events trigger a refresh", async () => {
    await useQueue.getState().init();
    queueStatusMock.mockClear();

    const finishedHandler = listenMock.mock.calls[1][1] as unknown as () => void;
    finishedHandler();
    await vi.waitFor(() => expect(queueStatusMock).toHaveBeenCalled());
  });

  it("refresh tolerates backend failures", async () => {
    queueStatusMock.mockRejectedValue(new Error("backend gone"));
    await useQueue.getState().refresh();
    expect(useQueue.getState().running).toBe(false);
    expect(useQueue.getState().items).toEqual([]);
  });
});
