import { useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "cn";
import {
  type CSSProperties,
  memo,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu";
import { type DragPayload, dropHint, useDrag } from "@/features/dnd/drag";
import { type ItemCell, ipc, type ViewMode } from "@/ipc";
import { thumbUrl } from "@/lib/thumb";
import { formatBytes, formatDate, formatDimensions } from "./format";
import { indicesInRect, type Layout, listLayout, moveFocus, PADDING, type Rect } from "./geometry";
import {
  itemAtScrollTop,
  type JustifiedLayout,
  justifiedIndicesInRect,
  moveFocusJustified,
  packJustified,
} from "./justified";
import {
  isSelected,
  NOTHING,
  only,
  rangeFrom,
  type Selection,
  selectedCount,
  toggle,
  withRange,
} from "./selection";
import { fetchCell, useCells, useViewDimensions } from "./useView";

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
  /** Selected items can be dragged onto folders (a writable library). */
  draggable?: boolean;
  /** Items can be dropped between cells: a folder in manual order. */
  reorderable?: boolean;
  /** The Trash: the list shows when each item went there. */
  inTrash?: boolean;
  /** The items of the right-click menu; a right click selects the item under it first. */
  menu?: ReactNode;
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
  draggable = false,
  reorderable = false,
  inTrash = false,
  menu,
}: Props) {
  const client = useQueryClient();
  const { begin, active } = useDrag();
  const scrollRef = useRef<HTMLDivElement>(null);
  const width = useWidth(scrollRef);
  const dimensions = useViewDimensions(mode === "grid" ? viewId : null);
  const justified: JustifiedLayout | null = useMemo(() => {
    if (mode !== "grid" || width <= 0 || total <= 0) return null;
    return packJustified(total, width, cellSize, showNames, (index) => {
      const pair = dimensions?.[index];
      return { width: pair?.[0] ?? 0, height: pair?.[1] ?? 0 };
    });
  }, [mode, width, total, cellSize, showNames, dimensions]);
  const list: Layout | null = mode === "list" ? listLayout(width) : null;
  const rowCount = justified?.rows.length ?? (list ? Math.ceil(total / list.columns) : 0);
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) => justified?.rows[index]?.stride ?? list?.rowHeight ?? cellSize,
    overscan: 3,
    paddingStart: PADDING,
    paddingEnd: PADDING,
  });

  // Keep the item at the top in place when the row geometry changes.
  const previousJustified = useRef(justified);
  const previousList = useRef(list);
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when the layout changes.
  useLayoutEffect(() => {
    const beforeJustified = previousJustified.current;
    const beforeList = previousList.current;
    previousJustified.current = justified;
    previousList.current = list;
    virtualizer.measure();
    const element = scrollRef.current;
    if (!element || element.scrollTop <= PADDING) return;
    if (justified && beforeJustified) {
      const item = itemAtScrollTop(beforeJustified, element.scrollTop);
      const row = justified.rowOf[item] ?? 0;
      element.scrollTop = PADDING + (justified.rows[row]?.top ?? 0);
      return;
    }
    if (list && beforeList) {
      const row = Math.floor((element.scrollTop - PADDING) / beforeList.rowHeight);
      const top = row * beforeList.columns;
      element.scrollTop = PADDING + Math.floor(top / list.columns) * list.rowHeight;
    }
  }, [justified, list?.columns, list?.rowHeight, cellSize, showNames]);

  const shownRows = virtualizer.getVirtualItems();
  const firstIndex = justified
    ? ((shownRows[0] ? justified.rows[shownRows[0].index]?.startIndex : 0) ?? 0)
    : (shownRows[0]?.index ?? 0) * (list?.columns ?? 1);
  const lastShown = shownRows[shownRows.length - 1];
  const lastIndex = justified
    ? lastShown
      ? (() => {
          const row = justified.rows[lastShown.index];
          const last = row?.cells[row.cells.length - 1];
          return last ? last.index : -1;
        })()
      : -1
    : Math.min(total - 1, ((lastShown?.index ?? -1) + 1) * (list?.columns ?? 1) - 1);
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
  const bandIndices = band
    ? new Set(
        justified
          ? justifiedIndicesInRect(justified, total, bandRect(band))
          : list
            ? indicesInRect(list, total, bandRect(band))
            : [],
      )
    : null;

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
      const picked = justified
        ? justifiedIndicesInRect(justified, total, bandRect(current))
        : list
          ? indicesInRect(list, total, bandRect(current))
          : [];
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

  /** What a drag from the cell at `index` carries: the selection, or the cell alone. */
  const payload = (index: number): DragPayload | null => {
    const cell = cellAt(index);
    if (!cell) return null;
    const current = latest.current;
    if (!isSelected(current, cell.id)) {
      const target = { viewId, all: false, ids: [cell.id] };
      return { kind: "items", target, count: 1, label: cell.name };
    }
    const count = selectedCount(current, total);
    return {
      kind: "items",
      target: { viewId, all: current.all, ids: [...current.ids] },
      count,
      label: count === 1 ? cell.name : `${count.toLocaleString("en")} items`,
    };
  };

  // Cells get stable handlers, so scrolling re-renders only the cells that change.
  const handlers = useRef({ select, onOpen, cellAt, payload, draggable });
  handlers.current = { select, onOpen, cellAt, payload, draggable };
  const onCellMouseDown = useCallback(
    (event: ReactMouseEvent, index: number) => {
      if (event.button !== 0) return;
      event.preventDefault();
      scrollRef.current?.focus({ preventScroll: true });
      const ctrl = event.ctrlKey || event.metaKey;
      const shift = event.shiftKey;
      const { select, cellAt, payload, draggable } = handlers.current;
      const cell = cellAt(index);
      // A press on a selected item keeps the selection, so all of it can be dragged; a click
      // without a drag then selects that item alone.
      const keep =
        draggable && !ctrl && !shift && cell !== undefined && isSelected(latest.current, cell.id);
      if (!keep) void select(index, { ctrl, shift });
      if (!draggable || ctrl || shift) return;
      begin(
        event,
        () => payload(index),
        keep ? () => void handlers.current.select(index, { ctrl: false, shift: false }) : undefined,
      );
    },
    [begin],
  );
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
    const viewport = scrollRef.current?.clientHeight ?? 0;
    let target: number | null;
    let scrollRow: number;
    if (justified) {
      const avg = justified.rows[0]?.stride ?? cellSize;
      const pageRows = Math.max(1, Math.floor(viewport / avg));
      target = moveFocusJustified(event.key, selection.focus, justified, pageRows);
      scrollRow = target === null ? 0 : (justified.rowOf[target] ?? 0);
    } else if (list) {
      const pageRows = Math.max(1, Math.floor(viewport / list.rowHeight));
      target = moveFocus(event.key, selection.focus, total, list.columns, pageRows);
      scrollRow = target === null ? 0 : Math.floor(target / list.columns);
    } else {
      return;
    }
    if (target === null) return;
    event.preventDefault();
    virtualizer.scrollToIndex(scrollRow);
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
  const dropAt = dropHint(active, (target) => target.kind === "cell");
  const dropCell = dropAt?.kind === "cell" ? dropAt : null;

  // Dragging items near the top or bottom edge scrolls, so a far position can be reached.
  const pointerY = active?.payload.kind === "items" ? active.y : null;
  useEffect(() => {
    const element = scrollRef.current;
    if (pointerY === null || !reorderable || !element) return;
    const box = element.getBoundingClientRect();
    if (pointerY < box.top + 32) element.scrollTop -= 24;
    else if (pointerY > box.bottom - 32) element.scrollTop += 24;
  }, [pointerY, reorderable]);

  const colCount = justified?.maxColumns ?? list?.columns ?? 1;
  const grid = (
    // biome-ignore lint/a11y/useSemanticElements: a virtualized grid of positioned rows is no table.
    <div
      ref={scrollRef}
      role="grid"
      aria-label="Items"
      aria-multiselectable
      aria-rowcount={rowCount}
      aria-colcount={colCount}
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
          if (justified) {
            const justifiedRow = justified.rows[row.index];
            if (!justifiedRow) return null;
            for (const entry of justifiedRow.cells) {
              const cell = cellAt(entry.index);
              cells.push(
                <GridCell
                  key={entry.index}
                  index={entry.index}
                  cell={cell}
                  selected={shown(cell, entry.index)}
                  focused={focus === entry.index}
                  transition={cell !== undefined && cell.id === transitionId}
                  reorderable={reorderable}
                  dropSide={dropCell?.index === entry.index ? dropCell.side : null}
                  inTrash={inTrash}
                  onMouseDown={onCellMouseDown}
                  onOpen={open}
                  thumbWidth={entry.width}
                  thumbHeight={entry.height}
                  height={justifiedRow.contentHeight}
                  showNames={showNames}
                />,
              );
            }
          } else if (list) {
            for (let column = 0; column < list.columns; column += 1) {
              const index = row.index * list.columns + column;
              if (index >= total) break;
              const cell = cellAt(index);
              cells.push(
                <ListRow
                  key={index}
                  index={index}
                  cell={cell}
                  selected={shown(cell, index)}
                  focused={focus === index}
                  transition={cell !== undefined && cell.id === transitionId}
                  reorderable={reorderable}
                  dropSide={dropCell?.index === index ? dropCell.side : null}
                  inTrash={inTrash}
                  onMouseDown={onCellMouseDown}
                  onOpen={open}
                />,
              );
            }
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
                height: justified?.rows[row.index]?.contentHeight,
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
  const withMenu = menu ? (
    <ContextMenu>
      <ContextMenuTrigger
        asChild
        onContextMenu={(event) => {
          const target = (event.target as HTMLElement).closest<HTMLElement>("[data-index]");
          // Empty space has no menu.
          if (!target) {
            event.preventDefault();
            return;
          }
          const index = Number(target.dataset.index);
          const cell = cellAt(index);
          if (!cell || !isSelected(latest.current, cell.id)) {
            void select(index, { ctrl: false, shift: false });
          }
        }}
      >
        {grid}
      </ContextMenuTrigger>
      <ContextMenuContent>{menu}</ContextMenuContent>
    </ContextMenu>
  ) : (
    grid
  );
  // One place in the tree for both modes: a remounted scroll container would leave the width
  // observer on the old, detached element, which measures 0.
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {mode === "list" && <ListHeader inTrash={inTrash} />}
      {withMenu}
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
  /** The cell is a drop target of a manual order. */
  reorderable: boolean;
  /** Where dragged items would land next to this cell. */
  dropSide: "before" | "after" | null;
  /** The cell is in the Trash: it says since when. */
  inTrash: boolean;
  onMouseDown: (event: ReactMouseEvent, index: number) => void;
  onOpen: (index: number) => void;
};

const GridCell = memo(function GridCell({
  index,
  cell,
  selected,
  focused,
  transition,
  reorderable,
  dropSide,
  inTrash,
  onMouseDown,
  onOpen,
  thumbWidth,
  thumbHeight,
  height,
  showNames,
}: CellProps & {
  thumbWidth: number;
  thumbHeight: number;
  height: number;
  showNames: boolean;
}) {
  return (
    // biome-ignore lint/a11y: a cell of a virtualized grid that holds focus (aria-activedescendant).
    <div
      role="gridcell"
      id={cellDomId(index)}
      data-index={index}
      data-drop={reorderable && cell ? "cell" : undefined}
      data-id={cell?.id}
      aria-selected={selected}
      aria-label={cell?.name ?? "Loading"}
      title={
        inTrash && cell?.deletedAt ? `In the trash since ${formatDate(cell.deletedAt)}` : undefined
      }
      className="relative flex shrink-0 flex-col"
      style={{ width: thumbWidth, height }}
      onMouseDown={(event) => onMouseDown(event, index)}
      onDoubleClick={() => onOpen(index)}
    >
      {dropSide && (
        <span
          aria-hidden
          className={cn(
            "absolute inset-y-0 w-0.5 rounded bg-primary",
            dropSide === "before" ? "-left-[5px]" : "-right-[5px]",
          )}
        />
      )}
      <div
        className={cn(
          "relative grid shrink-0 place-items-center overflow-hidden rounded-md bg-muted/60",
          selected && "outline-2 outline-offset-2 outline-primary",
          focused && !selected && "outline-1 outline-offset-2 outline-ring",
        )}
        style={{ width: thumbWidth, height: thumbHeight }}
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
  reorderable,
  dropSide,
  inTrash,
  onMouseDown,
  onOpen,
}: CellProps) {
  return (
    // biome-ignore lint/a11y: a cell of a virtualized grid that holds focus (aria-activedescendant).
    <div
      role="gridcell"
      id={cellDomId(index)}
      data-index={index}
      data-drop={reorderable && cell ? "cell" : undefined}
      data-id={cell?.id}
      data-axis="y"
      aria-selected={selected}
      aria-label={cell?.name ?? "Loading"}
      className={cn(
        "relative grid h-8 w-full grid-cols-[28px_minmax(0,3fr)_56px_72px_96px_150px_64px_minmax(0,2fr)] items-center gap-3 rounded-md px-2 text-[13px]",
        selected ? "bg-primary/15" : "hover:bg-muted/60",
        focused && "outline-1 outline-ring",
      )}
      onMouseDown={(event) => onMouseDown(event, index)}
      onDoubleClick={() => onOpen(index)}
    >
      {dropSide && (
        <span
          aria-hidden
          className={cn(
            "absolute inset-x-0 h-0.5 rounded bg-primary",
            dropSide === "before" ? "-top-px" : "-bottom-px",
          )}
        />
      )}
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
      <span className="text-muted-foreground">
        {cell ? formatDate(inTrash ? cell.deletedAt : cell.addedAt) : ""}
      </span>
      <span className="text-amber-500">{cell && cell.star > 0 ? "★".repeat(cell.star) : ""}</span>
      <span className="truncate text-muted-foreground">{cell?.tags.join(", ")}</span>
    </div>
  );
});

/** Column titles over the list; the rows' cells carry the names for assistive technology. */
function ListHeader({ inTrash }: { inTrash: boolean }) {
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
      <span>{inTrash ? "Date deleted" : "Date added"}</span>
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
            "relative motion-safe:transition-opacity motion-safe:duration-150",
            small ? "max-h-full max-w-full object-contain" : "h-full w-full object-cover",
            state === "loaded" ? "opacity-100" : "opacity-0",
          )}
          style={transition ? { viewTransitionName: PREVIEW_TRANSITION } : undefined}
        />
      )}
    </>
  );
}
