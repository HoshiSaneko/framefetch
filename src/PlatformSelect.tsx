import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";

type Option = { value: string; label: string };
export function PlatformSelect({ value, options, onChange, ariaLabel = "筛选平台", disabled = false, variant = "filter" }: {
  value: string; options: Option[]; onChange: (value: string) => void; ariaLabel?: string; disabled?: boolean; variant?: "filter" | "field";
}) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [position, setPosition] = useState({ top: 0, left: 0, width: 180, maxHeight: 280 });
  const selected = Math.max(0, options.findIndex(option => option.value === value));
  const show = () => {
    if (disabled || !options.length) return;
    const box = trigger.current!.getBoundingClientRect();
    const below = window.innerHeight - box.bottom - 16;
    const above = box.top - 16;
    const height = Math.min(options.length * 44 + 16, 280, Math.max(below, above));
    const width = Math.min(Math.max(180, box.width), window.innerWidth - 16);
    setPosition({ top: below >= height ? box.bottom + 8 : Math.max(8, box.top - height - 8),
      left: Math.max(8, Math.min(box.left, window.innerWidth - width - 8)), width, maxHeight: Math.max(44, height) });
    setActive(selected);
    setOpen(true);
  };
  const choose = (index: number) => { if (disabled || !options[index]) return; onChange(options[index].value); setOpen(false); trigger.current?.focus(); };
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!trigger.current?.contains(event.target as Node) && !menu.current?.contains(event.target as Node)) setOpen(false);
    };
    const close = (event: Event) => { if (event.type === "scroll" && menu.current?.contains(event.target as Node)) return; setOpen(false); };
    document.addEventListener("pointerdown", outside);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("pointerdown", outside);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [open]);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  useEffect(() => {
    if (open) menu.current?.querySelector(`[id="${id}-${active}"]`)?.scrollIntoView?.({block:"nearest"});
  }, [open, active, id]);
  return <>
    <button ref={trigger} className={`platform-select-trigger ${variant}`} disabled={disabled || !options.length} type="button" role="combobox"
      aria-label={ariaLabel} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? id : undefined}
      aria-activedescendant={open ? `${id}-${active}` : undefined}
      onClick={() => open ? setOpen(false) : show()}
      onKeyDown={event => {
        if (event.key === "Tab") { setOpen(false); return; }
        if (event.key === "Escape" && open) { event.preventDefault(); event.stopPropagation(); setOpen(false); return; }
        if (["ArrowDown", "ArrowUp", "Home", "End", "Enter", " "].includes(event.key)) {
          event.preventDefault();
          if (!open) { show(); return; }
          if (event.key === "Enter" || event.key === " ") { choose(active); return; }
          setActive(index => event.key === "Home" ? 0 : event.key === "End" ? options.length - 1 :
            (index + (event.key === "ArrowDown" ? 1 : -1) + options.length) % options.length);
        }
      }}>
      <span>{options[selected]?.label}</span><ChevronDown size={18} />
    </button>
    {open && !disabled && createPortal(<div ref={menu} id={id} className={`platform-select-menu ${variant}`} role="listbox" aria-label={ariaLabel}
      style={position}>
      {options.map((option, index) => <div key={option.value} id={`${id}-${index}`} role="option"
        aria-selected={value === option.value} className={`platform-select-option ${active === index ? "highlighted" : ""}`}
        onPointerMove={() => setActive(index)} onMouseDown={event => event.preventDefault()} onClick={() => choose(index)}>
        <span>{option.label}</span>{value === option.value && <Check size={18} />}
      </div>)}
    </div>, document.body)}
  </>;
}
