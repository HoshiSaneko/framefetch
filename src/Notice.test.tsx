// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Notice } from "./Notice";
import { PlatformNotice } from "./PlatformNotice";

afterEach(() => {cleanup(); vi.useRealTimers();});

it("stacks app and platform notices outside their components and cleans up the portal", () => {
  const {container, unmount} = render(<><Notice kind="success" message="已加入队列" onClose={vi.fn()}/><PlatformNotice platform="小红书" message="读取超时" onClose={vi.fn()}/></>);
  const stack = screen.getByRole("region", {name: "通知"});
  expect(stack.contains(screen.getByRole("status"))).toBe(true);
  expect(stack.contains(screen.getByRole("alert"))).toBe(true);
  expect(container.contains(stack)).toBe(false);
  unmount();
  expect(screen.queryByRole("region", {name: "通知"})).toBeNull();
});

it("auto dismisses success but pauses while hovered or keyboard focused", () => {
  vi.useFakeTimers();
  const close = vi.fn();
  render(<Notice kind="success" message="已保存" onClose={close}/>);
  const notice = screen.getByRole("status");
  fireEvent.mouseEnter(notice);
  act(() => vi.advanceTimersByTime(10000));
  expect(close).not.toHaveBeenCalled();
  fireEvent.focus(screen.getByRole("button", {name: "关闭提示"}));
  fireEvent.mouseLeave(notice);
  act(() => vi.advanceTimersByTime(10000));
  expect(close).not.toHaveBeenCalled();
  fireEvent.blur(screen.getByRole("button", {name: "关闭提示"}), {relatedTarget: document.body});
  act(() => vi.advanceTimersByTime(6500));
  expect(close).toHaveBeenCalledOnce();
});

it("keeps errors and actionable notices until dismissed and preserves retry", () => {
  vi.useFakeTimers();
  const close = vi.fn(), retry = vi.fn();
  render(<><Notice kind="error" message="下载失败" onClose={close}/><Notice kind="info" message="连接提示" onClose={close} action={{label: "重试", onClick: retry}}/></>);
  act(() => vi.advanceTimersByTime(60000));
  expect(close).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name: "重试"}));
  expect(retry).toHaveBeenCalledOnce();
  fireEvent.click(screen.getAllByRole("button", {name: "关闭提示"})[0]);
  expect(close).toHaveBeenCalledOnce();
});
