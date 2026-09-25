// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { NewDownload } from "./FrameApp";
import { api } from "./bridge";
import { previewPlatforms } from "./platforms";

beforeEach(() => {
  vi.spyOn(api, "bilibiliStatus").mockResolvedValue({sessionPresent: false});
  vi.spyOn(api, "xiaohongshuStatus").mockResolvedValue({sessionPresent: false, name: null, avatar: null});
});

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("shows logged-in Douyin collections inside New Download and preserves the pasted link", async () => {
  vi.spyOn(api,"douyinStatus").mockResolvedValue({sessionPresent:true});
  const library=vi.spyOn(api,"douyinLibrary").mockResolvedValue({items:[],cursor:"0",hasMore:false});
  render(<NewDownload platforms={previewPlatforms} connected={false} onClose={vi.fn()} onLogin={vi.fn()} onAdded={async()=>{}}/>);
  fireEvent.change(screen.getByRole("textbox",{name:"媒体链接"}),{target:{value:"https://t.me/demo/123"}});
  expect(screen.queryByText("我的喜欢")).toBeNull();
  expect(screen.queryByText("收藏夹")).toBeNull();
  fireEvent.click(await screen.findByRole("button",{name:"抖音"}));
  await waitFor(()=>expect(library).toHaveBeenCalledWith("likes","0",undefined));
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(screen.queryByRole("textbox",{name:"媒体链接"})).toBeNull();
  fireEvent.click(screen.getByRole("combobox",{name:"抖音作品来源"}));
  fireEvent.click(screen.getByRole("option",{name:"我的收藏夹"}));
  await waitFor(()=>expect(library).toHaveBeenCalledWith("folders","0",undefined));
  fireEvent.click(screen.getByRole("button",{name:"链接下载"}));
  expect((screen.getByRole("textbox",{name:"媒体链接"}) as HTMLInputElement).value).toBe("https://t.me/demo/123");
});

it("hides Douyin library entries without a saved session", async () => {
  const status=vi.spyOn(api,"douyinStatus").mockResolvedValue({sessionPresent:false});
  const library=vi.spyOn(api,"douyinLibrary");
  render(<NewDownload platforms={previewPlatforms} connected onClose={vi.fn()} onLogin={vi.fn()} onAdded={async()=>{}}/>);
  await waitFor(()=>expect(status).toHaveBeenCalled());
  expect(screen.queryByRole("button",{name:"我的喜欢"})).toBeNull();
  expect(screen.queryByRole("button",{name:"收藏夹"})).toBeNull();
  expect(screen.queryByRole("button",{name:"抖音"})).toBeNull();
  expect(library).not.toHaveBeenCalled();
});

it("queues a Douyin share link without requiring a Telegram connection", async () => {
  vi.spyOn(api, "douyinStatus").mockResolvedValue({sessionPresent: true});
  const enqueue = vi.spyOn(api, "enqueue").mockResolvedValue();
  const close = vi.fn();
  render(<NewDownload platforms={previewPlatforms} connected={false} onClose={close} onLogin={vi.fn()} onAdded={async () => {}} />);
  fireEvent.change(screen.getByRole("textbox", {name: "媒体链接"}), {target: {value: "分享作品 https://v.douyin.com/abc/ 复制打开抖音"}});
  fireEvent.click(screen.getByRole("button", {name: "加入下载"}));
  await waitFor(() => expect(enqueue).toHaveBeenCalledWith("https://v.douyin.com/abc/"));
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
});

it("keeps a Douyin link for retry when the account is disconnected", async () => {
  vi.spyOn(api, "douyinStatus").mockResolvedValue({sessionPresent: false});
  const enqueue = vi.spyOn(api, "enqueue");
  render(<NewDownload platforms={previewPlatforms} connected onClose={vi.fn()} onLogin={vi.fn()} onAdded={async () => {}} />);
  fireEvent.change(screen.getByRole("textbox", {name: "媒体链接"}), {target: {value: "https://www.douyin.com/video/123"}});
  fireEvent.click(screen.getByRole("button", {name: "加入下载"}));
  await screen.findByText("请先在平台连接中登录抖音。");
  expect(enqueue).not.toHaveBeenCalled();
});

