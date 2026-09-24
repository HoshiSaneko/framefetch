// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AboutModal } from "./AboutModal";
vi.mock("./bridge", () => ({ desktop: true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("opens the author and project in the system browser", () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  render(<AboutModal onClose={() => {}}/>);
  const author = screen.getByRole("link", { name: "Saneko" });
  const project = screen.getByRole("link", { name: "github.com/HoshiSaneko/framefetch" });
  expect(author.getAttribute("href")).toBe("https://github.com/HoshiSaneko");
  expect(project.getAttribute("href")).toBe("https://github.com/HoshiSaneko/framefetch");
  fireEvent.click(author);
  fireEvent.click(project);
  expect(invoke).toHaveBeenNthCalledWith(1, "open_external_link", { url: "https://github.com/HoshiSaneko" });
  expect(invoke).toHaveBeenNthCalledWith(2, "open_external_link", { url: "https://github.com/HoshiSaneko/framefetch" });
});
it("reports a browser launch failure while retaining the copyable link", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("unavailable"));
  render(<AboutModal onClose={() => {}}/>);
  fireEvent.click(screen.getByRole("link", { name: "Saneko" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("无法打开浏览器"));
  expect(screen.getByRole("link", { name: "Saneko" }).getAttribute("href")).toBe("https://github.com/HoshiSaneko");
});
