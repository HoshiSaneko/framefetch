import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, CircleAlert, Info, X } from "lucide-react";
import "./notice.css";

export type NoticeKind = "success" | "info" | "warning" | "error";
const labels = {success: "操作成功", info: "提示", warning: "请留意", error: "操作未完成"};
const icons = {success: Check, info: Info, warning: CircleAlert, error: CircleAlert};
let host: HTMLDivElement | null = null;
let users = 0;

/** Every notification shares one portal stack, outside page and card layout. */
export function Notice({kind = "info", title, message, onClose, closeLabel = "关闭提示", action}: {
  kind?: NoticeKind;
  title?: string;
  message: string;
  onClose: () => void;
  closeLabel?: string;
  action?: {label: string; onClick: () => void};
}) {
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const close = useRef(onClose);
  close.current = onClose;
  const persistent = kind === "error" || kind === "warning" || !!action;
  useEffect(() => {
    if (!host) {
      host = document.createElement("div");
      host.className = "notice-stack";
      host.setAttribute("aria-label", "通知");
      host.setAttribute("role", "region");
      document.body.appendChild(host);
    }
    users += 1;
    setContainer(host);
    return () => {
      users -= 1;
      if (!users) {host?.remove(); host = null;}
    };
  }, []);
  useEffect(() => {
    if (persistent || hovered || focused) return;
    const timer = setTimeout(() => close.current(), 6500);
    return () => clearTimeout(timer);
  }, [message, kind, persistent, hovered, focused]);
  const Icon = icons[kind];
  return container ? createPortal(
    <div className={`notice notice--${kind}`} role={kind === "error" ? "alert" : "status"} aria-atomic="true"
      onMouseEnter={() => setHovered(true)} onMouseLeave={() => setHovered(false)}
      onFocusCapture={() => setFocused(true)} onBlurCapture={event => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocused(false);
      }}>
      <span className="notice-icon"><Icon size={18} aria-hidden="true"/></span>
      <div className="notice-content">
        <strong>{title || labels[kind]}</strong>
        <p>{message}</p>
        {action && <button type="button" className="notice-action" onClick={action.onClick}>{action.label}</button>}
      </div>
      <button type="button" className="notice-close" aria-label={closeLabel} onClick={onClose}><X size={16}/></button>
    </div>, container,
  ) : null;
}
