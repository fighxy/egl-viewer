import { useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "cn";
import {
  type CSSProperties,
  memo,
  type MouseEvent as ReactMouseEvent,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { type ItemCell, ipc, type ViewMode } from "@/ipc";
import { thumbUrl } from "@/lib/thumb";
import { formatBytes, formatDate, formatDimensions } from "./format";
import {
  gridLayout,
  indicesInRect,
  type Layout,
  listLayout,
  moveFocus,
  PADDING,
  type Rect,
} from "./geometry";
import {
  isSelected,
  NOTHING,
  only,
  rangeFrom,
  type Selection,
  toggle,
  withRange,
} from "./selection";
import { fetchCell, useCells } from "./useView";

type Props = {
  viewId: number;
  total: number;
  mode: ViewMode;
  /** Grid cell width in CSS pixels. */
  cellSize: number;
  showNames: boolean;
  selection: Selection;
  onSelectionChange: (selection: Selection) => void;
  /** Double-click: the full-size preview of the item at `index`. */
  onOpen: (index: number, id: string | null) => void;
  /** Enter: move focus to the details panel. */
  onEnter: () => void;
  /** Ctrl + mouse wheel: `1` larger, `-1` smaller. */
  onZoom: (step: 1 | -1) => void;
  /** Rust no longer has the view. */
  onLost: () => void;
  /** The item whose thumbnail morphs into the preview (View Transitions). */
  transitionId?: string | null;
};

export const PREVIEW_TRANSITION = "preview-image";

export function cellDomId(index: number) {
  return `item-cell-${index}`;
}

/** Keeps track of an element's inner width. */
function useWidth(ref: React.RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    setWidth(element.clientWidth);
    const observer = new ResizeObserver(() => setWidth(element.clientWidth));
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}

type Band = { start: { x: number; y: number }; end: { x: number; y: number }; additive: boolean };

function bandRect(band: Band): Rect {
  return {
    left: Math.min(band.start.x, band.end.x),
    top: Math.min(band.start.y, band.end.y),
    right: Math.max(band.start.x, band.end.x),
    bottom: Math.max(band.start.y, band.end.y),
  };
}

/**
 * The virtualized grid or list of a view (docs/spec.md §7.3): windows of cells come from Rust
 * as they scroll into sight; click, Ctrl, Shift, a rubber band and the arrow keys select.
 */
export function ItemsView({
  viewId,
  total,
  mode,
  cellSize,
  showNames,
  selection,
  onSelectionChange,
  onOpen,
  onEnter,
  onZoom,
  onLost,
  transitionId,
}: Props) {
  const client = useQueryClient();
  const scrollRef = useRef<HTMLDivElement>(null);
  const width = useWidth(scrollRef);
  const layout: Layout =
    mode === "grid" ? gridLayout(width, cellSize, showNames) : listLayout(width);
  const rows = Math.ceil(total / layout.columns);
  const virtualizer = useVirtualizer({
    count: rows,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => layout.rowHeight,
    overscan: 3,
    paddingStart: PADDING,
    paddingEnd: PADDING,
  });

  // Keep the item at the top in place when the cell size or the column count changes: find it
  // with the previous layout, then scroll to its row in the new one.
  const previous = useRef(layout);
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when the layout changes.
  useLayoutEffect(() => {
    const before = previous.current;
    previous.current = layout;
    virtualizer.measure();
    const element = scrollRef.current;
    if (!element || element.scrollTop <= PADDING) return;
    const row = Math.floor((element.scrollTop - PADDING) / before.rowHeight);
    const top = row * before.columns;
    element.scrollTop = PADDING + Math.floor(top / layout.columns) * layout.rowHeight;
  }, [layout.columns, layout.rowHeight]);

  const shownRows = virtualizer.getVirtualItems();
  const firstIndex = (shownRows[0]?.index ?? 0) * layout.columns;
  const lastIndex = Math.min(
    total - 1,
    ((shownRows[shownRows.length - 1]?.index ?? -1) + 1) * layout.columns - 1,
  );
  const cellAt = useCells(viewId, firstIndex, lastIndex, onLost);

  // The selection as of the latest render, for answers that arrive later.
  const latest = useRef(selection);
  latest.current = selection;

  const idAt = useCallback(
    async (index: number) =>
      cellAt(index)?.id ?? (await fetchCell(client, viewId, index).catch(() => undefined))?.id,
    [cellAt, client, viewId],
  );

  const select = useCallback(
    async (index: number, { ctrl, shift }: { ctrl: boolean; shift: boolean }) => {
      if (shift) {
        const [start, len] = rangeFrom(latest.current, index);
        const ids = await ipc.viewIds(viewId, start, len).catch(() => null);
        if (ids) onSelectionChange(withRange(latest.current, index, ids, ctrl));
        return;
      }
      const id = await idAt(index);
      if (id === undefined) return;
      onSelectionChange(ctrl ? toggle(latest.current, index, id) : only(index, id));
    },
    [idAt, onSelectionChange, viewId],
  );

  // The rubber band, in content coordinates; its cells show as selected while it is drawn.
  const [band, setBand] = useState<Band | null>(null);
  const bandIndices = band ? new Set(indicesInRect(layout, total, bandRect(band))) : null;

  const contentPoint = (event: { clientX: number; clientY: number }) => {
    const element = scrollRef.current;
    const box = element?.getBoundingClientRect();
    return {
      x: event.clientX - (box?.left ?? 0) + (element?.scrollLeft ?? 0),
      y: event.clientY - (box?.top ?? 0) + (element?.scrollTop ?? 0),
    };
  };

  const startBand = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest("[data-index]")) return;
    // Clicks on the scrollbar do not start a band.
    const element = scrollRef.current;
    if (element && event.clientX - element.getBoundingClientRect().left > element.clientWidth) {
      return;
    }
    event.preventDefault();
    element?.focus({ preventScroll: true });
    const point = contentPoint(event);
    let current: Band = { start: point, end: point, additive: event.ctrlKey || event.metaKey };
    setBand(current);
    const onMove = (move: MouseEvent) => {
      const box = element?.getBoundingClientRect();
      if (element && box) {
        if (move.clientY < box.top + 24) element.scrollTop -= 24;
        else if (move.clientY > box.bottom - 24) element.scrollTop += 24;
      }
      current = { ...current, end: contentPoint(move) };
      setBand(current);
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      setBand(null);
      const picked = indicesInRect(layout, total, bandRect(current));
      const moved =
        Math.abs(current.end.x - current.start.x) + Math.abs(current.end.y - current.start.y) > 3;
      if (!moved || picked.length === 0) {
        if (!current.additive) onSelectionChange(NOTHING);
        return;
      }
      const start = picked[0] ?? 0;
      const end = picked[picked.length - 1] ?? start;
      const inBand = new Set(picked);
      void ipc
        .viewIds(viewId, start, end - start + 1)
        .then((ids) => {
          const base = current.additive ? latest.current : NOTHING;
          const chosen = ids.filter((_, offset) => inBand.has(start + offset));
          const next = withRange({ ...base, anchor: start }, end, chosen, current.additive);
          onSelectionChange(next);
        })
        .catch(() => {});
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  // Cells get stable handlers, so scrolling re-renders only the cells that change.
  const handlers = useRef({ select, onOpen, cellAt });
  handlers.current = { select, onOpen, cellAt };
  const onCellMouseDown = useCallback((event: ReactMouseEvent, index: number) => {
    if (event.button !== 0) return;
    event.preventDefault();
    scrollRef.current?.focus({ preventScroll: true });
    void handlers.current.select(index, {
      ctrl: event.ctrlKey || event.metaKey,
      shift: event.shiftKey,
    });
  }, []);
  const open = useCallback((index: number) => {
    const { onOpen, cellAt } = handlers.current;
    onOpen(index, cellAt(index)?.id ?? null);
  }, []);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      onEnter();
      return;
    }
    const pageRows = Math.max(
      1,
      Math.floor((scrollRef.current?.clientHeight ?? 0) / layout.rowHeight),
    );
    const target = moveFocus(event.key, selection.focus, total, layout.columns, pageRows);
    if (target === null) return;
    event.preventDefault();
    virtualizer.scrollToIndex(Math.floor(target / layout.columns));
    void select(target, { ctrl: false, shift: event.shiftKey });
  };

  const shown = (cell: ItemCell | undefined, index: number) => {
    if (bandIndices) {
      return (
        bandIndices.has(index) ||
        (band?.additive === true && cell !== undefined && isSelected(selection, cell.id))
      );
    }
    return cell !== undefined && isSelected(selection, cell.id);
  };

  const focus = selection.focus;
  const focusShown = focus !== null && focus >= firstIndex && focus <= lastIndex;

  const grid = (
    // biome-ignore lint/a11y/useSemanticElements: a virtualized grid of positioned rows is no table.
    <div
      ref={scrollRef}
      role="grid"
      aria-label="Items"
      aria-multiselectable
      aria-rowcount={rows}
      aria-colcount={layout.columns}
      aria-activedescendant={focusShown ? cellDomId(focus) : undefined}
      tabIndex={0}
      className="relative min-h-0 flex-1 overflow-auto bg-grid outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
      onMouseDown={startBand}
      onKeyDown={onKeyDown}
      onWheel={(event) => {
        if (event.ctrlKey) onZoom(event.deltaY < 0 ? 1 : -1);
      }}
    >
      <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
        {shownRows.map((row) => {
          const cells: React.ReactNode[] = [];
          for (let column = 0; column < layout.columns; column += 1) {
            const index = row.index * layout.columns + column;
            if (index >= total) break;
            const cell = cellAt(index);
            const props = {
              index,
              cell,
              selected: shown(cell, index),
              focused: focus === index,
              transition: cell !== undefined && cell.id === transitionId,
              onMouseDown: onCellMouseDown,
              onOpen: open,
            };
            cells.push(
              mode === "grid" ? (
                <GridCell
                  key={index}
                  {...props}
                  size={layout.cell}
                  height={layout.cellHeight}
                  showNames={showNames}
                />
              ) : (
                <ListRow key={index} {...props} />
              ),
            );
          }
          return (
            // biome-ignore lint/a11y: a row of a virtualized grid; the grid holds focus.
            <div
              key={row.key}
              role="row"
              aria-rowindex={row.index + 1}
              className="absolute top-0 left-0 flex w-full"
              style={{
                transform: `translateY(${row.start}px)`,
                gap: mode === "grid" ? "8px" : undefined,
                paddingInline: PADDING,
              }}
            >
              {cells}
            </div>
          );
        })}
        {band && (
          <div
            aria-hidden
            className="pointer-events-none absolute border border-primary bg-primary/10"
            style={rectStyle(bandRect(band))}
          />
        )}
      </div>
    </div>
  );
  if (mode === "grid") return grid;
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <ListHeader />
      {grid}
    </div>
  );
}

