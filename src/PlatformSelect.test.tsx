// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Modal } from "./Modal";
import { PlatformSelect } from "./PlatformSelect";
afterEach(cleanup);
const options = [{value:"all",label:"全部平台"},{value:"telegram",label:"Telegram"}];
it("commits keyboard selection and restores trigger focus", () => {
  const change = vi.fn();
  render(<PlatformSelect value="all" options={options} onChange={change} />);
  const trigger = screen.getByRole("combobox");
  fireEvent.keyDown(trigger, {key:"ArrowDown"});
  fireEvent.keyDown(trigger, {key:"End"});
  fireEvent.keyDown(trigger, {key:"Enter"});
  expect(change).toHaveBeenCalledWith("telegram");
  expect(screen.queryByRole("listbox")).toBeNull();
  expect(document.activeElement).toBe(trigger);
});
it("Escape and outside press dismiss without changing selection", () => {
  const change = vi.fn();
  render(<PlatformSelect value="telegram" options={options} onChange={change} />);
  const trigger = screen.getByRole("combobox");
  fireEvent.click(trigger);
  expect(screen.getByRole("option", {name:"Telegram"}).getAttribute("aria-selected")).toBe("true");
  fireEvent.keyDown(trigger, {key:"Escape"});
  expect(screen.queryByRole("listbox")).toBeNull();
  fireEvent.click(trigger);
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("listbox")).toBeNull();
  expect(change).not.toHaveBeenCalled();
});
it("mouse selection commits from the portal", () => {
  const change = vi.fn();
  render(<PlatformSelect value="all" options={options} onChange={change} />);
  fireEvent.click(screen.getByRole("combobox"));
  fireEvent.click(screen.getByRole("option", {name:"Telegram"}));
  expect(change).toHaveBeenCalledWith("telegram");
  expect(screen.queryByRole("listbox")).toBeNull();
});

it("Escape closes the dropdown before the enclosing dialog", () => {
  const close=vi.fn();
  render(<Modal title="下载选择" onClose={close}><PlatformSelect variant="field" value="all" options={options} onChange={vi.fn()} /></Modal>);
  const trigger=screen.getByRole("combobox");fireEvent.click(trigger);
  fireEvent.keyDown(trigger,{key:"Escape"});
  expect(screen.queryByRole("listbox")).toBeNull();expect(close).not.toHaveBeenCalled();
  fireEvent.keyDown(trigger,{key:"Escape"});expect(close).toHaveBeenCalledOnce();
});
it("disables the quality menu while a download is being queued", () => {
  const props={variant:"field" as const,value:"all",options,onChange:vi.fn()};
  const view=render(<PlatformSelect {...props}/>);
  fireEvent.click(screen.getByRole("combobox"));expect(screen.getByRole("listbox")).toBeTruthy();
  view.rerender(<PlatformSelect {...props} disabled/>);
  expect(screen.queryByRole("listbox")).toBeNull();expect((screen.getByRole("combobox") as HTMLButtonElement).disabled).toBe(true);
});
