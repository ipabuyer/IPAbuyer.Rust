import { beforeEach, describe, expect, it } from "vitest";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { renderHook } from "@testing-library/react";
import { useRenderMessage } from "./messages";

beforeEach(async () => {
  if (!i18next.isInitialized) {
    await i18next.use(initReactI18next).init({
      lng: "zh",
      resources: {
        zh: {
          translation: {
            "Test/Plain": "无参数消息",
            "Test/Interpolated": "开始购买：{{0}}",
          },
        },
      },
      keySeparator: false,
      nsSeparator: false,
      interpolation: { escapeValue: false },
    });
  }
});

// React 19 下 renderHook 对返回函数的 hook，result.current 不可靠；
// 用外部 holder 在渲染回调内接住返回值。
function setupRender() {
  const holder: { render?: ReturnType<typeof useRenderMessage> } = {};
  renderHook(() => {
    holder.render = useRenderMessage();
  });
  expect(typeof holder.render).toBe("function");
  return holder.render!;
}

describe("useRenderMessage", () => {
  it("returns empty string for nullish messages", () => {
    const render = setupRender();
    expect(render(null)).toBe("");
    expect(render(undefined)).toBe("");
  });

  it("passes raw messages through unchanged", () => {
    const render = setupRender();
    expect(render({ type: "raw", text: "原文输出" })).toBe("原文输出");
  });

  it("renders keyed messages with positional interpolation", () => {
    const render = setupRender();
    expect(
      render({ type: "key", key: "Test/Interpolated", args: ["com.example"] }),
    ).toBe("开始购买：com.example");
  });

  it("renders plain keys and falls back to the key name when missing", () => {
    const render = setupRender();
    expect(render({ type: "key", key: "Test/Plain", args: [] })).toBe("无参数消息");
    expect(render({ type: "key", key: "Missing/Key", args: [] })).toBe("Missing/Key");
  });
});
