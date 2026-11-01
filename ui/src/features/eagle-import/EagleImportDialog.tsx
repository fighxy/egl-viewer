import { type FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import type { EagleImportRequest, EagleLibrarySummary, ImportProgress, LibraryInfo } from "@/ipc";
import { describeError } from "@/lib/errors";

type Step = "pick" | "target" | "running" | "report";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The library that is open: the import can go into it. */
  openLibrary: LibraryInfo | null;
  loadDefaultLocation: () => Promise<string>;
  browse: (current: string) => Promise<string | null>;
  info: (path: string) => Promise<EagleLibrarySummary>;
  start: (request: EagleImportRequest) => Promise<{ id: number }>;
  /** The import into a new library: the library is open now. */
  onNewLibraryOpened: () => void;
  /** The progress of the running import, from `import-progress` events. */
  progress: ImportProgress | null;
  onCancel: (id: number) => void;
};

function formatBytes(bytes: number): string {
  if (bytes >= 1 << 30) return `${(bytes / (1 << 30)).toFixed(1)} GB`;
  if (bytes >= 1 << 20) return `${(bytes / (1 << 20)).toFixed(1)} MB`;
  if (bytes >= 1 << 10) return `${(bytes / (1 << 10)).toFixed(0)} KB`;
  return `${bytes} B`;
}

