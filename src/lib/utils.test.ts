import { describe, expect, it } from "vitest";
import { cn } from "./utils";

describe("cn", () => {
  it("joins class names and drops falsy values", () => {
    expect(cn("a", false && "b", undefined, "c")).toBe("a c");
  });

  it("dedupes conflicting tailwind utilities keeping the last", () => {
    expect(cn("px-2", "px-4")).toBe("px-4");
  });
});
