// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { TaskRow } from "./FrameApp";
import { designFixture } from "./designFixture";

afterEach(cleanup);
it.each(["xiaohongshu", "bilibili"] as const)("shows the author with the %s platform logo", (platform) => {
  const task = {...designFixture().tasks[0], platform, source:"作品作者"};
  const {container} = render(<TaskRow task={task} selected={false} busy={false} onSelect={vi.fn()} onAction={vi.fn()}/>);
  const chip = container.querySelector(`.task-platform-chip.${platform}`)!;
  expect(chip.textContent).toBe("作品作者");
  expect(chip.getAttribute("title")).toBe("作品作者");
  expect(chip.querySelector("img")).toBeTruthy();
});
it("opens failure details directly without opening playback or the menu", () => {
  const details = vi.fn();
  const open = vi.fn();
  render(<TaskRow task={{...designFixture().tasks[0], status: "failed"}} selected={false} busy={false} onSelect={details} onOpen={open} onAction={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", {name: "查看详情"}));
  expect(details).toHaveBeenCalledOnce();
  expect(open).not.toHaveBeenCalled();
  expect(screen.queryByRole("menu")).toBeNull();
});
it("opens playback when clicking a file without a video extension", () => {
  const open = vi.fn();
  const details = vi.fn();
  const task = {...designFixture().tasks[0], fileName: "无扩展名", title: "视频", status: "completed" as const};
  render(<TaskRow task={task} selected={false} busy={false} onSelect={details} onOpen={open} onAction={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", {name: "播放 视频"}));
  expect(open).toHaveBeenCalledOnce();
  expect(details).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name: "更多操作"}));
  fireEvent.click(screen.getByRole("menuitem", {name: "查看详情"}));
  expect(details).toHaveBeenCalledOnce();
});
it("shows resolving without download progress and allows pausing", () => {
  const action = vi.fn();
  render(<TaskRow task={{...designFixture().tasks[0], status: "resolving", totalBytes: 0, speed: 0}} selected={false} busy={false} onSelect={vi.fn()} onAction={action} />);
  expect(screen.getByText("解析中")).toBeTruthy();
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(screen.queryByText("0 B/s")).toBeNull();
  fireEvent.click(screen.getByRole("button", {name: "暂停下载"}));
  expect(action).toHaveBeenCalledWith("pause");
});
it("shows transfer progress only after resolution finishes", () => {
  render(<TaskRow task={designFixture().tasks[0]} selected={false} busy={false} onSelect={vi.fn()} onAction={vi.fn()} />);
  expect(screen.getByText("下载中")).toBeTruthy();
  expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("68");
});
it("does not announce a fabricated percentage for an unknown file size", () => {
  render(<TaskRow task={{...designFixture().tasks[0], totalBytes: 0}} selected={false} busy={false} onSelect={vi.fn()} onAction={vi.fn()} />);
  expect(screen.getByRole("progressbar").hasAttribute("aria-valuenow")).toBe(false);
  expect(screen.getByRole("progressbar").getAttribute("aria-valuetext")).toBe("文件大小未知");
});
it("uses a quiet completed status instead of an active progress indicator", () => {
  render(<TaskRow task={designFixture().tasks[3]} selected={false} busy={false} onSelect={vi.fn()} onAction={vi.fn()} />);
  expect(screen.getByText("已完成")).toBeTruthy();
  expect(screen.queryByRole("progressbar")).toBeNull();
});
it("opens details for an unfinished file and removes the stale placeholder state", () => {
  const open = vi.fn();
  render(<TaskRow task={{...designFixture().tasks[0], status: "resolving", fileName: "待解析", title: "消息 48200"}} selected={false} busy={false} onOpen={open} onSelect={vi.fn()} onAction={vi.fn()} />);
  expect(screen.queryByText("待解析")).toBeNull();
  fireEvent.click(screen.getByRole("button", {name: "查看 消息 48200 的详情"}));
  expect(open).toHaveBeenCalledOnce();
});

it("renders a Bilibili task cover without sending the desktop referrer", () => {
  const task = {...designFixture().tasks[3], platform:"bilibili" as const, thumbnail:"https://i0.hdslb.com/bfs/archive/cover.jpg"};
  const {container,rerender}=render(<TaskRow task={task} selected={false} busy={false} onSelect={vi.fn()} onAction={vi.fn()}/>);
  const image=container.querySelector('.file-thumbnail img') as HTMLImageElement;
  expect(image.src).toBe(task.thumbnail);
  expect(image.getAttribute('referrerpolicy')).toBe('no-referrer');
  fireEvent.error(image);
  expect(container.querySelector('.file-thumbnail')).toBeNull();
  rerender(<TaskRow task={{...task,thumbnail:"https://i1.hdslb.com/bfs/archive/new.jpg"}} selected={false} busy={false} onSelect={vi.fn()} onAction={vi.fn()}/>);
  expect(container.querySelector('.file-thumbnail img')?.getAttribute('src')).toContain('/new.jpg');
});