/** The import wizard of docs/spec.md §8.2: pick, target and options, progress, report. */
export function EagleImportDialog({
  open,
  onOpenChange,
  openLibrary,
  loadDefaultLocation,
  browse,
  info,
  start,
  onNewLibraryOpened,
  progress,
  onCancel,
}: Props) {
  const { t } = useTranslation();
  const [step, setStep] = useState<Step>("pick");
  const [path, setPath] = useState("");
  const [summary, setSummary] = useState<EagleLibrarySummary | null>(null);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [target, setTarget] = useState<"new" | "open">("new");
  const [name, setName] = useState("");
  const [location, setLocation] = useState("");
  const [importTrash, setImportTrash] = useState(false);
  const [ticket, setTicket] = useState<number | null>(null);

  useEffect(() => {
    if (!open) return;
    setStep("pick");
    setPath("");
    setSummary(null);
    setError(undefined);
    setImportTrash(false);
    setTicket(null);
  }, [open]);

  useEffect(() => {
    if (step === "target" && target === "new" && location === "") {
      loadDefaultLocation()
        .then(setLocation)
        .catch(() => setLocation(""));
    }
  }, [step, target, location, loadDefaultLocation]);

  // The running step follows the import's progress events to the report.
  useEffect(() => {
    if (step !== "running" || ticket === null) return;
    if (progress?.id === ticket && (progress.state === "done" || progress.state === "cancelled")) {
      setStep("report");
    }
  }, [step, ticket, progress]);

  const validate = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      const found = await info(path.trim());
      setSummary(found);
      setName(found.name);
      setTarget("new");
      setStep("target");
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  const begin = async () => {
    if (!summary) return;
    setBusy(true);
    setError(undefined);
    try {
      const created = await start({
        eaglePath: summary.path,
        create: target === "new" ? { parent: location.trim(), name: name.trim() } : null,
        importTrash,
      });
      setTicket(created.id);
      setStep("running");
      if (target === "new") onNewLibraryOpened();
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  const close = () => onOpenChange(false);
  const running = progress?.id === ticket ? progress : null;
  const report = running?.report ?? null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("eagleImport.title")}</DialogTitle>
          <DialogDescription>{t("eagleImport.description")}</DialogDescription>
        </DialogHeader>

        {step === "pick" && (
          <form className="grid gap-4" onSubmit={validate} noValidate>
            <div className="grid gap-1.5">
              <Label htmlFor="eagle-folder">{t("eagleImport.folder")}</Label>
              <div className="flex gap-2">
                <Input
                  id="eagle-folder"
                  autoFocus
                  value={path}
                  onChange={(event) => setPath(event.target.value)}
                  placeholder="D:\Libraries\My Library.library"
                />
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => {
                    void browse(path).then((picked) => {
                      if (picked) setPath(picked);
                    });
                  }}
                >
                  {t("common.browse")}
                </Button>
              </div>
            </div>
            {error && (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            )}
            <DialogFooter>
              <Button type="button" variant="outline" onClick={close}>
                {t("common.cancel")}
              </Button>
              <Button type="submit" disabled={path.trim() === "" || busy}>
                {t("eagleImport.next")}
              </Button>
            </DialogFooter>
          </form>
        )}

        {step === "target" && summary && (
          <div className="grid gap-4">
            <p className="text-sm text-muted-foreground">
              {t("eagleImport.summary", {
                name: summary.name,
                count: summary.itemCount,
                size: formatBytes(summary.bytes),
              })}
            </p>
            <div className="grid gap-2">
              <label className="flex items-center gap-2 rounded-md border border-border p-3">
                <input
                  type="radio"
                  name="eagle-import-target"
                  checked={target === "new"}
                  onChange={() => setTarget("new")}
                />
                <span className="text-sm font-medium">{t("eagleImport.intoNew")}</span>
              </label>
              {target === "new" && (
                <div className="grid gap-1.5 pl-6">
                  <Input
                    aria-label={t("createLibrary.name")}
                    value={name}
                    onChange={(event) => setName(event.target.value)}
                  />
                  <div className="flex gap-2">
                    <Input
                      aria-label={t("createLibrary.location")}
                      value={location}
                      onChange={(event) => setLocation(event.target.value)}
                    />
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => {
                        void browse(location).then((picked) => {
                          if (picked) setLocation(picked);
                        });
                      }}
                    >
                      {t("common.browse")}
                    </Button>
                  </div>
                </div>
              )}
              <label
                className={`flex items-center gap-2 rounded-md border border-border p-3 ${openLibrary ? "" : "opacity-60"}`}
              >
                <input
                  type="radio"
                  name="eagle-import-target"
                  disabled={!openLibrary}
                  checked={target === "open"}
                  onChange={() => setTarget("open")}
                />
                <div className="grid gap-1">
                  <span className="text-sm font-medium">
                    {openLibrary
                      ? t("eagleImport.intoOpen", { name: openLibrary.name })
                      : t("eagleImport.noOpenLibrary")}
                  </span>
                  {target === "open" && (
                    <span className="text-xs text-muted-foreground">
                      {t("eagleImport.intoOpenHint", { name: summary.name })}
                    </span>
                  )}
                </div>
              </label>
            </div>
            <div className="flex items-center gap-2">
              <Checkbox
                id="import-trash"
                checked={importTrash}
                onCheckedChange={(checked) => setImportTrash(checked === true)}
              />
              <Label htmlFor="import-trash" className="font-normal">
                {t("eagleImport.importTrash")}
              </Label>
            </div>
            {error && (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            )}
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setStep("pick")}>
                {t("eagleImport.back")}
              </Button>
              <Button
                type="button"
                disabled={
                  busy || (target === "new" && (name.trim() === "" || location.trim() === ""))
                }
                onClick={() => void begin()}
              >
                {t("eagleImport.start")}
              </Button>
            </DialogFooter>
          </div>
        )}

        {step === "running" && (
          <div className="grid gap-4">
            <div className="grid gap-1.5">
              <Progress
                value={running?.total ? (running.processed / running.total) * 100 : undefined}
              />
              <p className="text-sm text-muted-foreground">
                {running
                  ? t("eagleImport.progress", {
                      processed: running.processed,
                      total: running.total,
                    })
                  : t("eagleImport.starting")}
              </p>
            </div>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => ticket !== null && onCancel(ticket)}
              >
                {t("eagleImport.cancelImport")}
              </Button>
            </DialogFooter>
          </div>
        )}

        {step === "report" && report && (
          <div className="grid gap-4">
            <ul className="grid gap-1 text-sm">
              <li>{t("eagleImport.imported", { count: report.imported })}</li>
              {report.duplicates > 0 && (
                <li>{t("eagleImport.duplicates", { count: report.duplicates })}</li>
              )}
              {report.trashSkipped > 0 && (
                <li>{t("eagleImport.trashSkipped", { count: report.trashSkipped })}</li>
              )}
              {report.skipped.length > 0 && (
                <li>{t("eagleImport.skipped", { count: report.skipped.length })}</li>
              )}
            </ul>
            {report.lockedFolders.length > 0 && (
              <p className="text-sm text-muted-foreground">
                {t("eagleImport.locked", { names: report.lockedFolders.join(", ") })}
              </p>
            )}
            {report.skipped.length > 0 && (
              <ul className="max-h-36 overflow-y-auto rounded border p-2 text-xs text-muted-foreground">
                {report.skipped.map((line) => (
                  <li key={line.path} className="truncate" title={line.path}>
                    {line.path}: {line.reason}
                  </li>
                ))}
              </ul>
            )}
            {running?.state === "cancelled" && (
              <p className="text-sm text-muted-foreground">{t("eagleImport.cancelledHint")}</p>
            )}
            <DialogFooter>
              <Button type="button" onClick={close}>
                {t("common.close")}
              </Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
