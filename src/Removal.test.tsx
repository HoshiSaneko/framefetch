// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import FrameApp from "./FrameApp";
import { api } from "./bridge";
import { designFixture } from "./designFixture";
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("requires confirmation for single and bulk removal", async () => {
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  const data = designFixture();
  data.tasks = data.tasks.filter(t => t.status === "completed");
  vi.spyOn(api, "snapshot").mockImplementation(async () => ({...data, tasks: [...data.tasks]}));
  const remove = vi.spyOn(api, "remove").mockImplementation(async id => { data.tasks = data.tasks.filter(t => t.id !== id); });
  render(<FrameApp />);
  await screen.findAllByRole("button", {name: "更多操作"});
  expect(screen.queryByRole("checkbox")).toBeNull();
  fireEvent.click(screen.getAllByRole("button", {name: "移除记录"})[0]);
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(remove).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name: "关闭"}));
  expect(remove).not.toHaveBeenCalled();
  fireEvent.click(screen.getAllByRole("button", {name: "更多操作"})[0]);
  fireEvent.click(screen.getByRole("menuitem", {name: "选择"}));
  expect((screen.getByRole("checkbox", {name: `选择 ${data.tasks[0].title}`}) as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByRole("checkbox", {name: "全选当前列表"}));
  fireEvent.click(screen.getByRole("button", {name: "移除所选"}));
  expect(screen.getByRole("dialog", {name: "移除 2 条记录？"})).toBeTruthy();
  expect(remove).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name: "仅删除记录"}));
  await waitFor(() => expect(remove).toHaveBeenCalledTimes(2));
  expect(remove.mock.calls.every(call => call[1] === false)).toBe(true);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
});

it("sends file deletion only when selected and retains failed records for retry", async () => {
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  const data = designFixture();
  data.tasks = data.tasks.filter(t => t.status === "completed").slice(0, 1);
  vi.spyOn(api, "snapshot").mockResolvedValue(data);
  const remove = vi.spyOn(api, "remove").mockRejectedValue(new Error("文件正在使用"));
  render(<FrameApp />);
  fireEvent.click((await screen.findAllByRole("button", {name: "移除记录"}))[0]);
  expect(screen.queryByRole("radio")).toBeNull();
  expect(screen.getByRole("button", {name: "仅删除记录"})).toBeTruthy();
  expect(remove).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name: "同时删除文件"}));
  await waitFor(() => expect(remove).toHaveBeenCalledWith(data.tasks[0].id, true));
  await screen.findByText("文件正在使用");
  expect(screen.getByRole("dialog")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", {name: "关闭"}));
  fireEvent.click(screen.getAllByRole("button", {name: "移除记录"})[0]);
  expect(screen.queryByRole("radio")).toBeNull();
  expect(screen.getByRole("button", {name: "仅删除记录"})).toBeTruthy();
});