function rectStyle(rect: Rect): CSSProperties {
  return {
    left: rect.left,
    top: rect.top,
    width: rect.right - rect.left,
    height: rect.bottom - rect.top,
  };
}

type CellProps = {
  index: number;
  cell: ItemCell | undefined;
  selected: boolean;
  focused: boolean;
  transition: boolean;
  onMouseDown: (event: ReactMouseEvent, index: number) => void;
  onOpen: (index: number) => void;
};

const GridCell = memo(function GridCell({
  index,
  cell,
  selected,
  focused,
  transition,
  onMouseDown,
  onOpen,
  size,
  height,
  showNames,
}: CellProps & { size: number; height: number; showNames: boolean }) {
  return (
    // biome-ignore lint/a11y: a cell of a virtualized grid that holds focus (aria-activedescendant).
    <div
      role="gridcell"
      id={cellDomId(index)}
      data-index={index}
      aria-selected={selected}
      aria-label={cell?.name ?? "Loading"}
      className="flex shrink-0 flex-col"
      style={{ width: size, height }}
      onMouseDown={(event) => onMouseDown(event, index)}
      onDoubleClick={() => onOpen(index)}
    >
      <div
        className={cn(
          "relative grid shrink-0 place-items-center overflow-hidden rounded-md bg-muted/60",
          selected && "outline-2 outline-offset-2 outline-primary",
          focused && !selected && "outline-1 outline-offset-2 outline-ring",
        )}
        style={{ width: size, height: size }}
      >
        {cell && (
          <Thumb key={`${cell.id}:${cell.thumbVersion}`} cell={cell} transition={transition} />
        )}
        {cell && cell.star > 0 && (
          <span className="absolute bottom-1 left-1 rounded bg-black/60 px-1 text-[11px] leading-4 text-amber-300">
            {"★".repeat(cell.star)}
          </span>
        )}
      </div>
      {showNames && (
        <span className="truncate px-1 pt-1 text-center text-xs" title={cell?.name}>
          {cell?.name ?? " "}
        </span>
      )}
    </div>
  );
});

