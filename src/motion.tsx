import {
  useLayoutEffect,
  useRef,
  useState,
  type HTMLAttributes,
} from "react";

// Fluent timing: press < state change < navigation. Keep all motion interruptible.
export const motion = {
  fast: 167,
  normal: 250,
  enter: "cubic-bezier(0, 0, 0, 1)",
  exit: "cubic-bezier(1, 0, 1, 1)",
};
const reducedMotion = () =>
  window.matchMedia("(prefers-reduced-motion: reduce)");

function animate(
  element: HTMLElement,
  frames: Keyframe[],
  options: KeyframeAnimationOptions,
  finish?: () => void,
) {
  const preference = reducedMotion();
  if (preference.matches) {
    finish?.();
    return () => {};
  }
  const animation = element.animate(frames, options);
  const done = () => {
    preference.removeEventListener("change", change);
    finish?.();
  };
  const change = () => {
    animation.finish();
  };
  preference.addEventListener("change", change);
  animation.onfinish = done;
  return () => {
    animation.onfinish = null;
    preference.removeEventListener("change", change);
    animation.cancel();
  };
}

/** Retain a surface for its exit; reopening resumes from its current visual position. */
export function MotionPresence({
  open,
  variant = "popover",
  focusOnOpen = false,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  open: boolean;
  focusOnOpen?: boolean;
  variant?: "popover" | "panel";
}) {
  const [mounted, setMounted] = useState(open);
  const ref = useRef<HTMLDivElement>(null);
  const interrupted = useRef<{ opacity: string; transform: string } | null>(
    null,
  );
  useLayoutEffect(() => {
    if (open && !mounted) {
      setMounted(true);
      return;
    }
    const element = ref.current;
    if (!element) return;
    if (open && focusOnOpen)
      element
        .querySelector<HTMLElement>("button")
        ?.focus({ preventScroll: true });
    const direction = element.classList.contains("opens-up") ? 1 : -1;
    const resting = { opacity: "1", transform: "translateY(0px) scale(1)" };
    const offset = {
      opacity: "0",
      transform:
        variant === "panel"
          ? "translateX(26px)"
          : `translateY(${direction * 6}px) scale(.98)`,
    };
    const start = interrupted.current || (open ? offset : resting);
    interrupted.current = null;
    const stop = animate(
      element,
      [start, open ? resting : offset],
      {
        duration: open ? motion.normal : motion.fast,
        easing: open ? motion.enter : motion.exit,
        fill: "both",
      },
      () => {
        if (!open) setMounted(false);
      },
    );
    return () => {
      if (element.isConnected) {
        const style = getComputedStyle(element);
        interrupted.current = {
          opacity: style.opacity,
          transform: style.transform,
        };
      }
      stop();
    };
  }, [open, mounted, focusOnOpen, variant]);
  return mounted ? (
    <div {...props} ref={ref} inert={!open} aria-hidden={!open}>
      {children}
    </div>
  ) : null;
}
