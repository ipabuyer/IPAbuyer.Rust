import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Button } from "@/components/ui/button";
import { SettingsCard } from "./settings-card";
import { Tag } from "lucide-react";

describe("SettingsCard", () => {
  it("renders header, description and children actions", () => {
    render(
      <SettingsCard header="内置 ipatool" description="内置正式版 ipatool">
        <Button>导出</Button>
      </SettingsCard>,
    );
    expect(screen.getByText("内置 ipatool")).toBeTruthy();
    expect(screen.getByText("内置正式版 ipatool")).toBeTruthy();
    expect(screen.getByRole("button", { name: "导出" })).toBeTruthy();
  });

  it("renders the leading icon when provided", () => {
    render(<SettingsCard icon={Tag} header="版本要求" />);
    expect(document.querySelector("svg.lucide-tag")).toBeTruthy();
  });

  it("omits description when absent", () => {
    const { container } = render(<SettingsCard header="软件版本" />);
    expect(screen.getByText("软件版本")).toBeTruthy();
    expect(container.querySelector(".text-xs")).toBeNull();
  });
});
