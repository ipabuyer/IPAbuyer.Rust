import { beforeEach, describe, expect, it, vi } from "vitest";

const searchMock = vi.fn();
vi.mock("@/lib/api", () => ({
  api: { search: (...args: unknown[]) => searchMock(...(args as [string])) },
}));

import { useSearch } from "./search";

const result = (bundleId: string) => ({
  bundleId,
  id: "1",
  name: bundleId,
  developer: "dev",
  artworkUrl: null,
  price: "free",
  version: "1.0",
  purchased: "not_purchased",
});

beforeEach(() => {
  searchMock.mockReset();
  useSearch.setState({ query: "", results: [], searching: false, lastSearchEmpty: false });
});

describe("useSearch", () => {
  it("setQuery only updates the query", () => {
    useSearch.getState().setQuery("wechat");
    expect(useSearch.getState().query).toBe("wechat");
    expect(useSearch.getState().results).toEqual([]);
  });

  it("search ignores empty or duplicate in-flight queries", async () => {
    await useSearch.getState().search();
    expect(searchMock).not.toHaveBeenCalled();

    searchMock.mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve([result("a")]), 50)),
    );
    useSearch.getState().setQuery("x");
    const first = useSearch.getState().search();
    const second = useSearch.getState().search();
    await Promise.all([first, second]);
    expect(searchMock).toHaveBeenCalledTimes(1);
  });

  it("search trims the query and records empty results", async () => {
    searchMock.mockResolvedValue([]);
    useSearch.getState().setQuery("  wechat  ");
    await useSearch.getState().search();

    expect(searchMock).toHaveBeenCalledWith("wechat");
    expect(useSearch.getState().query).toBe("  wechat  ");
    expect(useSearch.getState().results).toEqual([]);
    expect(useSearch.getState().lastSearchEmpty).toBe(true);
    expect(useSearch.getState().searching).toBe(false);
  });

  it("search stores results and resets the empty flag", async () => {
    searchMock.mockResolvedValue([result("a"), result("b")]);
    useSearch.getState().setQuery("games");
    await useSearch.getState().search();

    expect(useSearch.getState().results.length).toBe(2);
    expect(useSearch.getState().lastSearchEmpty).toBe(false);
    expect(useSearch.getState().searching).toBe(false);
  });

  it("releases the searching flag when the backend rejects", async () => {
    searchMock.mockRejectedValue("network down");
    useSearch.getState().setQuery("x");
    await expect(useSearch.getState().search()).rejects.toBe("network down");
    expect(useSearch.getState().searching).toBe(false);
    expect(useSearch.getState().lastSearchEmpty).toBe(false);
  });
});