it("adds consecutive links without waiting for media preview", async () => {
  const preview = vi.spyOn(api, "preview").mockImplementation(() => new Promise(() => {}));
  const enqueue = vi.spyOn(api, "enqueue").mockResolvedValue(undefined);
  for (const id of [123, 124]) {
    const close = vi.fn();
    render(<NewDownload platforms={previewPlatforms} connected onClose={close} onLogin={vi.fn()} onAdded={async () => {}} />);
    expect(screen.queryByText("文件将保存到偏好设置中的下载文件夹。")).toBeNull();
    fireEvent.change(screen.getByRole("textbox", {name: "媒体链接"}), {target: {value: `https://t.me/channel/${id}`}});
    fireEvent.click(screen.getByRole("button", {name: "加入下载"}));
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
    expect(enqueue).toHaveBeenLastCalledWith(`https://t.me/channel/${id}`);
    cleanup();
  }
  expect(preview).not.toHaveBeenCalled();
  expect(enqueue).toHaveBeenCalledTimes(2);
});

it("keeps the link available for retry if enqueue fails", async () => {
  vi.spyOn(api, "enqueue").mockRejectedValue(new Error("无法保存工作区"));
  const close = vi.fn();
  render(<NewDownload platforms={previewPlatforms} connected onClose={close} onLogin={vi.fn()} onAdded={async () => {}} />);
  const input = screen.getByRole("textbox", {name: "媒体链接"}) as HTMLInputElement;
  fireEvent.change(input, {target: {value: "https://t.me/channel/123"}});
  fireEvent.click(screen.getByRole("button", {name: "加入下载"}));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("无法保存工作区"));
  expect(close).not.toHaveBeenCalled();
  expect(input.value).toBe("https://t.me/channel/123");
  expect((screen.getByRole("button", {name: "加入下载"}) as HTMLButtonElement).disabled).toBe(false);
});

it("opens Telegram bulk selection only for a connected account", async () => {
  vi.spyOn(api, "douyinStatus").mockResolvedValue({sessionPresent:false});
  const props = {platforms:previewPlatforms, onClose:vi.fn(), onLogin:vi.fn(), onAdded:async()=>{}};
  const {rerender} = render(<NewDownload {...props} connected={false}/>);
  expect(screen.queryByRole("button", {name:"Telegram"})).toBeNull();
  rerender(<NewDownload {...props} connected/>);
  fireEvent.click(screen.getByRole("button", {name:"Telegram"}));
  expect(screen.getByRole("textbox", {name:"消息链接"})).toBeTruthy();
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", {name:"链接下载"}));
  expect(screen.getByRole("textbox", {name:"媒体链接"})).toBeTruthy();
});

