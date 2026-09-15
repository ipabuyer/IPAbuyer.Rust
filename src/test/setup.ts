import { beforeEach, vi } from "vitest";

// jsdom 缺少的浏览器 API（shadcn Sidebar 的 use-mobile / ResizeObserver 依赖）
beforeEach(() => {
  if (!window.matchMedia) {
    Object.defineProperty(window, "matchMedia", {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
  }
  if (!("ResizeObserver" in globalThis)) {
    Object.defineProperty(globalThis, "ResizeObserver", {
      writable: true,
      value: class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    });
  }
  // jsdom 未实现 scrollIntoView（日志窗口自动滚动依赖）
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = vi.fn();
  }
  // radix 菜单/下拉的指针事件依赖
  if (!window.PointerEvent) {
    class PointerEvent extends MouseEvent {
      pointerId: number;
      pointerType: string;
      constructor(type: string, params: Record<string, unknown> = {}) {
        super(type, params as MouseEventInit);
        this.pointerId = (params.pointerId as number) ?? 1;
        this.pointerType = (params.pointerType as string) ?? "mouse";
      }
    }
    (window as unknown as { PointerEvent: unknown }).PointerEvent = PointerEvent;
  }
  Element.prototype.hasPointerCapture ??= vi.fn(() => false);
  Element.prototype.setPointerCapture ??= vi.fn();
  Element.prototype.releasePointerCapture ??= vi.fn();
});
