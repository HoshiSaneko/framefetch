// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import FrameApp from "./FrameApp";
import { api } from "./bridge";
import { designFixture } from "./designFixture";

afterEach(() => {cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals();});
it("shows both connections only in the footer and refreshes after a session changes", async () => {
  vi.stubGlobal("ResizeObserver", class {observe() {} unobserve() {} disconnect() {}});
  const data = designFixture();
  data.account.connected = true;
  vi.spyOn(api,"snapshot").mockResolvedValue(data);
  const status = vi.spyOn(api,"douyinStatus").mockResolvedValue({sessionPresent:true});
  const {container} = render(<FrameApp/>);
  await screen.findByText("Telegram 已连接");
  await screen.findByText("抖音 已连接");
  expect(container.querySelector(".account-pill")).toBeNull();
  expect(container.querySelector(".sidebar-platforms")).toBeNull();
  expect(container.querySelector(".app-statusbar")?.textContent).toContain("抖音 已连接");
  status.mockResolvedValue({sessionPresent:false});
  fireEvent(window,new Event("focus"));
  await waitFor(()=>expect(screen.queryByText("抖音 已连接")).toBeNull());
  expect(screen.getByText("Telegram 已连接")).toBeTruthy();
});
