import {
  useEffect,
  useLayoutEffect,
  useRef,
  type RefObject,
} from "react";

export const fluentMotion = {
  press: 90,
  release: 230,
  enter: 280,
  exit: 150,
  ease: "cubic-bezier(.2,0,0,1)",
};
const preference = () =>
  window.matchMedia?.("(prefers-reduced-motion: reduce)");
export const canAnimate = (element: Element) =>
  !preference()?.matches && typeof element.animate === "function";

/** Each motion can be interrupted at its displayed position, without a jump. */
export function tween(
  element: HTMLElement,
  frames: Keyframe[],
  duration: number,
  finished?: () => void,
) {
  if (!canAnimate(element)) {
    finished?.();
    return () => {};
  }
  const media = preference();
  const animation = element.animate(frames, {
    duration,
    easing: fluentMotion.ease,
    fill: "both",
  });
  const settle = () => {
    media?.removeEventListener("change", reduce);
    animation.cancel();
    finished?.();
  };
  const reduce = () => {
    if (media?.matches) settle();
  };
  animation.onfinish = settle;
  media?.addEventListener("change", reduce);
  return () => {
    animation.onfinish = null;
    media?.removeEventListener("change", reduce);
    animation.cancel();
  };
}

export function useEntranceMotion(
  root: RefObject<HTMLElement | null>,
  key: string,
) {
  const last = useRef(key);
  const interrupted = useRef<{ opacity: string; transform: string } | null>(
    null,
  );
  useLayoutEffect(() => {
    const element = root.current;
    if (!element || last.current === key) return;
    last.current = key;
    let completed = false;
    const start = interrupted.current ?? {
      opacity: ".45",
      transform: "translateY(9px)",
    };
    interrupted.current = null;
    const stop = tween(
      element,
      [start, { opacity: "1", transform: "translateY(0px)" }],
      fluentMotion.enter,
      () => {
        completed = true;
      },
    );
    return () => {
      if (!completed) {
        const style = getComputedStyle(element);
        interrupted.current = {
          opacity: style.opacity,
          transform: style.transform,
        };
      }
      stop();
    };
  }, [root, key]);
}

export function usePressFeedback(root: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const container = root.current;
    if (!container) return;
    const animations = new Map<HTMLElement, () => void>();
    let pressed: HTMLElement | null = null;
    const move = (button: HTMLElement, down: boolean) => {
      if (!canAnimate(button)) {
        button.style.transform = "";
        return;
      }
      const current = getComputedStyle(button).transform;
      animations.get(button)?.();
      const scale = down
        ? Math.max(0.96, 1 - 2 / button.getBoundingClientRect().width)
        : 1;
      animations.set(
        button,
        tween(
          button,
          [{ transform: current }, { transform: `scale(${scale})` }],
          down ? fluentMotion.press : fluentMotion.release,
          () => animations.delete(button),
        ),
      );
      // A held press keeps its resting transform after the animation releases its fill.
      button.style.transform = down ? `scale(${scale})` : "";
    };
    const down = (event: Event) => {
      if (
        event instanceof PointerEvent &&
        (event.button !== 0 || !event.isPrimary)
      )
        return;
      const button = (event.target as Element).closest<HTMLButtonElement>(
        "button",
      );
      if (!button || button.disabled || !container.contains(button)) return;
      if (pressed && pressed !== button) move(pressed, false);
      pressed = button;
      move(button, true);
    };
    const up = () => {
      if (pressed) {
        move(pressed, false);
        pressed = null;
      }
    };
    const keyDown = (event: KeyboardEvent) => {
      if (!event.repeat && ["Enter", " "].includes(event.key)) down(event);
    };
    const keyUp = (event: KeyboardEvent) => {
      if (["Enter", " "].includes(event.key)) up();
    };
    container.addEventListener("pointerdown", down);
    container.addEventListener("keydown", keyDown);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    window.addEventListener("keyup", keyUp);
    window.addEventListener("blur", up);
    return () => {
      container.removeEventListener("pointerdown", down);
      container.removeEventListener("keydown", keyDown);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      window.removeEventListener("keyup", keyUp);
      window.removeEventListener("blur", up);
      for (const [button, stop] of animations) {
        stop();
        button.style.transform = "";
      }
    };
  }, [root]);
}

/** Only an inert visual copy exits; the actual dialog and its auth effects unmount immediately. */
export function useDialogMotion(root: RefObject<HTMLDivElement | null>) {
  useLayoutEffect(() => {
    const backdrop = root.current,
      panel = backdrop?.querySelector<HTMLElement>(".modal");
    if (!backdrop || !panel || !canAnimate(panel)) return;
    let painted = false;
    const frame = requestAnimationFrame(() => {
      painted = true;
    });
    const stopVeil = tween(backdrop, [{ opacity: 0 }, { opacity: 1 }], 170);
    const stopPanel = tween(
      panel,
      [
        { opacity: 0, transform: "translateY(14px) scale(.965)" },
        { opacity: 1, transform: "translateY(0) scale(1)" },
      ],
      fluentMotion.enter,
    );
    return () => {
      cancelAnimationFrame(frame);
      const panelStyle = getComputedStyle(panel),
        veilStyle = getComputedStyle(backdrop);
      const from = {
        opacity: panelStyle.opacity,
        transform: panelStyle.transform,
      };
      const veilOpacity = veilStyle.opacity;
      stopVeil();
      stopPanel();
      if (!painted || !canAnimate(panel)) return;
      const copy = backdrop.cloneNode(true) as HTMLDivElement;
      copy.classList.add("dialog-exit-copy");
      copy.setAttribute("aria-hidden", "true");
      copy.setAttribute("inert", "");
      copy.querySelectorAll("[id],[autofocus]").forEach((node) => {
        node.removeAttribute("id");
        node.removeAttribute("autofocus");
      });
      document.body.appendChild(copy);
      const child = copy.querySelector<HTMLElement>(".modal")!;
      child.removeAttribute("role");
      child.removeAttribute("aria-modal");
      tween(
        child,
        [from, { opacity: 0, transform: "translateY(8px) scale(.985)" }],
        fluentMotion.exit,
      );
      tween(
        copy,
        [{ opacity: veilOpacity }, { opacity: 0 }],
        fluentMotion.exit,
        () => copy.remove(),
      );
    };
  }, [root]);
}

