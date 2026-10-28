import { type KeyboardEvent, type PointerEvent, useRef } from "react";

type Props = {
  /** Which bar the handle resizes: the left bar grows to the right, the right bar to the left. */
  side: "left" | "right";
  width: number;
  min: number;
  max: number;
  label: string;
  /** `commit` is true when a drag or a key press ends: the width is then saved. */
  onChange: (width: number, commit: boolean) => void;
};

const STEP = 16;

/** A drag handle between a bar and the grid; arrow keys work too. */
export function Splitter({ side, width, min, max, label, onChange }: Props) {
  const start = useRef<{ x: number; width: number } | null>(null);
  const clamp = (value: number) => Math.min(max, Math.max(min, Math.round(value)));

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture?.(event.pointerId);
    start.current = { x: event.clientX, width };
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!start.current) return;
    const delta = event.clientX - start.current.x;
    onChange(clamp(start.current.width + (side === "left" ? delta : -delta)), false);
  };
  const onPointerUp = () => {
    if (!start.current) return;
    start.current = null;
    onChange(width, true);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const grow = side === "left" ? "ArrowRight" : "ArrowLeft";
    const shrink = side === "left" ? "ArrowLeft" : "ArrowRight";
    if (event.key === grow) onChange(clamp(width + STEP), true);
    else if (event.key === shrink) onChange(clamp(width - STEP), true);
    else return;
    event.preventDefault();
  };

  return (
    // biome-ignore lint/a11y/useSemanticElements: a focusable, draggable window splitter; <hr> cannot take focus or pointer drags.
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      className="relative z-20 w-1 shrink-0 cursor-col-resize bg-transparent outline-none after:absolute after:inset-y-0 after:left-1/2 after:w-px after:-translate-x-1/2 after:bg-border hover:after:w-0.5 hover:after:bg-ring focus-visible:after:w-0.5 focus-visible:after:bg-ring"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onKeyDown={onKeyDown}
    />
  );
}
