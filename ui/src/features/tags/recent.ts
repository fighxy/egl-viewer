// The tags this window used last, per library (docs/spec.md §3 "The app keeps its own list of
// recently used tags"). A convenience kept in the browser's storage: it may be lost, and
// library.db holds none of it.
import { useCallback, useState } from "react";

const KEY = "egl-viewer.recentTags:";

/** How many recent tags are kept. */
export const RECENT_LIMIT = 12;

function load(path: string | null): string[] {
  if (!path) return [];
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(KEY + path) ?? "[]");
    return Array.isArray(stored)
      ? stored.filter((tag): tag is string => typeof tag === "string").slice(0, RECENT_LIMIT)
      : [];
  } catch {
    return [];
  }
}

function save(path: string | null, tags: readonly string[]) {
  if (!path) return;
  try {
    localStorage.setItem(KEY + path, JSON.stringify(tags));
  } catch {
    // Storage can be blocked; the list then starts empty next time.
  }
}

/** `tags` with `used` first, each name once (exact strings: "Poster" and "poster" are two). */
export function remember(tags: readonly string[], used: readonly string[]): string[] {
  const fresh = [...new Set(used)];
  return [...fresh, ...tags.filter((tag) => !fresh.includes(tag))].slice(0, RECENT_LIMIT);
}

/** The list after `from` became `to` (a rename or a merge); `to` null: the tag was deleted. */
export function replaced(tags: readonly string[], from: string, to: string | null): string[] {
  const next = tags.map((tag) => (tag === from ? to : tag));
  return [...new Set(next.filter((tag): tag is string => tag !== null))];
}

export type RecentTags = {
  tags: string[];
  /** Tags were just added to items. */
  use: (tags: readonly string[]) => void;
  /** A tag was renamed or merged (`to`), or deleted (`null`). */
  replace: (from: string, to: string | null) => void;
};

/** The recent tags of the library at `path`. */
export function useRecentTags(path: string | null): RecentTags {
  const [state, setState] = useState(() => ({ path, tags: load(path) }));
  let tags = state.tags;
  if (state.path !== path) {
    tags = load(path);
    setState({ path, tags });
  }
  const update = useCallback((change: (tags: string[]) => string[]) => {
    setState((previous) => {
      const next = change(previous.tags);
      save(previous.path, next);
      return { ...previous, tags: next };
    });
  }, []);
  const use = useCallback(
    (used: readonly string[]) => update((current) => remember(current, used)),
    [update],
  );
  const replace = useCallback(
    (from: string, to: string | null) => update((current) => replaced(current, from, to)),
    [update],
  );
  return { tags, use, replace };
}
