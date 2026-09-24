// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { TaskActions } from "./TaskActions";
import { designFixture } from "./designFixture";
afterEach(cleanup);
it("exposes completed file actions and keeps extra actions in a keyboard menu", () => {
  const action = vi.fn();
  render(<TaskActions task={designFixture().tasks[3]} onAction={action} onSelect={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", {name: "在文件夹中显示"}));
  expect(action).toHaveBeenLastCalledWith("reveal");
  fireEvent.click(screen.getByRole("button", {name: "移除记录"}));
  expect(action).toHaveBeenLastCalledWith("remove");
  const trigger = screen.getByRole("button", {name: "更多操作"});
  fireEvent.click(trigger);
  expect(document.activeElement).toBe(screen.getByRole("menuitem", {name: "查看详情"}));
  fireEvent.keyDown(document.activeElement!, {key: "End"});
  expect(document.activeElement).toBe(screen.getByRole("menuitem", {name: "打开文件"}));
  fireEvent.keyDown(document.activeElement!, {key: "Escape"});
  expect(screen.queryByRole("menu")).toBeNull();
  expect(document.activeElement).toBe(trigger);
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("menuitem", {name: "复制链接"}));
  expect(action).toHaveBeenLastCalledWith("copy");
  expect(screen.queryByRole("menu")).toBeNull();
});
it("prevents removing an active transfer and offers cancellation", () => {
  const action = vi.fn();
  render(<TaskActions task={designFixture().tasks[0]} onAction={action} onSelect={vi.fn()} />);
  expect(screen.queryByRole("button", {name: "移除记录"})).toBeNull();
  fireEvent.click(screen.getByRole("button", {name: "取消下载"}));
  expect(action).toHaveBeenCalledWith("cancel");
  fireEvent.click(screen.getByRole("button", {name: "更多操作"}));
  expect(screen.queryByRole("menuitem", {name: "打开文件"})).toBeNull();
  fireEvent.click(screen.getByRole("menuitem", {name: "取消下载"}));
  expect(action).toHaveBeenCalledWith("cancel");
});
it("keeps action buttons mounted and disabled while a request is pending", () => {
  const action = vi.fn();
  const task = designFixture().tasks[0];
  const {rerender} = render(<TaskActions task={task} onAction={action} onSelect={vi.fn()} />);
  const pause = screen.getByRole("button", {name: "暂停下载"});
  rerender(<TaskActions task={task} busy onAction={action} onSelect={vi.fn()} />);
  expect(screen.getByRole("button", {name: "暂停下载"})).toBe(pause);
  expect((pause as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", {name: "取消下载"}));
  expect(action).not.toHaveBeenCalled();
});
