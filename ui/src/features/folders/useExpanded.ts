// Which folders are expanded in the tree, per library. A convenience of this window kept in the
// browser's storage: it may be lost, and it is not library data (library.db holds none of it).
import { useCallback, useState } from "react";

const KEY = "egl-viewer.expanded:";

function load(path: string | null): Set<string> {
  if (!path) return new Set();
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(KEY + path) ?? "[]");
    return new Set(
      Array.isArray(stored) ? stored.filter((id): id is string => typeof id === "string") : [],
    );
  } catch {
    return new Set();
  }
}

function save(path: string | null, ids: ReadonlySet<string>) {
  if (!path) return;
  try {
    localStorage.setItem(KEY + path, JSON.stringify([...ids]));
  } catch {
    // Storage can be blocked; the tree then starts collapsed next time.
  }
}

/** The expanded folders of the library at `path`, and a setter for one folder. */
export function useExpanded(path: string | null) {
  const [state, setState] = useState(() => ({ path, ids: load(path) }));
  let ids = state.ids;
  if (state.path !== path) {
    ids = load(path);
    setState({ path, ids });
  }
  const setExpanded = useCallback((id: string, open: boolean) => {
    setState((previous) => {
      if (previous.ids.has(id) === open) return previous;
      const next = new Set(previous.ids);
      if (open) next.add(id);
      else next.delete(id);
      save(previous.path, next);
      return { ...previous, ids: next };
    });
  }, []);
  return [ids as ReadonlySet<string>, setExpanded] as const;
}
