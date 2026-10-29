import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { formatBytes } from "@/features/browse/format";
import { inTauri, ipc, type LibraryCheck } from "@/ipc";
import { describeError } from "@/lib/errors";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  libraryPath: string | null;
  /** `true`, or why nothing can be repaired (read-only). */
  writable: true | string;
  /** Items or files were removed. */
  onRepaired: () => void;
};

function plural(count: number, word: string): string {
  return `${count.toLocaleString("en")} ${word}${count === 1 ? "" : "s"}`;
}

/**
 * Check Library (docs/spec.md §9, phase 7): the database's own check, items whose original is
 * missing, and files no item points at. Nothing is removed unless the user asks.
 */
export function CheckLibraryDialog({
  open,
  onOpenChange,
  libraryPath,
  writable,
  onRepaired,
}: Props) {
  const check = useQuery({
    queryKey: ["checkLibrary", libraryPath],
    queryFn: ipc.checkLibrary,
    enabled: open && inTauri(),
    staleTime: 0,
    gcTime: 0,
  });
  const [result, setResult] = useState<LibraryCheck | null>(null);
  const [orphans, setOrphans] = useState(false);
  const [missing, setMissing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  useEffect(() => {
    if (open) {
      setResult(null);
      setOrphans(false);
      setMissing(false);
      setError(undefined);
    }
  }, [open]);

  const report = result ?? check.data ?? null;
  const strays = report ? report.orphanCount + report.orphanThumbnails : 0;

  const repair = async () => {
    setBusy(true);
    setError(undefined);
    try {
      setResult(await ipc.repairLibrary({ orphans, missing }));
      setOrphans(false);
      setMissing(false);
      onRepaired();
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="grid max-h-[85vh] grid-cols-[minmax(0,1fr)] gap-4 overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Check Library</DialogTitle>
          <DialogDescription>
            {report
              ? `${plural(report.items, "item")} checked.`
              : check.isError
                ? describeError(check.error)
                : "Checking the database and the files…"}
          </DialogDescription>
        </DialogHeader>
        {report && (
          <div className="grid gap-3 text-[13px]">
            <p>
              {report.databaseOk
                ? "The database is fine."
                : `The database has problems: ${report.databaseProblems.join("; ")}. Restore a backup in Library Settings.`}
            </p>
            {report.missingCount === 0 ? (
              <p>Every item has its file.</p>
            ) : (
              <div className="grid gap-1">
                <p>{plural(report.missingCount, "item")} without a file:</p>
                <ul className="max-h-32 overflow-y-auto rounded-md border px-2 py-1 text-xs">
                  {report.missing.map((item) => (
                    <li key={item.id}>
                      {item.name || "An item of a locked folder"}
                      {item.inTrash && " (in the trash)"}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {strays === 0 ? (
              <p>No stray files.</p>
            ) : (
              <div className="grid gap-1">
                <p>
                  {[
                    report.orphanCount > 0 &&
                      `${plural(report.orphanCount, "file")} (${formatBytes(report.orphanBytes)})`,
                    report.orphanThumbnails > 0 && plural(report.orphanThumbnails, "thumbnail"),
                  ]
                    .filter(Boolean)
                    .join(" and ")}{" "}
                  that no item points at. They are often left by restoring an older backup.
                </p>
                {report.orphans.length > 0 && (
                  <ul className="max-h-32 overflow-y-auto rounded-md border px-2 py-1 text-xs break-all select-text">
                    {report.orphans.map((path) => (
                      <li key={path}>{path}</li>
                    ))}
                  </ul>
                )}
              </div>
            )}
            {(strays > 0 || report.missingCount > 0) && writable === true && (
              <fieldset className="grid gap-2 rounded-md border p-3">
                <legend className="px-1 text-xs font-medium">Repair</legend>
                {strays > 0 && (
                  <div className="flex items-center justify-between gap-3">
                    <Label htmlFor="repair-orphans">Delete the stray files</Label>
                    <Switch id="repair-orphans" checked={orphans} onCheckedChange={setOrphans} />
                  </div>
                )}
                {report.missingCount > 0 && (
                  <div className="flex items-center justify-between gap-3">
                    <Label htmlFor="repair-missing">Remove the items without a file</Label>
                    <Switch id="repair-missing" checked={missing} onCheckedChange={setMissing} />
                  </div>
                )}
                <p className="text-xs text-muted-foreground">
                  There is no undo. A backup restored later may need the deleted files.
                </p>
              </fieldset>
            )}
          </div>
        )}
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
          {report && (orphans || missing) && (
            <Button variant="destructive" disabled={busy} onClick={() => void repair()}>
              Repair
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
