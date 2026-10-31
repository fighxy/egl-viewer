// List geometry and the legacy uniform-square helpers (still used by list mode and tests).
// The grid packs justified rows in `justified.ts` (D-067, docs/spec.md §7.3).

export const PADDING = 12;
export const GAP = 8;
/** The name line under a cell when View → Show Names is on. */
export const NAME_HEIGHT = 20;
export const LIST_ROW = 32;

export type Layout = {
  columns: number;
  /** Width and height of a cell's square in the grid; the row width in the list. */
  cell: number;
  /** Row height including the gap below it. */
  rowHeight: number;
  /** Height of the content box of a cell: the square plus the name, or the list row. */
  cellHeight: number;
};

/** As many cells of about `size` px as fit, stretched to fill the width. */
export function gridLayout(width: number, size: number, showNames: boolean): Layout {
  const available = Math.max(0, width - 2 * PADDING);
  const columns = Math.max(1, Math.floor((available + GAP) / (size + GAP)));
  const cell = Math.max(1, Math.floor((available - (columns - 1) * GAP) / columns));
  const cellHeight = cell + (showNames ? NAME_HEIGHT : 0);
  return { columns, cell, cellHeight, rowHeight: cellHeight + GAP };
}

export function listLayout(width: number): Layout {
  return {
    columns: 1,
    cell: Math.max(1, width - 2 * PADDING),
    cellHeight: LIST_ROW,
    rowHeight: LIST_ROW,
  };
}

export type Rect = { left: number; top: number; right: number; bottom: number };

/** The box of the cell at `index`, in content coordinates (scroll included). */
export function cellRect(layout: Layout, index: number): Rect {
  const row = Math.floor(index / layout.columns);
  const column = index % layout.columns;
  const left = PADDING + column * (layout.cell + GAP);
  const top = PADDING + row * layout.rowHeight;
  return { left, top, right: left + layout.cell, bottom: top + layout.cellHeight };
}

/** The positions whose cells the rectangle touches. */
export function indicesInRect(layout: Layout, total: number, rect: Rect): number[] {
  const stride = layout.cell + GAP;
  const firstRow = Math.max(0, Math.floor((rect.top - PADDING) / layout.rowHeight));
  const lastRow = Math.floor((rect.bottom - PADDING) / layout.rowHeight);
  const firstColumn = Math.max(0, Math.floor((rect.left - PADDING) / stride));
  const lastColumn = Math.min(layout.columns - 1, Math.floor((rect.right - PADDING) / stride));
  const found: number[] = [];
  for (let row = firstRow; row <= lastRow; row += 1) {
    for (let column = firstColumn; column <= lastColumn; column += 1) {
      const index = row * layout.columns + column;
      if (index >= total) break;
      const box = cellRect(layout, index);
      const touches =
        box.left <= rect.right &&
        box.right >= rect.left &&
        box.top <= rect.bottom &&
        box.bottom >= rect.top;
      if (touches) found.push(index);
    }
  }
  return found;
}

/** The position a keyboard key moves to, or `null` for keys the grid does not use. */
export function moveFocus(
  key: string,
  focus: number | null,
  total: number,
  columns: number,
  pageRows: number,
): number | null {
  if (total === 0) return null;
  const from = focus ?? -1;
  let target: number;
  switch (key) {
    case "ArrowRight":
      target = columns === 1 ? from : from + 1;
      break;
    case "ArrowLeft":
      target = columns === 1 ? from : from - 1;
      break;
    case "ArrowDown":
      target = from < 0 ? 0 : from + columns;
      break;
    case "ArrowUp":
      target = from - columns;
      break;
    case "Home":
      target = 0;
      break;
    case "End":
      target = total - 1;
      break;
    case "PageDown":
      target = from < 0 ? 0 : from + pageRows * columns;
      break;
    case "PageUp":
      target = from - pageRows * columns;
      break;
    default:
      return null;
  }
  return Math.min(Math.max(target, 0), total - 1);
}
