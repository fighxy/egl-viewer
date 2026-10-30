// Dragging inside the window: folders in the tree, items onto folders and tag rows, items
// into a new manual order. Built on mouse events, not HTML5 drag and drop: with the file
// drop of the Tauri window on, Windows hands HTML5 drags to the window instead of the page
// (D-042, D-057).
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import type { ItemsTarget } from "@/ipc";

export type DragPayload =
  | { kind: "folder"; id: string; label: string }
  | { kind: "items"; target: ItemsTarget; count: number; label: string };

export type DropTarget =
  | { kind: "folder"; id: string; zone: "before" | "inside" | "after" }
  | { kind: "root" }
  | { kind: "cell"; index: number; id: string; side: "before" | "after" }
  | { kind: "tag"; name: string; row: string };

/** How a folder's inside zone, and a tag row, show that they would take the items. */
export const DROP_INSIDE_CLASS = "bg-primary/15 ring-1 ring-primary";

type Active = { payload: DragPayload; over: DropTarget | null; x: number; y: number };

type DragContextValue = {
  /**
   * Called from a mouse down: the drag starts once the mouse moved a few pixels, with the
   * payload `make` returns. A release before that runs `onClick`.
   */
  begin: (
    event: { clientX: number; clientY: number; button: number },
    make: () => DragPayload | null,
    onClick?: () => void,
  ) => void;
  active: Active | null;
};

const DragContext = createContext<DragContextValue>({ begin: () => {}, active: null });

export function useDrag() {
  return useContext(DragContext);
}

/** Pixels the mouse moves before a press becomes a drag. */
const THRESHOLD = 5;

/** Reads the drop target under the pointer from `data-drop` attributes. */
function targetAt(x: number, y: number, payload: DragPayload): DropTarget | null {
  const element = document.elementFromPoint?.(x, y);
  const zone = element instanceof Element ? element.closest<HTMLElement>("[data-drop]") : null;
  if (!zone) return null;
  const rect = zone.getBoundingClientRect();
  switch (zone.dataset.drop) {
    case "root":
      return { kind: "root" };
    case "folder": {
      const id = zone.dataset.id ?? "";
      if (payload.kind === "items") return { kind: "folder", id, zone: "inside" };
      const part = rect.height > 0 ? (y - rect.top) / rect.height : 0.5;
      return {
        kind: "folder",
        id,
        zone: part < 0.25 ? "before" : part > 0.75 ? "after" : "inside",
      };
    }
    case "cell": {
      const index = Number(zone.dataset.index);
      const id = zone.dataset.id ?? "";
      // Grid cells split left and right, list rows top and bottom.
      const after =
        zone.dataset.axis === "y"
          ? rect.height > 0 && y - rect.top > rect.height / 2
          : rect.width > 0 && x - rect.left > rect.width / 2;
      return { kind: "cell", index, id, side: after ? "after" : "before" };
    }
    case "tag": {
      const name = zone.dataset.name ?? "";
      if (name === "") return null;
      return { kind: "tag", name, row: zone.dataset.row ?? name };
    }
    default:
      return null;
  }
}

type Props = {
  children: ReactNode;
  /** Whether `payload` may drop on `target`; refused targets are not highlighted. */
  canDrop: (payload: DragPayload, target: DropTarget) => boolean;
  onDrop: (payload: DragPayload, target: DropTarget) => void;
};

/** Tracks one drag at a time and draws a label next to the pointer. */
export function DragProvider({ children, canDrop, onDrop }: Props) {
  const [active, setActive] = useState<Active | null>(null);
  const handlers = useRef({ canDrop, onDrop });
  handlers.current = { canDrop, onDrop };
  const cleanup = useRef<(() => void) | null>(null);

  useEffect(() => () => cleanup.current?.(), []);

  const begin = useCallback<DragContextValue["begin"]>((start, make, onClick) => {
    if (start.button !== 0) return;
    cleanup.current?.();
    let current: Active | null = null;
    const over = (payload: DragPayload, x: number, y: number) => {
      const target = targetAt(x, y, payload);
      return target && handlers.current.canDrop(payload, target) ? target : null;
    };
    const onMove = (event: MouseEvent) => {
      if (!current) {
        const moved =
          Math.abs(event.clientX - start.clientX) + Math.abs(event.clientY - start.clientY);
        if (moved < THRESHOLD) return;
        const payload = make();
        if (!payload) {
          stop();
          return;
        }
        current = { payload, over: null, x: event.clientX, y: event.clientY };
      }
      current = {
        ...current,
        x: event.clientX,
        y: event.clientY,
        over: over(current.payload, event.clientX, event.clientY),
      };
      setActive(current);
    };
    const onUp = () => {
      const done = current;
      stop();
      if (!done) onClick?.();
      else if (done.over) handlers.current.onDrop(done.payload, done.over);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") stop();
    };
    const stop = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("keydown", onKey, true);
      cleanup.current = null;
      setActive(null);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    window.addEventListener("keydown", onKey, true);
    cleanup.current = stop;
  }, []);

  const value = useMemo(() => ({ begin, active }), [begin, active]);
  return (
    <DragContext.Provider value={value}>
      {children}
      {active &&
        createPortal(
          <div
            aria-hidden
            className="pointer-events-none fixed z-50 rounded-md bg-primary px-2 py-0.5 text-xs text-primary-foreground shadow"
            style={{ left: active.x + 12, top: active.y + 12 }}
          >
            {active.payload.label}
          </div>,
          document.body,
        )}
    </DragContext.Provider>
  );
}

/** How a drop target shows that it would take the drag, for its own element. */
export function dropHint(active: Active | null, matches: (target: DropTarget) => boolean) {
  const target = active?.over;
  return target && matches(target) ? target : null;
}
