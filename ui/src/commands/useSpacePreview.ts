import { useLayoutEffect, useRef } from "react";
import { isInteractive, keyName } from "@/lib/desktop";
import { SPACE_HOLD_MS, type SpaceDown, spaceHold, spaceKeyDown, spaceKeyUp } from "./spacePreview";

type Options = {
  /** Off while another dialog blocks the grid (the preview itself stays enabled). */
  enabled: boolean;
  isOpen: boolean;
  /** When closed, Space opens only if an item is selected. */
  canOpen: boolean;
  open: () => void;
  close: () => void;
};

/**
 * Hold-vs-toggle Space for the full-size preview (D-073). Lives beside {@link useShortcuts}
 * because the preview counts as a modal for other shortcuts but Space must still run while it
 * is open.
 */
export function useSpacePreview({ enabled, isOpen, canOpen, open, close }: Options) {
  const latest = useRef({ isOpen, canOpen, open, close });
  useLayoutEffect(() => {
    latest.current = { isOpen, canOpen, open, close };
  });

  const down = useRef<SpaceDown | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useLayoutEffect(() => {
    const clearTimer = () => {
      if (timer.current !== null) {
        clearTimeout(timer.current);
        timer.current = null;
      }
    };
    const reset = () => {
      clearTimer();
      down.current = null;
    };

    if (!enabled) {
      reset();
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (keyName(event) !== "Space" || event.repeat) return;
      if (isInteractive(event.target)) return;
      if (down.current) return;
      const { isOpen, canOpen, open } = latest.current;
      if (!isOpen && !canOpen) return;
      event.preventDefault();
      const next = spaceKeyDown(isOpen);
      down.current = next.down;
      if (next.open) open();
      timer.current = setTimeout(() => {
        if (down.current) down.current = spaceHold(down.current);
      }, SPACE_HOLD_MS);
    };

    const onKeyUp = (event: KeyboardEvent) => {
      if (keyName(event) !== "Space") return;
      if (!down.current) return;
      event.preventDefault();
      const result = spaceKeyUp(down.current);
      reset();
      if (result.close) latest.current.close();
    };

    // A lost keyup (Alt-Tab while holding) must not leave a stuck "down" that fights the next tap.
    const onBlur = () => {
      if (!down.current) return;
      const held = down.current.held;
      reset();
      if (held) latest.current.close();
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", onBlur);
    return () => {
      reset();
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", onBlur);
    };
  }, [enabled]);
}
