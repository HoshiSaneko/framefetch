// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { DownloadDirectoryModal } from "./DownloadDirectoryModal";
import { api } from "./bridge";
import { designFixture } from "./designFixture";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("keeps the directory when the picker is canceled and saves only on submit", async () => {
  vi.spyOn(api, "pickFolder").mockResolvedValue(null);
  const current = designFixture();
  vi.spyOn(api, "snapshot").mockResolvedValue(current);
  const save = vi.spyOn(api, "saveSettings").mockResolvedValue(current.settings);
  const close = vi.fn();
  const saved = vi.fn().mockResolvedValue(undefined);
  render(<DownloadDirectoryModal directory={"C:\\Downloads"} onClose={close} onSaved={saved}/>);
  fireEvent.click(screen.getByRole("button", {name:"选择文件夹"}));
  await waitFor(() => expect((screen.getByRole("button", {name:"保存为默认目录"}) as HTMLButtonElement).disabled).toBe(false));
  expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("C:\\Downloads");
  expect(save).not.toHaveBeenCalled();
  fireEvent.change(screen.getByRole("textbox"), {target:{value:"D:\\我的下载"}});
  fireEvent.click(screen.getByRole("button", {name:"保存为默认目录"}));
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
  expect(save).toHaveBeenCalledWith({...current.settings, downloadDir:"D:\\我的下载"});
  expect(saved).toHaveBeenCalledOnce();
});

it("keeps the dialog open and displays a failed save", async () => {
  vi.spyOn(api, "snapshot").mockResolvedValue(designFixture());
  vi.spyOn(api, "saveSettings").mockRejectedValue(new Error("请选择一个有效的绝对路径作为下载文件夹"));
  const close = vi.fn();
  render(<DownloadDirectoryModal directory="relative" onClose={close} onSaved={vi.fn()}/>);
  fireEvent.click(screen.getByRole("button", {name:"保存为默认目录"}));
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(close).not.toHaveBeenCalled();
});
