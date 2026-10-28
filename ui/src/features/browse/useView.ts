// A view in Rust for the screen's query, and the windows of cells the grid shows
// (docs/architecture.md §6.5). Whole item lists never cross IPC.
import { type QueryClient, useQueries, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  EVENTS,
  type ItemCell,
  inTauri,
  ipc,
  type LibraryQuery,
  type ScopeProgress,
  type SourceError,
  type ViewOpened,
  type ViewsInvalidated,
} from "@/ipc";
import { describeError } from "@/lib/errors";

/** Cells per `view_range` call (Rust serves at most 200). */
export const WINDOW = 120;

function windowQuery(viewId: number, window: number) {
  return {
    queryKey: ["view", viewId, window] as const,
    queryFn: () => ipc.viewRange(viewId, window * WINDOW, WINDOW),
    // A view changes only through `views-invalidated`, which invalidates these keys.
    staleTime: Number.POSITIVE_INFINITY,
  };
}

function isNotFound(error: unknown): boolean {
  return (error as SourceError | null)?.kind === "notFound";
}

/**
 * Opens a view for `query` and keeps its total current. The previous view closes once the new
 * one is open, so the grid never flashes empty when the sort changes.
 */
export function useView(query: LibraryQuery | null) {
  const client = useQueryClient();
  const [view, setView] = useState<ViewOpened | null>(null);
  const [progress, setProgress] = useState<ScopeProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const current = useRef<ViewOpened | null>(null);
  const key = query === null ? null : JSON.stringify(query);

  const forget = useCallback(
    (viewId: number) => {
      ipc.closeView(viewId).catch(() => {});
      client.removeQueries({ queryKey: ["view", viewId] });
    },
    [client],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` reopens a lost view.
  useEffect(() => {
    if (key === null || !inTauri()) {
      if (current.current) forget(current.current.viewId);
      current.current = null;
      setView(null);
      return;
    }
    let cancelled = false;
    setProgress(null);
    ipc
      .openView(JSON.parse(key) as LibraryQuery, (next) => {
        if (!cancelled) setProgress(next);
      })
      .then((opened) => {
        if (cancelled) {
          forget(opened.viewId);
          return;
        }
        const previous = current.current;
        current.current = opened;
        setView(opened);
        setError(null);
        if (previous) forget(previous.viewId);
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(describeError(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [key, attempt, forget]);

  // The last view closes with the screen.
  useEffect(
    () => () => {
      if (current.current) forget(current.current.viewId);
      current.current = null;
    },
    [forget],
  );

  // Views recomputed after a write keep their ids: new totals, refetched windows.
  useEffect(() => {
    if (!inTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<ViewsInvalidated>(EVENTS.viewsInvalidated, (event) => {
      for (const changed of event.payload.views) {
        if (changed.viewId === current.current?.viewId) {
          current.current = changed;
          setView(changed);
        }
        void client.invalidateQueries({ queryKey: ["view", changed.viewId] });
      }
      void client.invalidateQueries({ queryKey: ["item"] });
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [client]);

  const reopen = useCallback(() => setAttempt((count) => count + 1), []);
  return { view, progress, error, reopen };
}

/**
 * The cells at positions `first..=last` of a view, fetched in windows of {@link WINDOW}.
 * `onLost` runs when Rust no longer has the view (it was dropped as old).
 */
export function useCells(
  viewId: number | null,
  first: number,
  last: number,
  onLost?: () => void,
): (index: number) => ItemCell | undefined {
  const windows: number[] = [];
  if (viewId !== null && last >= first && first >= 0) {
    for (let w = Math.floor(first / WINDOW); w <= Math.floor(last / WINDOW); w += 1) {
      windows.push(w);
    }
  }
  const results = useQueries({
    queries: windows.map((w) => windowQuery(viewId ?? 0, w)),
  });
  const lost = results.some((result) => isNotFound(result.error));
  useEffect(() => {
    if (lost) onLost?.();
  }, [lost, onLost]);
  return (index) => {
    const w = Math.floor(index / WINDOW);
    const at = windows.indexOf(w);
    return at < 0 ? undefined : results[at]?.data?.[index - w * WINDOW];
  };
}

/** The cell at `index`, from the cache or from Rust (for keyboard moves past the screen). */
export async function fetchCell(
  client: QueryClient,
  viewId: number,
  index: number,
): Promise<ItemCell | undefined> {
  const w = Math.floor(index / WINDOW);
  const cells = await client.fetchQuery(windowQuery(viewId, w));
  return cells[index - w * WINDOW];
}
