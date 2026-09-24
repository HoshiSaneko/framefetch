// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { parseTelegramLinks, TelegramBatch } from "./TelegramBatch";
import { api } from "./bridge";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });
it("deduplicates message links and rejects other platforms and channel homepages", () => {
  expect(parseTelegramLinks("https://t.me/s/demo/1?single\nhttp://telegram.me/demo/1\nhttps://t.me/c/123/2。\nhttps://t.me/demo\nhttps://douyin.com/video/3")).toEqual({links:["https://t.me/demo/1", "https://t.me/c/123/2"],invalid:2});
});
it("queues checked links only, preserves failed items, and never resubmits successes", async () => {
  const enqueue = vi.spyOn(api, "enqueue").mockResolvedValueOnce().mockRejectedValueOnce(new Error("连接已断开")).mockResolvedValue();
  const close = vi.fn();
  render(<TelegramBatch onAdded={async () => {}} onQueued={close} onBusyChange={vi.fn()}/>);
  fireEvent.change(screen.getByLabelText("消息链接"), {target:{value:"https://t.me/demo/1\nhttps://t.me/demo/2\nhttps://t.me/demo/3"}});
  fireEvent.click(screen.getByRole("button", {name:"识别链接"}));
  fireEvent.click(screen.getByLabelText("选择 https://t.me/demo/3"));
  expect(enqueue).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name:"下载所选"}));
  await screen.findByText("连接已断开");
  expect(enqueue.mock.calls.map(call => call[0])).toEqual(["https://t.me/demo/1", "https://t.me/demo/2"]);
  expect(screen.queryByLabelText("选择 https://t.me/demo/1")).toBeNull();
  expect(close).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name:"下载所选"}));
  await waitFor(() => expect(enqueue).toHaveBeenCalledTimes(3));
  expect(enqueue.mock.calls[2][0]).toBe("https://t.me/demo/2");
  const batch = enqueue.mock.calls[0][2];
  expect(batch?.id).toBeTruthy();
  expect(enqueue.mock.calls[1][2]).toEqual(batch);
  expect(enqueue.mock.calls[2][2]).toEqual(batch);
});

it("deduplicates forum and direct links to the same Telegram message", () => {
  expect(parseTelegramLinks("https://t.me/c/3942745692/3437/15942\nhttps://t.me/c/3942745692/15942").links).toEqual(["https://t.me/c/3942745692/15942"]);
});
