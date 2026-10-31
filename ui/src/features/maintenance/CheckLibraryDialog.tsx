import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
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
  const { t, i18n } = useTranslation();
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
  const fmt = (count: number) => count.toLocaleString(i18n.language);

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

  const strayParts = report
    ? [
        report.orphanCount > 0 &&
          t("maintenance.checkOrphanFiles", {
            count: report.orphanCount,
            formatted: fmt(report.orphanCount),
            bytes: formatBytes(report.orphanBytes),
          }),
        report.orphanThumbnails > 0 &&
          t("maintenance.checkOrphanThumbnails", {
            count: report.orphanThumbnails,
            formatted: fmt(report.orphanThumbnails),
          }),
      ].filter((part): part is string => Boolean(part))
    : [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="grid max-h-[85vh] grid-cols-[minmax(0,1fr)] gap-4 overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("maintenance.checkLibrary")}</DialogTitle>
          <DialogDescription>
            {report
              ? t("maintenance.checkItemsChecked", {
                  count: report.items,
                  formatted: fmt(report.items),
                })
              : check.isError
                ? describeError(check.error)
                : t("maintenance.checkChecking")}
          </DialogDescription>
        </DialogHeader>
        {report && (
          <div className="grid gap-3 text-[13px]">
            <p>
              {report.databaseOk
                ? t("maintenance.checkDatabaseOk")
                : t("maintenance.checkDatabaseProblems", {
                    problems: report.databaseProblems.join("; "),
                  })}
            </p>
            {report.missingCount === 0 ? (
              <p>{t("maintenance.checkEveryFile")}</p>
            ) : (
              <div className="grid gap-1">
                <p>
                  {t("maintenance.checkMissing", {
                    count: report.missingCount,
                    formatted: fmt(report.missingCount),
                  })}
                </p>
                <ul className="max-h-32 overflow-y-auto rounded-md border px-2 py-1 text-xs">
                  {report.missing.map((item) => (
                    <li key={item.id}>
                      {item.name || t("maintenance.checkLockedItem")}
                      {item.inTrash && t("maintenance.checkInTrash")}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {strays === 0 ? (
              <p>{t("maintenance.checkNoStrays")}</p>
            ) : (
              <div className="grid gap-1">
                <p>
                  {t("maintenance.checkStraysJoin", {
                    parts: strayParts.join(t("maintenance.checkAnd")),
                  })}
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
                <legend className="px-1 text-xs font-medium">{t("maintenance.checkRepair")}</legend>
                {strays > 0 && (
                  <div className="flex items-center justify-between gap-3">
                    <Label htmlFor="repair-orphans">{t("maintenance.checkDeleteOrphans")}</Label>
                    <Switch id="repair-orphans" checked={orphans} onCheckedChange={setOrphans} />
                  </div>
                )}
                {report.missingCount > 0 && (
                  <div className="flex items-center justify-between gap-3">
                    <Label htmlFor="repair-missing">{t("maintenance.checkRemoveMissing")}</Label>
                    <Switch id="repair-missing" checked={missing} onCheckedChange={setMissing} />
                  </div>
                )}
                <p className="text-xs text-muted-foreground">{t("maintenance.checkRepairHint")}</p>
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
            {t("maintenance.checkClose")}
          </Button>
          {report && (orphans || missing) && (
            <Button variant="destructive" disabled={busy} onClick={() => void repair()}>
              {t("maintenance.checkRepairAction")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
