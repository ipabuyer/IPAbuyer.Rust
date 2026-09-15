import { describe, expect, it } from "vitest";
import {
  appStoreUrl,
  displayPrice,
  displayStatus,
  isFreePrice,
  logLevelTag,
  queueStatusKey,
} from "./status";

describe("isFreePrice", () => {
  it("accepts the literal free in any case", () => {
    expect(isFreePrice("free")).toBe(true);
    expect(isFreePrice("Free")).toBe(true);
  });

  it("accepts zero and negative numbers", () => {
    expect(isFreePrice("0")).toBe(true);
    expect(isFreePrice("0.00")).toBe(true);
    expect(isFreePrice("-1")).toBe(true);
  });

  it("rejects paid and unparseable values", () => {
    expect(isFreePrice("6.00")).toBe(false);
    expect(isFreePrice("9.99")).toBe(false);
    expect(isFreePrice("¥6")).toBe(false);
    expect(isFreePrice("")).toBe(false);
    expect(isFreePrice(null)).toBe(false);
    expect(isFreePrice(undefined)).toBe(false);
  });
});

describe("displayPrice", () => {
  it("normalizes free prices", () => {
    expect(displayPrice("0")).toBe("free");
    expect(displayPrice("Free")).toBe("free");
    expect(displayPrice("6.00")).toBe("6.00");
  });
});

describe("displayStatus", () => {
  const item = (purchased: string) => ({ purchased }) as Parameters<typeof displayStatus>[0];

  it("maps known statuses", () => {
    expect(displayStatus(item("purchased"))).toBe("purchased");
    expect(displayStatus(item("purchase_blocked"))).toBe("purchase_blocked");
    expect(displayStatus(item("not_purchased"))).toBe("not_purchased");
  });

  it("treats unknown values as not purchased", () => {
    expect(displayStatus(item(""))).toBe("not_purchased");
    expect(displayStatus(item("whatever"))).toBe("not_purchased");
  });
});

describe("queueStatusKey", () => {
  it("builds resw-style keys", () => {
    expect(queueStatusKey("Downloading")).toBe("DownloadQueue/Status/Downloading");
  });
});

describe("logLevelTag", () => {
  it("maps levels to tags with INFO fallback", () => {
    expect(logLevelTag("tip")).toBe("TIP");
    expect(logLevelTag("success")).toBe("SUCCESS");
    expect(logLevelTag("error")).toBe("ERROR");
    expect(logLevelTag("ipatool")).toBe("ipatool");
    expect(logLevelTag("info")).toBe("INFO");
    expect(logLevelTag("anything")).toBe("INFO");
  });
});

describe("appStoreUrl", () => {
  it("builds country scoped app urls", () => {
    expect(appStoreUrl("cn", "414478124")).toBe(
      "https://apps.apple.com/cn/app/id414478124",
    );
  });
});
