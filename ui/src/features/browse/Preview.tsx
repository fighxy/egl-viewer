import { ChevronLeft, ChevronRight, Minus, Plus, RotateCw, X } from "lucide-react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { isEditable } from "@/lib/desktop";
import { originalUrl, showsOriginal, thumbUrl } from "@/lib/thumb";
import { PREVIEW_TRANSITION } from "./ItemsView";
import { useCells } from "./useView";

type Props = {
  viewId: number;
  total: number;
  /** The position shown, or `null` when the preview is closed. */
  index: number | null;
  onIndexChange: (index: number) => void;
  /** Gets the id of the item shown last, for the transition back into its cell. */
  onClose: (id: string | null) => void;
  onLost: () => void;
};

const MAX_ZOOM = 8;

/**
 * The full-size preview (docs/spec.md §7.4): a layer over the window with the original at
 * window size, zoom and pan, rotate, and the previous and next items of the view. Originals the
 * WebView cannot show (TIFF, PSD) show their thumbnail. Rotation is session-only (D-058).
 */
export function Preview({ viewId, total, index, onIndexChange, onClose, onLost }: Props) {
  const at = index ?? 0;
  const cell = useCells(index === null ? null : viewId, at, at, onLost)(at);
  const [zoom, setZoom] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  /** Clockwise degrees: 0, 90, 180 or 270. Not written to the library (D-058). */
  const [rotation, setRotation] = useState(0);
  const [failed, setFailed] = useState(false);
  const [stageSize, setStageSize] = useState({ w: 0, h: 0 });
  const drag = useRef<{ x: number; y: number } | null>(null);
  const layer = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: every item starts fitted and upright.
  useEffect(() => {
    setZoom(1);
    setOffset({ x: 0, y: 0 });
    setRotation(0);
    setFailed(false);
  }, [index]);

  useEffect(() => {
    const el = stage.current;
    if (!el || index === null) return;
    const sync = () => setStageSize({ w: el.clientWidth, h: el.clientHeight });
    sync();
    const observer = new ResizeObserver(sync);
    observer.observe(el);
    return () => observer.disconnect();
  }, [index]);

  const zoomTo = (next: number) => {
    const clamped = Math.min(MAX_ZOOM, Math.max(1, next));
    setZoom(clamped);
    if (clamped === 1) setOffset({ x: 0, y: 0 });
  };
  const rotateCw = () => setRotation((deg) => (deg + 90) % 360);
  const go = (step: -1 | 1) => {
    if (index === null) return;
    const next = index + step;
    if (next >= 0 && next < total) onIndexChange(next);
  };

  let source: string | null = null;
  if (cell && !failed) {
    if (showsOriginal(cell.ext)) source = originalUrl(cell.id);
    else if (cell.hasThumbnail) source = thumbUrl(cell);
  }

  // After a 90°/270° rotate the layout box is still upright, so swap the fit limits.
  const sideways = rotation % 180 !== 0;
  const fitW = sideways ? stageSize.h : stageSize.w;
  const fitH = sideways ? stageSize.w : stageSize.h;

  return (
    <DialogPrimitive.Root
      open={index !== null}
      onOpenChange={(open) => {
        if (!open) onClose(cell?.id ?? null);
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/90" />
        <DialogPrimitive.Content
          ref={layer}
          aria-describedby={undefined}
          className="fixed inset-0 z-50 flex flex-col text-white outline-none"
          // The layer itself takes focus, so no button shows a focus ring on open.
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            layer.current?.focus();
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowLeft") go(-1);
            else if (event.key === "ArrowRight") go(1);
            else if (event.key === "+" || event.key === "=") zoomTo(zoom * 1.25);
            else if (event.key === "-") zoomTo(zoom / 1.25);
            else if (event.key === "0") zoomTo(1);
            else if (
              event.key.toLowerCase() === "r" &&
              !event.ctrlKey &&
              !event.metaKey &&
              !event.altKey &&
              !isEditable(event.target)
            ) {
              rotateCw();
            } else return;
            event.preventDefault();
          }}
        >
          <div className="flex h-11 shrink-0 items-center gap-3 px-3 text-[13px]">
            <DialogPrimitive.Title className="truncate font-medium">
              {cell?.name ?? "Preview"}
            </DialogPrimitive.Title>
            <span className="text-white/60 tabular-nums">
              {(at + 1).toLocaleString("en")} of {total.toLocaleString("en")}
            </span>
            <div className="ml-auto flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Zoom out"
                className="text-white hover:bg-white/10 hover:text-white"
                disabled={zoom <= 1}
                onClick={() => zoomTo(zoom / 1.25)}
              >
                <Minus aria-hidden />
              </Button>
              <button
                type="button"
                className="w-12 rounded text-center text-xs tabular-nums hover:bg-white/10"
                title="Fit to window (0)"
                onClick={() => zoomTo(1)}
              >
                {Math.round(zoom * 100)}%
              </button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Zoom in"
                className="text-white hover:bg-white/10 hover:text-white"
                disabled={zoom >= MAX_ZOOM}
                onClick={() => zoomTo(zoom * 1.25)}
              >
                <Plus aria-hidden />
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Rotate clockwise"
                title="Rotate clockwise (R)"
                className="text-white hover:bg-white/10 hover:text-white"
                onClick={rotateCw}
              >
                <RotateCw aria-hidden />
              </Button>
              <DialogPrimitive.Close asChild>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Close preview"
                  className="text-white hover:bg-white/10 hover:text-white"
                >
                  <X aria-hidden />
                </Button>
              </DialogPrimitive.Close>
            </div>
          </div>
          {/* biome-ignore lint/a11y/noStaticElementInteractions: mouse zoom and pan; the keyboard has + − 0 on the dialog. */}
          <div
            ref={stage}
            className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden"
            onWheel={(event) => zoomTo(zoom * (event.deltaY < 0 ? 1.1 : 1 / 1.1))}
            onMouseDown={(event) => {
              if (zoom > 1 && event.button === 0) {
                event.preventDefault();
                drag.current = { x: event.clientX - offset.x, y: event.clientY - offset.y };
              }
            }}
            onMouseMove={(event) => {
              if (drag.current) {
                setOffset({
                  x: event.clientX - drag.current.x,
                  y: event.clientY - drag.current.y,
                });
              }
            }}
            onMouseUp={() => {
              drag.current = null;
            }}
            onMouseLeave={() => {
              drag.current = null;
            }}
          >
            {source ? (
              <img
                key={source}
                src={source}
                alt={cell?.name ?? ""}
                draggable={false}
                onError={() => setFailed(true)}
                className="object-contain select-none"
                style={{
                  maxWidth: fitW > 0 ? fitW : "100%",
                  maxHeight: fitH > 0 ? fitH : "100%",
                  transform: `translate(${offset.x}px, ${offset.y}px) rotate(${rotation}deg) scale(${zoom})`,
                  cursor: zoom > 1 ? "grab" : "default",
                  viewTransitionName: PREVIEW_TRANSITION,
                }}
              />
            ) : (
              cell && (
                <div className="grid justify-items-center gap-2 text-white/70">
                  <span className="text-2xl font-medium uppercase">{cell.ext}</span>
                  <span className="text-sm">No preview for this file.</span>
                </div>
              )
            )}
            <Button
              variant="ghost"
              size="icon"
              aria-label="Previous"
              className="absolute left-3 text-white hover:bg-white/10 hover:text-white"
              disabled={at === 0}
              onClick={() => go(-1)}
            >
              <ChevronLeft aria-hidden />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Next"
              className="absolute right-3 text-white hover:bg-white/10 hover:text-white"
              disabled={at >= total - 1}
              onClick={() => go(1)}
            >
              <ChevronRight aria-hidden />
            </Button>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
