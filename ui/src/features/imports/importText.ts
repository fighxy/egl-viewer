import type { ImportProgress } from "@/ipc";

export function plural(count: number, one: string, many = `${one}s`) {
  return `${count.toLocaleString("en")} ${count === 1 ? one : many}`;
}

export function isRunning(progress: ImportProgress) {
  return progress.state !== "done" && progress.state !== "cancelled";
}

export function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Percent done: files while adding, previews afterwards. */
export function percent(progress: ImportProgress): number {
  if (progress.state === "done") return 100;
  if (progress.kind === "previews" || progress.state === "previews") {
    return progress.previewsTotal === 0
      ? 100
      : Math.round((progress.previewsDone / progress.previewsTotal) * 100);
  }
  return progress.total === 0 ? 0 : Math.round((progress.processed / progress.total) * 100);
}

/** One line for the status bar and the Imports panel (docs/spec.md §4, §7.3). */
export function importSummary(progress: ImportProgress): string {
  if (progress.kind === "previews") {
    return isRunning(progress)
      ? `Making previews: ${progress.previewsDone} of ${progress.previewsTotal}`
      : `Made ${plural(progress.previewsDone, "preview")}`;
  }
  const extras = [
    progress.duplicates > 0 ? `${plural(progress.duplicates, "duplicate")} skipped` : null,
    progress.failed > 0 ? plural(progress.failed, "problem") : null,
  ].filter((part): part is string => part !== null);
  const tail = extras.length > 0 ? ` · ${extras.join(" · ")}` : "";
  switch (progress.state) {
    case "scanning":
      return "Looking for files…";
    case "adding":
      return `Adding files: ${progress.processed} of ${progress.total}${tail}`;
    case "waitingForDecision":
      return "Waiting for your choice about duplicates";
    case "previews":
      return `Making previews: ${progress.previewsDone} of ${progress.previewsTotal}${tail}`;
    case "done":
      return `Added ${plural(progress.added, "file")}${tail}`;
    case "cancelled":
      return `Stopped · added ${plural(progress.added, "file")}${tail}`;
  }
}
