import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { EVENTS, type ImportProgress, inTauri, ipc } from "@/ipc";
import { isRunning } from "./importText";

/** Import tasks of the open library, kept current from `import-progress` events. */
export function useImports(libraryKey: string | null) {
  const [byId, setById] = useState<Map<number, ImportProgress>>(new Map());

  useEffect(() => {
    setById(new Map());
    if (!inTauri() || libraryKey === null) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<ImportProgress>(EVENTS.importProgress, (event) => {
      setById((current) => new Map(current).set(event.payload.id, event.payload));
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    ipc
      .imports()
      .then((list) =>
        setById((current) => {
          const next = new Map(current);
          for (const progress of list) if (!next.has(progress.id)) next.set(progress.id, progress);
          return next;
        }),
      )
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [libraryKey]);

  return useMemo(() => {
    const imports = [...byId.values()].sort((a, b) => b.id - a.id);
    return {
      imports,
      running: imports.filter(isRunning),
      waiting: imports.find((progress) => progress.state === "waitingForDecision"),
      added: imports.reduce((sum, progress) => sum + progress.added, 0),
    };
  }, [byId]);
}
