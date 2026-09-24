// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { VideoPlayer } from "./VideoPlayer";
import { api } from "./bridge";
import { designFixture } from "./designFixture";
vi.mock("@tauri-apps/api/core", () => ({isTauri: () => false, invoke: vi.fn(), convertFileSrc: (path: string) => `http://asset.localhost/${path}`}));
beforeEach(() => {
  localStorage.clear();
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
});
afterEach(() => {cleanup(); vi.restoreAllMocks();});
it("browses a work's images in one player", async () => {
  const first = {...designFixture().tasks[0],id:"one",fileName:"123-1.jpg",status:"completed" as const};
  const second = {...first,id:"two",fileName:"123-2.jpg"};
  const playback = vi.spyOn(api,"playbackPath").mockImplementation(async id => `${id}.jpg`);
  render(<VideoPlayer task={first} galleryTasks={[first,second]} onClose={vi.fn()}/>);
  await waitFor(()=>expect(screen.getByRole("img").getAttribute("src")).toContain("one.jpg"));
  fireEvent.click(screen.getByRole("button",{name:"下一张"}));
  await waitFor(()=>expect(screen.getByRole("img").getAttribute("src")).toContain("two.jpg"));
  expect(playback).toHaveBeenLastCalledWith("two");
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(screen.getByText("2 / 2")).toBeTruthy();
});
it("previews a JPEG as an image without video controls", async () => {
  vi.spyOn(api, "playbackPath").mockResolvedValue("photo-8172.jpg");
  const {container} = render(<VideoPlayer task={{...designFixture().tasks[0], fileName: "photo-8172.jpg"}} onClose={vi.fn()} />);
  const image = await screen.findByRole("img");
  expect(image.getAttribute("src")).toContain("photo-8172.jpg");
  expect(container.querySelector("video")).toBeNull();
  expect(screen.queryByRole("slider", {name: "播放进度"})).toBeNull();
});
it("previews the hovered time without seeking the main video", async () => {
  vi.spyOn(api, "playbackPath").mockResolvedValue("movie.mp4");
  const {container} = render(<VideoPlayer task={designFixture().tasks[0]} onClose={vi.fn()} />);
  const main = container.querySelector("video")!;
  await waitFor(() => expect(main.getAttribute("src")).toContain("movie.mp4"));
  Object.defineProperty(main, "duration", {value: 120});
  main.currentTime = 10;
  fireEvent.loadedMetadata(main);
  const slider = screen.getByRole("slider", {name: "播放进度"});
  vi.spyOn(slider, "getBoundingClientRect").mockReturnValue({left: 0, width: 400} as DOMRect);
  fireEvent.mouseMove(slider, {clientX: 200});
  const tooltip = screen.getByRole("tooltip", {name: "预览 1:00"});
  const preview = tooltip.querySelector("video")!;
  Object.defineProperty(preview, "readyState", {value: 1});
  fireEvent.loadedMetadata(preview);
  expect(preview.currentTime).toBe(60);
  expect(main.currentTime).toBe(10);
  fireEvent.seeked(preview);
  fireEvent.mouseLeave(slider.parentElement!);
  expect(screen.queryByRole("tooltip")).toBeNull();
});
it("loads a local video, seeks, changes volume and stops on unmount", async () => {
  vi.spyOn(api, "playbackPath").mockResolvedValue("movie.mp4");
  const {container, unmount} = render(<VideoPlayer task={designFixture().tasks[0]} onClose={vi.fn()} />);
  const video = container.querySelector("video")!;
  await waitFor(() => expect(video.getAttribute("src")).toContain("movie.mp4"));
  Object.defineProperty(video, "duration", {value: 120});
  fireEvent.loadedMetadata(video);
  fireEvent.change(screen.getByRole("slider", {name: "播放进度"}), {target: {value: "30"}});
  expect(video.currentTime).toBe(30);
  fireEvent.change(screen.getByRole("slider", {name: "音量"}), {target: {value: "0.4"}});
  expect(video.volume).toBe(0.4);
  fireEvent.click(screen.getByRole("button", {name: "静音"}));
  expect(video.muted).toBe(true);
  unmount();
  expect(video.pause).toHaveBeenCalled();
  expect(video.getAttribute("src")).toBeNull();
});
it("offers an external player after a media decoding error", async () => {
  vi.spyOn(api, "playbackPath").mockResolvedValue("movie.mp4");
  const open = vi.spyOn(api, "openFile").mockResolvedValue();
  const task = designFixture().tasks[0];
  const {container} = render(<VideoPlayer task={task} onClose={vi.fn()} />);
  await waitFor(() => expect(container.querySelector("video")!.getAttribute("src")).toContain("movie.mp4"));
  fireEvent.error(container.querySelector("video")!);
  fireEvent.click(screen.getByRole("button", {name: "系统播放器打开"}));
  expect(open).toHaveBeenCalledWith(task.id);
});

it("remembers volume and mute across reopening and different videos", async () => {
  vi.spyOn(api, "playbackPath").mockResolvedValue("movie.mp4");
  const first = render(<VideoPlayer task={designFixture().tasks[0]} onClose={vi.fn()}/>);
  fireEvent.change(screen.getByRole("slider", {name:"音量"}), {target:{value:"0.27"}});
  fireEvent.click(screen.getByRole("button", {name:"静音"}));
  first.unmount();
  const reopened=render(<VideoPlayer task={{...designFixture().tasks[0],id:"another-video"}} onClose={vi.fn()}/>);
  const video=reopened.container.querySelector("video")!;
  // Settings must already be applied while the file path is still loading.
  expect(video.volume).toBe(0.27);expect(video.muted).toBe(true);
  await waitFor(()=>expect(video.getAttribute("src")).toContain("movie.mp4"));
  expect(video.volume).toBe(0.27);expect(video.muted).toBe(true);
  fireEvent.click(screen.getByRole("button",{name:"取消静音"}));
  expect(video.volume).toBe(0.27);expect(video.muted).toBe(false);
  expect(JSON.parse(localStorage.getItem("framefetch.previewAudio")!)).toEqual({volume:0.27,muted:false});
});
it("restores zero volume and tolerates invalid saved preferences", () => {
  vi.spyOn(api, "playbackPath").mockImplementation(()=>new Promise(()=>{}));
  for(const [stored,expected] of [['{"volume":0,"muted":false}',0],['broken-json',1],['{"volume":5}',1],['{"volume":-2}',0]] as const){
    localStorage.setItem("framefetch.previewAudio",stored);
    const view=render(<VideoPlayer task={designFixture().tasks[0]} onClose={vi.fn()}/>);
    expect(view.container.querySelector("video")!.volume).toBe(expected);
    view.unmount();
  }
});
