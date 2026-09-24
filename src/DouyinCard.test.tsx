// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { DouyinCard } from "./DouyinCard";
import { api } from "./bridge";
vi.mock("./bridge", () => ({desktop: true, api: {douyinQrStart: vi.fn(), douyinQrPoll: vi.fn(), douyinQrCancel: vi.fn(), douyinProfile: vi.fn().mockResolvedValue({name: "测试账号", avatar: "https://p3.douyinpic.com/avatar.jpg"}), douyinStatus: vi.fn(), douyinOpen: vi.fn(), douyinFinish: vi.fn(), douyinLogout: vi.fn()}}));
afterEach(() => {cleanup(); vi.resetAllMocks();});
it("shows the website QR and completes automatically on a detected session", async () => {
  vi.mocked(api.douyinProfile).mockResolvedValue({name: "测试账号", avatar: "https://p3.douyinpic.com/avatar.jpg"});
  vi.mocked(api.douyinStatus).mockResolvedValue({sessionPresent: false});
  vi.mocked(api.douyinQrStart).mockResolvedValue("attempt-1");
  vi.mocked(api.douyinQrCancel).mockResolvedValue();
  vi.mocked(api.douyinQrPoll).mockResolvedValueOnce({loggedIn: false, image: "data:image/png;base64,test"}).mockResolvedValue({loggedIn: true, image: null});
  render(<DouyinCard />);
  fireEvent.click(screen.getByRole("button", {name: "扫码登录"}));
  await screen.findByRole("img", {name: "抖音登录二维码"});
  expect(screen.queryByText("会话已保存")).toBeNull();
  await screen.findByRole("button", {name: "断开连接"}, {timeout: 4000});
  expect(screen.queryByRole("dialog")).toBeNull();
  await waitFor(() => expect(api.douyinQrCancel).toHaveBeenCalledWith("attempt-1"));
});
it("restores a saved session and clears it when disconnected", async () => {
  vi.mocked(api.douyinProfile).mockResolvedValue({name: "测试账号", avatar: "https://p3.douyinpic.com/avatar.jpg"});
  vi.mocked(api.douyinStatus).mockResolvedValue({sessionPresent: true});
  vi.mocked(api.douyinLogout).mockResolvedValue();
  render(<DouyinCard />);
  await screen.findByText("测试账号");
  expect(screen.getByRole("img", {name: "账号头像"})).toBeTruthy();
  expect(screen.queryByRole("button", {name: "打开抖音"})).toBeNull();
  fireEvent.click(await screen.findByRole("button", {name: "断开连接"}));
  await waitFor(() => expect(api.douyinLogout).toHaveBeenCalledOnce());
  await screen.findByRole("button", {name: "扫码登录"});
  expect(screen.queryByText("下载功能尚未接入")).toBeNull();
});


