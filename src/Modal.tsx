import { useEffect, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { useDialogMotion } from "./winuiMotion";
export function Modal({
  title,
  subtitle,
  icon,
  onClose,
  children,
  busy = false,
  className = "",
}: {
  title: string;
  subtitle?: string;
  icon?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  busy?: boolean;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const backdrop = useRef<HTMLDivElement>(null);
  useDialogMotion(backdrop);
  const close = useRef(onClose);
  close.current = onClose;
  const busyRef = useRef(busy);
  busyRef.current = busy;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const background = Array.from(
      document.querySelectorAll<HTMLElement>(".app-sidebar,.app-body"),
    );
    const originalInert = background.map((element) =>
      element.hasAttribute("inert"),
    );
    background.forEach((element) => element.setAttribute("inert", ""));
    const timer = setTimeout(() => {
      const target =
        ref.current?.querySelector<HTMLElement>("input:not(:disabled)") ||
        ref.current?.querySelector<HTMLElement>("button:not(:disabled)");
      target?.focus();
    }, 30);
    const key = (e: KeyboardEvent) => {
      const dialogs = Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"][aria-modal="true"]')).filter(el => !el.closest('.dialog-exit-copy'));
      if (dialogs.at(-1) !== ref.current) return;
      if (e.key === "Escape" && !busyRef.current) {
        e.preventDefault();
        close.current();
      }
      if (e.key === "Tab") {
        const elements = ref.current?.querySelectorAll<HTMLElement>(
          "button:not(:disabled), input:not(:disabled), select, textarea, a[href]",
        );
        if (!elements?.length) return;
        const first = elements[0],
          last = elements[elements.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        }
        if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", key);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("keydown", key);
      background.forEach((element, index) => {
        if (!originalInert[index]) element.removeAttribute("inert");
      });
      previous?.focus();
    };
  }, []);
  return (
    <div
      ref={backdrop}
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !busy) onClose();
      }}
    >
      <div
        ref={ref}
        className={`modal ${className}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <button
          className="icon-button modal-close"
          onClick={onClose}
          disabled={busy}
          aria-label="关闭"
        >
          <X size={20} />
        </button>
        {icon && <div className="modal-symbol" aria-hidden="true">{icon}</div>}
        <h2>{title}</h2>
        {subtitle && <p className="modal-subtitle">{subtitle}</p>}
        {children}
      </div>
    </div>
  );
}

