// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { TransferActivity } from "./TransferActivity";
import { designFixture } from "./designFixture";
afterEach(() => { cleanup(); vi.useRealTimers(); });
it("samples actual aggregate speed and freezes the history when paused", () => {
  vi.useFakeTimers();
  const task = {...designFixture().tasks[0], status: "downloading" as const, speed: 1024};
  const { rerender } = render(<TransferActivity tasks={[task, {...task, id: "second"}]} />);
  expect(screen.getByText("2 KB/s")).toBeTruthy();
  act(() => vi.advanceTimersByTime(2000));
  const chart = screen.getByRole("img");
  expect(chart.children).toHaveLength(30);
  expect((chart.lastElementChild as HTMLElement).style.height).toBe("34px");
  rerender(<TransferActivity tasks={[{...task, status: "paused"}]} />);
  const before = chart.innerHTML;
  act(() => vi.advanceTimersByTime(5000));
  expect(chart.innerHTML).toBe(before);
  expect(screen.getByText("已暂停")).toBeTruthy();
  rerender(<TransferActivity tasks={[task]} />);
  rerender(<TransferActivity tasks={[{...task, status: "completed"}]} />);
  expect(screen.getByText("下载完成")).toBeTruthy();
  act(() => vi.advanceTimersByTime(4000));
  expect(screen.getByText("拾帧")).toBeTruthy();
  rerender(<TransferActivity tasks={[]} />);
  expect(screen.getByText("拾帧")).toBeTruthy();
});
it("does not announce historical completed tasks on initial load or refresh", () => {
  const task = {...designFixture().tasks[0], status: "completed" as const};
  const { rerender } = render(<TransferActivity tasks={[task]} />);
  expect(screen.queryByText("下载完成")).toBeNull();
  rerender(<TransferActivity tasks={[]} />);
  rerender(<TransferActivity tasks={[{...task}]} />);
  expect(screen.getByText("拾帧")).toBeTruthy();
  expect(screen.queryByText("下载完成")).toBeNull();
});