it("queues Bilibili from Link Download without parsing or showing choices", async () => {
  vi.spyOn(api,"douyinStatus").mockResolvedValue({sessionPresent:false});
  const preview=vi.spyOn(api,"bilibiliPreview");
  const enqueue=vi.spyOn(api,"enqueue").mockResolvedValue();const close=vi.fn();
  render(<NewDownload platforms={previewPlatforms} connected={false} onClose={close} onLogin={vi.fn()} onAdded={async()=>{}}/>);
  fireEvent.change(screen.getByRole("textbox",{name:"媒体链接"}),{target:{value:"https://www.bilibili.com/video/BV1xx411c7mD"}});
  expect(screen.queryByRole("group",{name:"下载内容"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
  await waitFor(()=>expect(enqueue).toHaveBeenCalledWith("https://www.bilibili.com/video/BV1xx411c7mD"));
  await waitFor(()=>expect(close).toHaveBeenCalledOnce());expect(preview).not.toHaveBeenCalled();
});

it("offers a standalone Bilibili tab and keeps its link separate", async () => {
  vi.mocked(api.bilibiliStatus).mockResolvedValue({sessionPresent: true});
  vi.spyOn(api,"douyinStatus").mockResolvedValue({sessionPresent:false});
  const preview=vi.spyOn(api,"bilibiliPreview").mockResolvedValue({url:"https://www.bilibili.com/video/BV1xx411c7mD?p=1",title:"Bilibili 示例",thumbnail:null,formats:[{id:"80+30280",label:"1080P 高清"}],hasAudio:true});
  render(<NewDownload platforms={previewPlatforms} connected={false} onClose={vi.fn()} onLogin={vi.fn()} onAdded={async()=>{}}/>);
  fireEvent.change(screen.getByRole("textbox",{name:"媒体链接"}),{target:{value:"https://t.me/demo/123"}});
  fireEvent.click(await screen.findByRole("button",{name:"Bilibili"}));
  expect(screen.queryByRole("textbox",{name:"媒体链接"})).toBeNull();
  fireEvent.change(screen.getByRole("textbox",{name:"Bilibili 视频链接"}),{target:{value:"https://www.bilibili.com/video/BV1xx411c7mD"}});
  await screen.findByRole("group",{name:"下载内容"});expect(preview).toHaveBeenCalledOnce();
  expect(screen.getByRole("combobox",{name:"视频清晰度"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"链接下载"}));
  expect((screen.getByRole("textbox",{name:"媒体链接"}) as HTMLInputElement).value).toBe("https://t.me/demo/123");
  expect(screen.queryByRole("group",{name:"下载内容"})).toBeNull();
  fireEvent.click(await screen.findByRole("button",{name:"Bilibili"}));
  expect((screen.getByRole("textbox",{name:"Bilibili 视频链接"}) as HTMLInputElement).value).toBe("https://www.bilibili.com/video/BV1xx411c7mD");
});


it("keeps Xiaohongshu default link downloads separate from the preview tab",async()=>{
 vi.mocked(api.xiaohongshuStatus).mockResolvedValue({sessionPresent: true, name: null, avatar: null});
 vi.spyOn(api,"douyinStatus").mockResolvedValue({sessionPresent:false});
 const enqueue=vi.spyOn(api,"enqueue").mockResolvedValue();const preview=vi.spyOn(api,"xiaohongshuPreview");
 render(<NewDownload platforms={previewPlatforms} connected={false} onClose={vi.fn()} onLogin={vi.fn()} onAdded={async()=>{}}/>);
 fireEvent.change(screen.getByRole("textbox",{name:"媒体链接"}),{target:{value:"分享 http://xhslink.com/a/abc 复制打开"}});
 fireEvent.click(await screen.findByRole("button",{name:"小红书"}));
 expect((screen.getByRole("textbox",{name:"小红书作品链接"}) as HTMLInputElement).value).toBe("");
 fireEvent.click(screen.getByRole("button",{name:"链接下载"}));
 fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
 await waitFor(()=>expect(enqueue).toHaveBeenCalledWith("http://xhslink.com/a/abc"));expect(preview).not.toHaveBeenCalled();
});


it("shows only platforms with confirmed sessions while another status check is pending or fails", async () => {
  vi.spyOn(api, "douyinStatus").mockResolvedValue({sessionPresent: false});
  let resolveBilibili!: (status: {sessionPresent: boolean}) => void;
  vi.mocked(api.bilibiliStatus).mockImplementation(() => new Promise(resolve => {resolveBilibili = resolve;}));
  vi.mocked(api.xiaohongshuStatus).mockRejectedValue(new Error("status unavailable"));
  render(<NewDownload platforms={previewPlatforms} connected={false} onClose={vi.fn()} onLogin={vi.fn()} onAdded={async()=>{}}/>);
  expect(screen.queryByRole("group", {name: "下载来源"})).toBeNull();
  for (const name of ["Telegram", "抖音", "Bilibili", "小红书"]) {
    expect(screen.queryByRole("button", {name})).toBeNull();
  }
  resolveBilibili({sessionPresent: true});
  expect(await screen.findByRole("button", {name: "Bilibili"})).toBeTruthy();
  for (const name of ["Telegram", "抖音", "小红书"]) {
    expect(screen.queryByRole("button", {name})).toBeNull();
  }
});
