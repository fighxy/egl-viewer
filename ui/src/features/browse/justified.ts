// Justified rows: equal height per row, cell widths from aspect ratios so the row fills the
// container (Flickr / Google Photos). Uses item width/height from the view; missing sizes are 1:1.

import { GAP, NAME_HEIGHT, PADDING, type Rect } from "./geometry";

/** Pixel size of one item; both zero means unknown → square. */
export type CellDimensions = { width: number; height: number };

export type JustifiedCell = {
  index: number;
  /** Content box width (thumbnail). */
  width: number;
  /** Thumbnail height (same for every cell in the row). */
  height: number;
  left: number;
};

export type JustifiedRow = {
  /** First item index in this row. */
  startIndex: number;
  cells: JustifiedCell[];
  /** Thumbnail row height before the optional name line. */
  thumbHeight: number;
  /** Full row content height (thumb + name). */
  contentHeight: number;
  /** Distance from one row top to the next (content + gap). */
  stride: number;
  /** Top of the row content in scroll coordinates (padding excluded; virtualizer adds it). */
  top: number;
};

export type JustifiedLayout = {
  rows: JustifiedRow[];
  /** Total content height including bottom gap after the last row, excluding outer padding. */
  totalHeight: number;
  /** Widest row's cell count (for aria-colcount). */
  maxColumns: number;
  /** Index → row index. */
  rowOf: number[];
};

/** Aspect ratio from stored pixels; missing or zero → 1 (square). */
export function aspectOf(width: number, height: number): number {
  if (width > 0 && height > 0) return width / height;
  return 1;
}

/**
 * Packs `total` items into justified rows. `sizeAt` returns stored width/height (zeros = unknown).
 * `targetHeight` is the thumbnail size slider value (row height before stretch).
 */
export function packJustified(
  total: number,
  containerWidth: number,
  targetHeight: number,
  showNames: boolean,
  sizeAt: (index: number) => CellDimensions,
): JustifiedLayout {
  const available = Math.max(1, containerWidth - 2 * PADDING);
  const target = Math.max(1, targetHeight);
  const caption = showNames ? NAME_HEIGHT : 0;
  const rows: JustifiedRow[] = [];
  const rowOf: number[] = new Array(total);
  let index = 0;
  let top = 0;
  let maxColumns = 1;

  while (index < total) {
    const aspects: { index: number; aspect: number }[] = [];
    let sumAspect = 0;
    while (index < total) {
      const size = sizeAt(index);
      const aspect = aspectOf(size.width, size.height);
      const gaps = aspects.length * GAP;
      const widthIfAdded = (sumAspect + aspect) * target + gaps;
      if (aspects.length > 0 && widthIfAdded > available) break;
      aspects.push({ index, aspect });
      sumAspect += aspect;
      index += 1;
    }
    if (aspects.length === 0) break;

    const gaps = Math.max(0, aspects.length - 1) * GAP;
    const isLast = index >= total;
    const naturalWidth = sumAspect * target + gaps;
    // Last row stays near the target height unless it nearly fills the width (avoids a
    // stretched strip of one or two leftovers).
    let thumbHeight =
      isLast && naturalWidth / available < 0.7
        ? target
        : Math.max(1, (available - gaps) / sumAspect);
    // Keep the slider meaningful: do not shrink or grow more than 50%.
    thumbHeight = Math.min(target * 1.5, Math.max(target * 0.5, thumbHeight));

    const contentHeight = thumbHeight + caption;
    const cells: JustifiedCell[] = [];
    let left = 0;
    let used = 0;
    for (let i = 0; i < aspects.length; i += 1) {
      const entry = aspects[i];
      if (!entry) continue;
      const width =
        i === aspects.length - 1 && !(isLast && naturalWidth / available < 0.7)
          ? Math.max(1, available - gaps - used)
          : Math.max(1, entry.aspect * thumbHeight);
      if (i < aspects.length - 1) used += width;
      cells.push({
        index: entry.index,
        width,
        height: thumbHeight,
        left,
      });
      rowOf[entry.index] = rows.length;
      left += width + GAP;
    }

    const first = cells[0];
    if (!first) continue;
    maxColumns = Math.max(maxColumns, cells.length);
    rows.push({
      startIndex: first.index,
      cells,
      thumbHeight,
      contentHeight,
      stride: contentHeight + GAP,
      top,
    });
    top += contentHeight + GAP;
  }

  return {
    rows,
    totalHeight: rows.length === 0 ? 0 : top - GAP,
    maxColumns,
    rowOf,
  };
}