const ListRow = memo(function ListRow({
  index,
  cell,
  selected,
  focused,
  transition,
  onMouseDown,
  onOpen,
}: CellProps) {
  return (
    // biome-ignore lint/a11y: a cell of a virtualized grid that holds focus (aria-activedescendant).
    <div
      role="gridcell"
      id={cellDomId(index)}
      data-index={index}
      aria-selected={selected}
      aria-label={cell?.name ?? "Loading"}
      className={cn(
        "grid h-8 w-full grid-cols-[28px_minmax(0,3fr)_56px_72px_96px_150px_64px_minmax(0,2fr)] items-center gap-3 rounded-md px-2 text-[13px]",
        selected ? "bg-primary/15" : "hover:bg-muted/60",
        focused && "outline-1 outline-ring",
      )}
      onMouseDown={(event) => onMouseDown(event, index)}
      onDoubleClick={() => onOpen(index)}
    >
      <div className="relative grid size-6 place-items-center overflow-hidden rounded bg-muted/60">
        {cell && (
          <Thumb
            key={`${cell.id}:${cell.thumbVersion}`}
            cell={cell}
            transition={transition}
            small
          />
        )}
      </div>
      <span className="truncate">{cell?.name}</span>
      <span className="uppercase text-muted-foreground">{cell?.ext}</span>
      <span className="text-muted-foreground tabular-nums">
        {cell ? formatBytes(cell.size) : ""}
      </span>
      <span className="text-muted-foreground tabular-nums">
        {cell ? formatDimensions(cell.width, cell.height) : ""}
      </span>
      <span className="text-muted-foreground">{cell ? formatDate(cell.addedAt) : ""}</span>
      <span className="text-amber-500">{cell && cell.star > 0 ? "★".repeat(cell.star) : ""}</span>
      <span className="truncate text-muted-foreground">{cell?.tags.join(", ")}</span>
    </div>
  );
});

