// @vitest-environment jsdom
import { useEffect } from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Modal } from "./Modal";

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  cleanup();
  document
    .querySelectorAll(".dialog-exit-copy")
    .forEach((node) => node.remove());
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("dialog interaction lifecycle", () => {
  it("keeps focus when busy changes and only permits Escape when idle", () => {
    const close = vi.fn();
    const view = (busy: boolean) => (
      <Modal title="测试弹窗" onClose={close} busy={busy}>
        <input aria-label="第一项" />
        <input aria-label="第二项" />
      </Modal>
    );
    const mounted = render(view(false));
    act(() => {
      vi.advanceTimersByTime(40);
    });
    const second = screen.getByRole("textbox", { name: "第二项" });
    second.focus();
    mounted.rerender(view(true));
    act(() => {
      vi.advanceTimersByTime(40);
    });
    expect(document.activeElement).toBe(second);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).not.toHaveBeenCalled();
    mounted.rerender(view(false));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(close).toHaveBeenCalledTimes(1);
  });
  it("blocks the background while open and restores its previous inert state", () => {
    const content = document.createElement("div");
    content.className = "app-body";
    const sidebar = document.createElement("div");
    sidebar.className = "app-sidebar";
    sidebar.setAttribute("inert", "");
    document.body.append(content, sidebar);
    const view = render(
      <Modal title="测试弹窗" onClose={() => {}}>
        内容
      </Modal>,
    );
    expect(content.hasAttribute("inert")).toBe(true);
    view.unmount();
    expect(content.hasAttribute("inert")).toBe(false);
    expect(sidebar.hasAttribute("inert")).toBe(true);
    content.remove();
    sidebar.remove();
  });
  it("unmounts live children immediately while only an inert visual copy exits", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    const animations: Array<{
      onfinish: null | (() => void);
      cancel: ReturnType<typeof vi.fn>;
    }> = [];
    Object.defineProperty(Element.prototype, "animate", {
      configurable: true,
      value: vi.fn(() => {
        const animation = { onfinish: null, cancel: vi.fn() };
        animations.push(animation);
        return animation;
      }),
    });
    const stopped = vi.fn();
    function LiveContent() {
      useEffect(() => stopped, []);
      return <button>仍在运行的操作</button>;
    }
    const view = render(
      <Modal title="测试弹窗" onClose={() => {}}>
        <LiveContent />
      </Modal>,
    );
    view.unmount();
    expect(stopped).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).toBeNull();
    const copy = document.querySelector(".dialog-exit-copy");
    expect(copy?.hasAttribute("inert")).toBe(true);
    expect(copy?.getAttribute("aria-hidden")).toBe("true");
    act(() => animations.at(-1)?.onfinish?.());
    expect(document.querySelector(".dialog-exit-copy")).toBeNull();
    delete (Element.prototype as unknown as { animate?: unknown }).animate;
  });
  it("does not retain an exit copy when reduced motion is enabled", () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({ matches: true })),
    );
    const view = render(
      <Modal title="测试弹窗" onClose={() => {}}>
        内容
      </Modal>,
    );
    view.unmount();
    expect(document.querySelector(".dialog-exit-copy")).toBeNull();
  });
});

it("Escape dismisses only the top login dialog", () => {
  const parent=vi.fn(), child=vi.fn();
  render(<><Modal title="新建下载" onClose={parent}><input/></Modal><Modal title="登录 Bilibili" onClose={child}><button>刷新</button></Modal></>);
  fireEvent.keyDown(document,{key:"Escape"});
  expect(child).toHaveBeenCalledOnce();expect(parent).not.toHaveBeenCalled();
});
