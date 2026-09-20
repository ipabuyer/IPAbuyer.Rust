import { describe, expect, it } from "vitest";
import {
  appStoreUrl,
  collectDevelopers,
  deriveUnpurchasedStatus,
  displayPrice,
  displayStatus,
  filterResults,
  isFreePrice,
  logLevelTag,
  queueStatusKey,
} from "./status";
import type { SearchResultItem } from "./types";

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
    expect(displayStatus(item("blocked"))).toBe("blocked");
    expect(displayStatus(item("not_purchased"))).toBe("not_purchased");
  });

  it("treats unknown values as not purchased", () => {
    expect(displayStatus(item(""))).toBe("not_purchased");
    expect(displayStatus(item("whatever"))).toBe("not_purchased");
  });
});

describe("deriveUnpurchasedStatus", () => {
  it("mirrors core resolve_unpurchased_status", () => {
    expect(deriveUnpurchasedStatus("0")).toBe("not_purchased");
    expect(deriveUnpurchasedStatus("free")).toBe("not_purchased");
    // 价格未知（空值）与后端一致归为未购买
    expect(deriveUnpurchasedStatus("")).toBe("not_purchased");
    expect(deriveUnpurchasedStatus(null)).toBe("not_purchased");
    expect(deriveUnpurchasedStatus("6.00")).toBe("blocked");
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

const item = (overrides: Partial<SearchResultItem>): SearchResultItem => ({
  bundleId: overrides.bundleId ?? "com.x",
  id: "1",
  name: overrides.name ?? "X",
  developer: overrides.developer ?? null,
  artworkUrl: null,
  price: "free",
  version: "1.0",
  platform: "ios",
  purchased: "not_purchased",
  ...overrides,
});

describe("collectDevelopers", () => {
  it("trims, dedupes case-insensitively, and keeps order", () => {
    expect(
      collectDevelopers([
        item({ developer: " Tencent " }),
        item({ developer: "tencent" }),
        item({ developer: " NetEase " }),
        item({ developer: null }),
      ]),
    ).toEqual(["Tencent", "NetEase"]);
  });

  it("returns empty for results without developers", () => {
    expect(collectDevelopers([item({ developer: null })])).toEqual([]);
    expect(collectDevelopers([])).toEqual([]);
  });
});

describe("filterResults", () => {
  const results = [
    item({ bundleId: "com.a", purchased: "purchased", developer: "Tencent" }),
    item({ bundleId: "com.b", purchased: "not_purchased", developer: "tencent" }),
    item({ bundleId: "com.c", purchased: "blocked", developer: "NetEase" }),
  ];

  it("filter all keeps everything", () => {
    expect(filterResults(results, "all", "all").length).toBe(3);
  });

  it("purchased / not_purchased partition by status", () => {
    expect(filterResults(results, "purchased", "all").map((r) => r.bundleId)).toEqual(["com.a"]);
    expect(filterResults(results, "not_purchased", "all").map((r) => r.bundleId)).toEqual([
      "com.b",
      "com.c",
    ]);
  });

  it("developer filter is case-insensitive", () => {
    expect(filterResults(results, "all", "TENCENT").map((r) => r.bundleId)).toEqual([
      "com.a",
      "com.b",
    ]);
  });

  it("combined filter intersects", () => {
    expect(filterResults(results, "purchased", "tencent").map((r) => r.bundleId)).toEqual([
      "com.a",
    ]);
    expect(filterResults(results, "not_purchased", "NetEase").map((r) => r.bundleId)).toEqual([
      "com.c",
    ]);
  });

  it("unknown developer yields empty list", () => {
    expect(filterResults(results, "all", "missing")).toEqual([]);
  });

  it("platform filter keeps only matching platform entries", () => {
    const withMac = [...results, item({ bundleId: "com.mac", platform: "macos" })];

    expect(filterResults(withMac, "all", "all", "all").length).toBe(4);
    expect(
      filterResults(withMac, "all", "all", "ios").map((r) => r.bundleId),
    ).toEqual(["com.a", "com.b", "com.c"]);
    expect(filterResults(withMac, "all", "all", "macos").map((r) => r.bundleId)).toEqual([
      "com.mac",
    ]);
  });

  it("platform filter intersects with status and developer", () => {
    const withMac = [
      ...results,
      item({ bundleId: "com.mac", platform: "macos", purchased: "purchased" }),
    ];

    expect(
      filterResults(withMac, "purchased", "all", "macos").map((r) => r.bundleId),
    ).toEqual(["com.mac"]);
    expect(
      filterResults(withMac, "purchased", "NetEase", "macos"),
    ).toEqual([]);
  });
});
