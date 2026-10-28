// The selection of a view: ids picked with click, Ctrl, Shift and the rubber band, or every item
// after Ctrl+A. Ranges reach beyond the loaded windows, so Rust gives their ids (`view_ids`);
// "everything" is a flag, never a list of every id.

export type Selection = {
  /** `true`: every item of the view except `ids`. `false`: exactly `ids`. */
  all: boolean;
  ids: ReadonlySet<string>;
  /** Where a Shift range starts: a position in the view. */
  anchor: number | null;
  /** Where the keyboard moves from. */
  focus: number | null;
};

export const NOTHING: Selection = { all: false, ids: new Set(), anchor: null, focus: null };

export function isSelected(selection: Selection, id: string): boolean {
  return selection.all !== selection.ids.has(id);
}

export function selectedCount(selection: Selection, total: number): number {
  return selection.all ? Math.max(0, total - selection.ids.size) : selection.ids.size;
}

/** The selected item when exactly one is selected and its id is known. */
export function singleId(selection: Selection): string | null {
  if (selection.all || selection.ids.size !== 1) return null;
  return selection.ids.values().next().value ?? null;
}

/** A plain click: this item only. */
export function only(index: number, id: string): Selection {
  return { all: false, ids: new Set([id]), anchor: index, focus: index };
}

/** Ctrl+click: this item in or out; it becomes the anchor. */
export function toggle(selection: Selection, index: number, id: string): Selection {
  const ids = new Set(selection.ids);
  if (ids.has(id)) ids.delete(id);
  else ids.add(id);
  return { ...selection, ids, anchor: index, focus: index };
}

/** The positions from the anchor to `index`, as `[start, length]` for `view_ids`. */
export function rangeFrom(selection: Selection, index: number): [number, number] {
  const anchor = selection.anchor ?? index;
  const start = Math.min(anchor, index);
  return [start, Math.abs(anchor - index) + 1];
}

/**
 * Shift+click or Shift+arrow: `rangeIds` replace the selection, or join it with Ctrl. The
 * anchor stays where it was.
 */
export function withRange(
  selection: Selection,
  index: number,
  rangeIds: readonly string[],
  additive: boolean,
): Selection {
  const anchor = selection.anchor ?? index;
  if (additive && selection.all) {
    const except = new Set(selection.ids);
    for (const id of rangeIds) except.delete(id);
    return { all: true, ids: except, anchor, focus: index };
  }
  const ids = new Set(additive ? selection.ids : []);
  for (const id of rangeIds) ids.add(id);
  return { all: false, ids, anchor, focus: index };
}

/** Ctrl+A. */
export function everything(selection: Selection): Selection {
  return { ...selection, all: true, ids: new Set() };
}
