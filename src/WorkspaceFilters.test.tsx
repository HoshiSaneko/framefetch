// @vitest-environment jsdom
import {cleanup, fireEvent, render, screen, within} from "@testing-library/react";
import {afterEach, expect, it, vi} from "vitest";
import FrameApp from "./FrameApp";
import {api} from "./bridge";
import {designFixture} from "./designFixture";

afterEach(() => {cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals();});
it("makes paused and failed tasks discoverable and combines search with state filters", async () => {
  vi.stubGlobal("ResizeObserver", class {observe() {} unobserve() {} disconnect() {}});
  vi.spyOn(api, "snapshot").mockResolvedValue(designFixture());
  vi.spyOn(api, "douyinStatus").mockResolvedValue({sessionPresent:false});
  render(<FrameApp />);
  const filters = within(screen.getByRole("group", {name:"任务分类"}));
  fireEvent.click(await filters.findByRole("button", {name:/已暂停/}));
  expect(screen.getAllByRole("article")).toHaveLength(1);
  expect(screen.getByRole("article").getAttribute("aria-label")).toContain("已暂停");
  fireEvent.click(filters.getByRole("button", {name:/需处理/}));
  expect(screen.getAllByRole("article")).toHaveLength(1);
  expect(screen.getByText("连接中断，请重试（演示状态）")).toBeTruthy();
  fireEvent.change(screen.getByRole("textbox", {name:"搜索下载记录"}), {target:{value:"不存在的文件"}});
  expect(screen.queryByRole("article")).toBeNull();
  fireEvent.click(screen.getByRole("button", {name:"清除搜索"}));
  expect(screen.getAllByRole("article")).toHaveLength(1);
  expect(filters.getByRole("button", {name:/需处理/}).getAttribute("aria-pressed")).toBe("true");
});
