// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { TopicTags } from "./TopicTags";
import { TaskRow } from "./FrameApp";
import { designFixture } from "./designFixture";

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("displays Douyin topics from saved metadata and older hashtag titles", () => {
  const task = {...designFixture().tasks[0], platform:"douyin" as const, title:"海边日落 #旅行 #风景", topics:["旅行", "摄影"]};
  render(<TaskRow task={task} selected={false} busy={false} onSelect={vi.fn()} onAction={vi.fn()}/>);
  expect(screen.getByText("海边日落")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", {name:"查看全部 3 个话题"}));
  const dialog = screen.getByRole("dialog");
  ["旅行", "摄影", "风景"].forEach(topic => expect(within(dialog).getByText(`#${topic}`)).toBeTruthy());
});

it("recalculates overflow when space changes and opens every full topic", () => {
  let width = 240;
  let resized = () => {};
  vi.stubGlobal("ResizeObserver", class { constructor(callback: () => void) { resized = callback; } observe() {} disconnect() {} });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function(this: HTMLElement) {
    return {width: this.classList.contains("topic-tags-inline") ? width : 90} as DOMRect;
  });
  const topics = Array.from({length:10}, (_,i)=>`话题${i}`);
  render(<TopicTags topics={topics}/>);
  const trigger = screen.getByRole("button", {name:"查看全部 10 个话题"});
  expect(trigger.textContent).toBe("+8");
  width = 140;
  act(() => resized());
  expect(trigger.textContent).toBe("+9");
  fireEvent.click(trigger);
  const dialog = screen.getByRole("dialog", {name:"全部话题 · 10"});
  topics.forEach(topic => expect(within(dialog).getByText(`#${topic}`)).toBeTruthy());
  fireEvent.keyDown(document, {key:"Escape"});
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("opens tags independently of playback and never nests buttons", () => {
  const onOpen = vi.fn();
  const task = {...designFixture().tasks[0], platform:"xiaohongshu" as const, title:"作品", status:"completed" as const, topics:["摄影", "旅行", "风景", "日常", "记录"]};
  const {container} = render(<TaskRow task={task} selected={false} busy={false} onSelect={vi.fn()} onOpen={onOpen} onAction={vi.fn()}/>);
  expect(container.querySelector("button button")).toBeNull();
  fireEvent.click(screen.getByRole("button", {name:"查看全部 5 个话题"}));
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(onOpen).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name:"关闭"}));
  fireEvent.click(screen.getByRole("button", {name:"播放 作品"}));
  expect(onOpen).toHaveBeenCalledOnce();
});
