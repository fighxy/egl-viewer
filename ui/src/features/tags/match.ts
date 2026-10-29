import type { Tag } from "@/ipc";

/**
 * Tags whose name contains `text`, names that start with it first. Matching ignores case so
 * that "post" finds "Poster" and "poster"; the tags themselves stay exact strings and are
 * never changed (docs/spec.md §5).
 */
export function matchTags<T extends Pick<Tag, "name">>(tags: readonly T[], text: string): T[] {
  const needle = text.trim().toLocaleLowerCase();
  if (needle === "") return [...tags];
  const starts: T[] = [];
  const contains: T[] = [];
  for (const tag of tags) {
    const name = tag.name.toLocaleLowerCase();
    if (name.startsWith(needle)) starts.push(tag);
    else if (name.includes(needle)) contains.push(tag);
  }
  return [...starts, ...contains];
}

/** The name as typed, without spaces at either end; empty when there is none. */
export function cleanTag(text: string): string {
  return text.trim();
}

/** How a tag's item count reads in lists. */
export function countText(count: number | null | undefined): string {
  return count === null || count === undefined ? "" : count.toLocaleString("en");
}
