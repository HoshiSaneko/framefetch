// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LoginModal } from "./LoginModal";
import { api } from "./bridge";
import type { QrLoginResult, Settings } from "./types";

vi.mock("./bridge", () => ({
  desktop: true,
  api: {
    startQr: vi.fn(),
    pollQr: vi.fn(),
    password: vi.fn(),
    cancelQr: vi.fn(async () => {}),
  },
}));
vi.mock("qrcode", () => ({
  default: {
    toDataURL: vi.fn(
      async (url: string) => `data:image/png;base64,${btoa(url)}`,
    ),
  },
}));
const settings: Settings = {
  apiId: "12345",
  apiHash: "a".repeat(32),
  concurrency: 2,
  downloadDir: "C:\\Downloads",
  proxyUrl: "",
};
const code = (expiresAt = 130, token = "test"): QrLoginResult => ({
  step: "qr",
  url: `tg://login?token=${token}`,
  expiresAt,
  hint: "",
  account: null,
});
const flush = async () => {
  await act(async () => {});
};
function mount(config = settings) {
  const onClose = vi.fn(),
    onConnected = vi.fn();
  const view = render(
    <LoginModal
      settings={config}
      onClose={onClose}
      onConnected={onConnected}
    />,
  );
  return { ...view, onClose, onConnected };
}
beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(100000);
  vi.clearAllMocks();
  vi.mocked(api.startQr).mockResolvedValue(code());
  vi.mocked(api.pollQr).mockResolvedValue(code());
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("QR login lifecycle", () => {
  it("opens a QR directly with saved config and does not restart on account refresh", async () => {
    const view = mount();
    await flush();
    expect(screen.getByRole("img").getAttribute("src")).toMatch(/^data:image/);
    expect(screen.queryByLabelText("手机号")).toBeNull();
    view.rerender(
      <LoginModal
        settings={{ ...settings }}
        onClose={view.onClose}
        onConnected={view.onConnected}
      />,
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(api.startQr).toHaveBeenCalledTimes(1);
    expect(api.pollQr).toHaveBeenCalledTimes(2);
  });
  it("does not send credentials until first-time config is valid", async () => {
    mount({ ...settings, apiId: "", apiHash: "" });
    expect(api.startQr).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("API ID"), {
      target: { value: "2147483648" },
    });
    fireEvent.change(screen.getByLabelText("API Hash"), {
      target: { value: "a".repeat(32) },
    });
    fireEvent.click(screen.getByRole("button", { name: "生成登录二维码" }));
    expect(screen.getByRole("alert").textContent).toContain("有效的 API ID");
    expect(api.startQr).not.toHaveBeenCalled();
  });
  it("hides an expired QR while awaiting a replacement", async () => {
    vi.mocked(api.startQr).mockResolvedValue(code(101));
    let resolve!: (value: QrLoginResult) => void;
    vi.mocked(api.pollQr).mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    mount();
    await flush();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });
    expect(screen.queryByRole("img")).toBeNull();
    await act(async () => {
      resolve(code(150, "new"));
    });
    expect(screen.getByRole("img")).toBeTruthy();
  });
  it("keeps the attempt through 2FA, stops polling, and allows wrong-password retry", async () => {
    vi.mocked(api.pollQr).mockResolvedValue({
      ...code(),
      step: "password",
      url: null,
      hint: "提示",
    });
    const view = mount();
    await flush();
    const id = vi.mocked(api.startQr).mock.calls[0][1];
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(api.pollQr).toHaveBeenCalledTimes(1);
    expect(api.cancelQr).not.toHaveBeenCalled();
    expect(screen.queryByRole("img")).toBeNull();
    vi.mocked(api.password).mockRejectedValueOnce("两步验证密码不正确");
    fireEvent.change(screen.getByLabelText("两步验证密码"), {
      target: { value: "wrong" },
    });
    fireEvent.click(screen.getByRole("button", { name: "验证并连接" }));
    await flush();
    expect(screen.getByRole("alert").textContent).toContain("密码不正确");
    const account = { connected: true, name: "Test", username: "test" };
    vi.mocked(api.password).mockResolvedValueOnce({
      step: "connected",
      hint: "",
      account,
    });
    fireEvent.change(screen.getByLabelText("两步验证密码"), {
      target: { value: "right" },
    });
    fireEvent.click(screen.getByRole("button", { name: "验证并连接" }));
    await flush();
    expect(api.password).toHaveBeenLastCalledWith("right", id);
    expect(view.onConnected).toHaveBeenCalledWith(account);
    expect(view.onClose).toHaveBeenCalledTimes(1);
  });
  it("ignores a late successful response after close and cancels the same attempt", async () => {
    let resolve!: (value: QrLoginResult) => void;
    vi.mocked(api.startQr).mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const view = mount();
    const id = vi.mocked(api.startQr).mock.calls[0][1];
    view.unmount();
    expect(api.cancelQr).toHaveBeenCalledWith(id);
    await act(async () => {
      resolve({
        ...code(),
        step: "connected",
        account: { connected: true, name: "Test", username: "" },
      });
    });
    expect(view.onConnected).not.toHaveBeenCalled();
    expect(api.pollQr).not.toHaveBeenCalled();
  });
  it("completes a scan without 2FA and stops polling", async () => {
    const account = { connected: true, name: "Test", username: "" };
    vi.mocked(api.pollQr).mockResolvedValue({
      ...code(),
      step: "connected",
      url: null,
      account,
    });
    const view = mount();
    await flush();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(api.pollQr).toHaveBeenCalledTimes(1);
    expect(view.onConnected).toHaveBeenCalledWith(account);
    expect(view.onClose).toHaveBeenCalledTimes(1);
  });
});
