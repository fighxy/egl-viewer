// Walking the folder tree from Rust: lookups, paths, the rows the left bar shows, and where a
// folder may move (docs/spec.md §3).
import type { Folder } from "@/ipc";

export function findFolder(tree: readonly Folder[], id: string): Folder | undefined {
  for (const folder of tree) {
    if (folder.id === id) return folder;
    const found = findFolder(folder.children, id);
    if (found) return found;
  }
  return undefined;
}

/** The folders from the root down to `id`, or `[]` for an unknown id. */
export function folderPath(tree: readonly Folder[], id: string): Folder[] {
  for (const folder of tree) {
    if (folder.id === id) return [folder];
    const below = folderPath(folder.children, id);
    if (below.length > 0) return [folder, ...below];
  }
  return [];
}

/** The parent of `id`: `null` at the root, `undefined` for an unknown id. */
export function parentOf(tree: readonly Folder[], id: string): string | null | undefined {
  const path = folderPath(tree, id);
  if (path.length === 0) return undefined;
  return path.length > 1 ? (path[path.length - 2]?.id ?? null) : null;
}

/** The siblings of `id`, itself included, in order. */
export function siblingsOf(tree: readonly Folder[], id: string): Folder[] {
  const parent = parentOf(tree, id);
  if (parent === undefined) return [];
  return parent === null ? [...tree] : (findFolder(tree, parent)?.children ?? []);
}

/** `id` and every folder under it. */
export function subtreeIds(folder: Folder): Set<string> {
  const ids = new Set<string>([folder.id]);
  for (const child of folder.children) for (const id of subtreeIds(child)) ids.add(id);
  return ids;
}

/** The cycle guard: a folder may not move into itself or under one of its descendants. */
export function canMove(tree: readonly Folder[], id: string, parent: string | null): boolean {
  if (parent === null) return true;
  const folder = findFolder(tree, id);
  return folder !== undefined && !subtreeIds(folder).has(parent);
}

export type FolderRow = { folder: Folder; depth: number; hasChildren: boolean };

/** The rows of the tree as shown: children only under expanded folders. */
export function visibleRows(tree: readonly Folder[], expanded: ReadonlySet<string>, depth = 0) {
  const rows: FolderRow[] = [];
  for (const folder of tree) {
    rows.push({ folder, depth, hasChildren: folder.children.length > 0 });
    if (expanded.has(folder.id)) rows.push(...visibleRows(folder.children, expanded, depth + 1));
  }
  return rows;
}

/** Every folder in tree order, with its depth (pickers). */
export function allRows(tree: readonly Folder[], depth = 0): FolderRow[] {
  return tree.flatMap((folder) => [
    { folder, depth, hasChildren: folder.children.length > 0 },
    ...allRows(folder.children, depth + 1),
  ]);
}

/** The count a row shows: direct items, or with subfolders when that switch is on. */
export function folderCount(folder: Folder, includeSubfolders: boolean): number | null {
  return includeSubfolders ? folder.descendantImageCount : folder.imageCount;
}

/** CSS colours of the eight folder colours (Eagle's names). */
export const FOLDER_COLORS: Record<string, string> = {
  red: "#ef4444",
  orange: "#f97316",
  yellow: "#eab308",
  green: "#22c55e",
  aqua: "#06b6d4",
  blue: "#3b82f6",
  purple: "#a855f7",
  pink: "#ec4899",
};
