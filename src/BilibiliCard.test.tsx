// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { BilibiliCard } from "./BilibiliCard";
import { api } from "./bridge";
vi.mock("./bridge", () => ({desktop: true, api: {bilibiliQrStart: vi.fn(), bilibiliQrPoll: vi.fn(), bilibiliQrCancel: vi.fn(), bilibiliProfile: vi.fn().mockResolvedValue({name: "测试账号", avatar: "https://i0.hdslb.com/avatar.jpg"}), bilibiliStatus: vi.fn(), bilibiliOpen: vi.fn(), bilibiliFinish: vi.fn(), bilibiliLogout: vi.fn()}}));
afterEach(() => {cleanup(); vi.resetAllMocks();});
it("shows the website QR and completes automatically on a detected session", async () => {
  vi.mocked(api.bilibiliProfile).mockResolvedValue({name: "测试账号", avatar: "https://i0.hdslb.com/avatar.jpg"});
  vi.mocked(api.bilibiliStatus).mockResolvedValue({sessionPresent: false});
  vi.mocked(api.bilibiliQrStart).mockResolvedValue("attempt-1");
  vi.mocked(api.bilibiliQrCancel).mockResolvedValue();
  vi.mocked(api.bilibiliQrPoll).mockResolvedValueOnce({loggedIn: false, image: "data:image/png;base64,test"}).mockResolvedValue({loggedIn: true, image: null});
  render(<BilibiliCard />);
  fireEvent.click(screen.getByRole("button", {name: "扫码登录"}));
  await screen.findByRole("img", {name: "哔哩哔哩登录二维码"});
  expect(screen.queryByText("会话已保存")).toBeNull();
  await screen.findByRole("button", {name: "断开连接"}, {timeout: 4000});
  expect(screen.queryByRole("dialog")).toBeNull();
  await waitFor(() => expect(api.bilibiliQrCancel).toHaveBeenCalledWith("attempt-1"));
});
it("restores a saved session and clears it when disconnected", async () => {
  vi.mocked(api.bilibiliProfile).mockResolvedValue({name: "测试账号", avatar: "https://i0.hdslb.com/avatar.jpg"});
  vi.mocked(api.bilibiliStatus).mockResolvedValue({sessionPresent: true});
  vi.mocked(api.bilibiliLogout).mockResolvedValue();
  render(<BilibiliCard />);
  await screen.findByText("测试账号");
  expect(screen.getByRole("img", {name: "账号头像"})).toBeTruthy();
  expect(screen.queryByRole("button", {name: "打开哔哩哔哩"})).toBeNull();
  fireEvent.click(await screen.findByRole("button", {name: "断开连接"}));
  await waitFor(() => expect(api.bilibiliLogout).toHaveBeenCalledOnce());
  await screen.findByRole("button", {name: "扫码登录"});
  expect(screen.queryByText("下载功能尚未接入")).toBeNull();
});


