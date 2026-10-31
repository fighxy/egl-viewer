import i18n from "@/i18n";
import type { ImportProgress } from "@/ipc";

/** English-only plural helper still used by a few untranslated dialogs. */
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

function fmt(count: number) {
  return count.toLocaleString(i18n.language);
}

/** One line for the status bar and the Imports panel (docs/spec.md §4, §7.3). */
export function importSummary(progress: ImportProgress): string {
  if (progress.kind === "previews") {
    return isRunning(progress)
      ? i18n.t("imports.makingPreviews", {
          done: progress.previewsDone,
          total: progress.previewsTotal,
        })
      : i18n.t("imports.madePreviews", {
          count: progress.previewsDone,
          formatted: fmt(progress.previewsDone),
        });
  }
  const extras = [
    progress.duplicates > 0
      ? i18n.t("imports.duplicatesSkipped", {
          count: progress.duplicates,
          formatted: fmt(progress.duplicates),
        })
      : null,
    progress.failed > 0
      ? i18n.t("imports.problemsCount", {
          count: progress.failed,
          formatted: fmt(progress.failed),
        })
      : null,
  ].filter((part): part is string => part !== null);
  const tail = extras.length > 0 ? ` · ${extras.join(" · ")}` : "";
  switch (progress.state) {
    case "scanning":
      return i18n.t("imports.looking");
    case "adding":
      return (
        i18n.t("imports.adding", { processed: progress.processed, total: progress.total }) + tail
      );
    case "waitingForDecision":
      return i18n.t("imports.waitingDupes");
    case "previews":
      return (
        i18n.t("imports.makingPreviews", {
          done: progress.previewsDone,
          total: progress.previewsTotal,
        }) + tail
      );
    case "done":
      return (
        i18n.t("imports.added", { count: progress.added, formatted: fmt(progress.added) }) + tail
      );
    case "cancelled":
      return (
        i18n.t("imports.stopped", { count: progress.added, formatted: fmt(progress.added) }) +
        tail
      );
  }
}