/** Column titles over the list; the rows' cells carry the names for assistive technology. */
function ListHeader() {
  return (
    <div
      aria-hidden
      className="grid h-8 shrink-0 grid-cols-[28px_minmax(0,3fr)_56px_72px_96px_150px_64px_minmax(0,2fr)] items-center gap-3 border-b bg-background px-5 text-xs font-medium text-muted-foreground"
    >
      <span />
      <span>Name</span>
      <span>Type</span>
      <span>Size</span>
      <span>Dimensions</span>
      <span>Date added</span>
      <span>Rating</span>
      <span>Tags</span>
    </div>
  );
}

/** Black or white text, whichever reads better on the palette colour. */
function textOn([r, g, b]: [number, number, number]): string {
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#000" : "#fff";
}

/**
 * The thumbnail, faded in once loaded; until then, or without one, the first palette colour
 * and the extension.
 */
function Thumb({
  cell,
  transition,
  small = false,
}: {
  cell: ItemCell;
  transition: boolean;
  small?: boolean;
}) {
  const [state, setState] = useState<"loading" | "loaded" | "failed">(
    cell.hasThumbnail ? "loading" : "failed",
  );
  const palette = cell.palette0;
  return (
    <>
      {state !== "loaded" && (
        <div
          data-testid="placeholder"
          className={cn(
            "absolute inset-0 grid place-items-center font-medium uppercase",
            small ? "text-[8px]" : "text-xs",
            !palette && "text-muted-foreground",
          )}
          style={
            palette
              ? { background: `rgb(${palette.join(" ")})`, color: textOn(palette) }
              : undefined
          }
        >
          {cell.ext}
        </div>
      )}
      {cell.hasThumbnail && state !== "failed" && (
        <img
          src={thumbUrl(cell)}
          alt=""
          draggable={false}
          decoding="async"
          onLoad={() => setState("loaded")}
          onError={() => setState("failed")}
          className={cn(
            "relative max-h-full max-w-full object-contain motion-safe:transition-opacity motion-safe:duration-150",
            state === "loaded" ? "opacity-100" : "opacity-0",
          )}
          style={transition ? { viewTransitionName: PREVIEW_TRANSITION } : undefined}
        />
      )}
    </>
  );
}
