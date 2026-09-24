import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ArrowUpRight, Copy, FolderOpen, Info, MoreHorizontal, Pause, Play, Trash2, X, SquareCheck } from "lucide-react";
import type { DownloadTask } from "./types";

export function TaskActions({ task, onAction, onSelect, busy = false }: {
  task: DownloadTask; onAction: (action: string) => void; onSelect: () => void; busy?: boolean;
}) {
  const [position, setPosition] = useState<{top: number; left: number} | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const active = ["queued", "resolving", "downloading"].includes(task.status);
  const completed = task.status === "completed";
  const items = [
    {label: "查看详情", icon: Info, run: onSelect},
    {label: "选择", icon: SquareCheck, run: () => onAction("select")},
    {label: "复制链接", icon: Copy, run: () => onAction("copy")},
    ...(completed ? [{label: "打开文件", icon: ArrowUpRight, run: () => onAction("open")}] : []),
    ...(!completed && task.status !== "canceled" ? [{label: "取消下载", icon: X, run: () => onAction("cancel")}] : []),
  ];
  const close = (focus = false) => { setPosition(null); if (focus) trigger.current?.focus(); };
  useEffect(() => {
    if (!position) return;
    menu.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const outside = (e: PointerEvent) => {
      if (!menu.current?.contains(e.target as Node) && !trigger.current?.contains(e.target as Node)) setPosition(null);
    };
    const dismiss = () => setPosition(null);
    document.addEventListener("pointerdown", outside);
    window.addEventListener("resize", dismiss);
    window.addEventListener("scroll", dismiss, true);
    return () => {
      document.removeEventListener("pointerdown", outside);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("scroll", dismiss, true);
    };
  }, [position]);
  return <>
    {completed ? <button disabled={busy} className="icon-button" aria-label="在文件夹中显示" title="在文件夹中显示" onClick={() => onAction("reveal")}><FolderOpen size={18} /></button>
      : task.status !== "canceled" ? <button disabled={busy} className="icon-button" aria-label={active ? "暂停下载" : task.status === "failed" ? "重试下载" : "继续下载"} title={active ? "暂停下载" : task.status === "failed" ? "重试下载" : "继续下载"} onClick={() => onAction(active ? "pause" : "resume")}>{active ? <Pause size={17} /> : <Play size={17} />}</button> : <span className="action-spacer" />}
    {active ? <button disabled={busy} className="icon-button cancel-task" aria-label="取消下载" title="取消下载，保留任务记录" onClick={() => onAction("cancel")}><X size={18} /></button>
      : <button className="icon-button remove-task" aria-label="移除记录" title="移除记录…" disabled={busy} onClick={() => onAction("remove")}><Trash2 size={17} /></button>}
    <button ref={trigger} className="icon-button" aria-label="更多操作" title="更多操作" aria-haspopup="menu" aria-expanded={!!position} aria-controls={position ? id : undefined} onClick={() => {
      if (position) { close(); return; }
      const rect = trigger.current!.getBoundingClientRect();
      const height = items.length * 44 + 16;
      setPosition({left: Math.max(8, Math.min(rect.right - 184, window.innerWidth - 192)), top: rect.bottom + height + 8 > window.innerHeight ? Math.max(8, rect.top - height - 8) : rect.bottom + 8});
    }}><MoreHorizontal size={20} /></button>
    {position && createPortal(<div ref={menu} id={id} role="menu" aria-label="更多操作" className="task-action-menu" style={position} onKeyDown={e => {
      const buttons = Array.from(menu.current!.querySelectorAll<HTMLButtonElement>("button"));
      const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
      if (e.key === "Escape") { e.preventDefault(); close(true); }
      else if (e.key === "Tab") { close(true); }
      else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) {
        e.preventDefault();
        buttons[e.key === "Home" ? 0 : e.key === "End" ? buttons.length - 1 : (current + (e.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
      }
    }}>{items.map(item => <button key={item.label} role="menuitem" onClick={() => {close(true); item.run();}}><item.icon size={17} />{item.label}</button>)}</div>, document.body)}
  </>;
}
