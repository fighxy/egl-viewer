import type { ItemsTarget, SelectionTags } from "@/ipc";

/**
 * Whether dropping items of `target` onto the tag `name` would change nothing.
 * True only when every item is already known to carry that exact string.
 * An item whose tags are not loaded is not treated as already tagged (D-057).
 */
export function dropAddsNothing(
  name: string,
  target: ItemsTarget,
  known: ReadonlyMap<string, readonly string[]>,
  selection: { target: ItemsTarget; tags: SelectionTags } | null,
): boolean {
  if (!target.all && target.ids.length === 1) {
    const id = target.ids[0];
    if (id === undefined) return false;
    return known.get(id)?.includes(name) ?? false;
  }
  if (!selection || !sameTarget(target, selection.target) || selection.tags.items === 0) {
    return false;
  }
  return selection.tags.tags.some(
    (tag) => tag.name === name && tag.imageCount === selection.tags.items,
  );
}

function sameTarget(a: ItemsTarget, b: ItemsTarget): boolean {
  if (a.viewId !== b.viewId || a.all !== b.all || a.ids.length !== b.ids.length) return false;
  const ids = new Set(a.ids);
  return b.ids.every((id) => ids.has(id));
}