/** Content-coordinate box of the cell at `index` (padding included on the left/top). */
export function justifiedCellRect(layout: JustifiedLayout, index: number): Rect | null {
  const rowIndex = layout.rowOf[index];
  if (rowIndex === undefined) return null;
  const row = layout.rows[rowIndex];
  if (!row) return null;
  const cell = row.cells.find((entry) => entry.index === index);
  if (!cell) return null;
  const left = PADDING + cell.left;
  const top = PADDING + row.top;
  return {
    left,
    top,
    right: left + cell.width,
    bottom: top + row.contentHeight,
  };
}

/** Positions whose cells the rectangle touches. */
export function justifiedIndicesInRect(
  layout: JustifiedLayout,
  total: number,
  rect: Rect,
): number[] {
  const found: number[] = [];
  for (const row of layout.rows) {
    const rowTop = PADDING + row.top;
    const rowBottom = rowTop + row.contentHeight;
    if (rowBottom < rect.top || rowTop > rect.bottom) continue;
    for (const cell of row.cells) {
      if (cell.index >= total) break;
      const left = PADDING + cell.left;
      const right = left + cell.width;
      if (right < rect.left || left > rect.right) continue;
      if (
        rowBottom >= rect.top &&
        rowTop <= rect.bottom &&
        right >= rect.left &&
        left <= rect.right
      ) {
        found.push(cell.index);
      }
    }
  }
  return found;
}

/** Nearest cell in `row` by horizontal centre to `fromX`. */
function nearestInRow(row: JustifiedRow, fromX: number): number {
  const first = row.cells[0];
  if (!first) return 0;
  let best = first.index;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const cell of row.cells) {
    const centre = cell.left + cell.width / 2;
    const dist = Math.abs(centre - fromX);
    if (dist < bestDist) {
      bestDist = dist;
      best = cell.index;
    }
  }
  return best;
}

/**
 * Keyboard move on a justified grid. Down/Up pick the nearest cell by X in the next/previous
 * row; Left/Right walk item indices; Page keys jump by about `pageRows` rows.
 */
export function moveFocusJustified(
  key: string,
  focus: number | null,
  layout: JustifiedLayout,
  pageRows: number,
): number | null {
  const total = layout.rowOf.length;
  if (total === 0) return null;
  const from = focus ?? -1;

  const rowIndexOf = (index: number) => layout.rowOf[index] ?? 0;
  const xOf = (index: number): number => {
    const row = layout.rows[rowIndexOf(index)];
    const cell = row?.cells.find((entry) => entry.index === index);
    return cell ? cell.left + cell.width / 2 : 0;
  };

  let target: number;
  switch (key) {
    case "ArrowRight":
      target = from < 0 ? 0 : from + 1;
      break;
    case "ArrowLeft":
      target = from < 0 ? 0 : from - 1;
      break;
    case "ArrowDown": {
      if (from < 0) {
        target = 0;
        break;
      }
      const row = rowIndexOf(from);
      const next = layout.rows[row + 1];
      if (!next) {
        target = total - 1;
        break;
      }
      target = nearestInRow(next, xOf(from));
      break;
    }
    case "ArrowUp": {
      if (from < 0) {
        target = 0;
        break;
      }
      const row = rowIndexOf(from);
      const prev = layout.rows[row - 1];
      if (!prev) {
        target = 0;
        break;
      }
      target = nearestInRow(prev, xOf(from));
      break;
    }
    case "Home":
      target = 0;
      break;
    case "End":
      target = total - 1;
      break;
    case "PageDown": {
      if (from < 0) {
        target = 0;
        break;
      }
      const row = Math.min(layout.rows.length - 1, rowIndexOf(from) + Math.max(1, pageRows));
      const page = layout.rows[row];
      target = page ? nearestInRow(page, xOf(from)) : total - 1;
      break;
    }
    case "PageUp": {
      if (from < 0) {
        target = 0;
        break;
      }
      const row = Math.max(0, rowIndexOf(from) - Math.max(1, pageRows));
      const page = layout.rows[row];
      target = page ? nearestInRow(page, xOf(from)) : 0;
      break;
    }
    default:
      return null;
  }
  return Math.min(Math.max(target, 0), total - 1);
}

/** The item index near the top of the scroll offset (for anchoring when the layout changes). */
export function itemAtScrollTop(layout: JustifiedLayout, scrollTop: number): number {
  if (layout.rows.length === 0) return 0;
  const y = Math.max(0, scrollTop - PADDING);
  for (const row of layout.rows) {
    if (y < row.top + row.stride) return row.startIndex;
  }
  return layout.rows[layout.rows.length - 1]?.startIndex ?? 0;
}
