import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderOpen, Images, RotateCcw, Save, Stethoscope } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { formatBytes, formatDate } from "@/features/browse/format";
import { type Backup, type BackupKind, inTauri, ipc, type LibraryInfo } from "@/ipc";
import { describeError } from "@/lib/errors";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  library: LibraryInfo;
  /** `true`, or why the library cannot change (read-only). */
  writable: true | string;
  /** The name changed, or a backup was restored: the library as it is now. */
  onChanged: (library: LibraryInfo, restored: boolean) => void;
  onCheck: () => void;
  onRebuild: () => void;
  onReveal: () => void;
};

const KIND_KEYS: Record<BackupKind, string> = {
  daily: "maintenance.kindDaily",
  manual: "maintenance.kindManual",
  beforeMigration: "maintenance.kindBeforeMigration",
  beforeRestore: "maintenance.kindBeforeRestore",
};

/**
 * Library Settings (docs/spec.md §7.4, item 6): the name, where the library is and its size on
 * disk, backups (make one, restore one), Rebuild Thumbnails and Check Library.
 */
export function LibrarySettingsDialog({
  open,
  onOpenChange,
  library,
  writable,
  onChanged,
  onCheck,
  onRebuild,
  onReveal,
}: Props) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [name, setName] = useState(library.name);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  /** The backup the user asked to restore, waiting for a second click. */
  const [restoring, setRestoring] = useState<string | null>(null);
  useEffect(() => {
    if (open) {
      setName(library.name);
      setError(undefined);
      setRestoring(null);
    }
  }, [open, library.name]);

  const stats = useQuery({
    queryKey: ["libraryStats", library.path],
    queryFn: ipc.libraryStats,
    enabled: open && inTauri(),
    staleTime: 0,
  });
  const backups = useQuery({
    queryKey: ["backups", library.path],
    queryFn: ipc.backups,
    enabled: open && inTauri(),
    staleTime: 0,
  });
  const blocked = writable === true ? undefined : writable;

  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    setError(undefined);
    try {
      await work();
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  const rename = (event: FormEvent) => {
    event.preventDefault();
    if (name.trim() === library.name) return;
    void run(async () => onChanged(await ipc.renameLibrary(name), false));
  };

  const backUp = () =>
    run(async () => {
      await ipc.backUpNow();
      await client.invalidateQueries({ queryKey: ["backups"] });
      await client.invalidateQueries({ queryKey: ["libraryStats"] });
    });

  const restore = (backup: Backup) =>
    run(async () => {
      const restored = await ipc.restoreBackup(backup.name);
      setRestoring(null);
      onChanged(restored, true);
      onOpenChange(false);
    });

  const total = stats.data
    ? stats.data.originals + stats.data.thumbnails + stats.data.database + stats.data.backups
    : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="grid max-h-[85vh] grid-cols-[minmax(0,1fr)] gap-5 overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("maintenance.librarySettings")}</DialogTitle>
          <DialogDescription>{library.name}</DialogDescription>
        </DialogHeader>

        <form className="grid gap-1.5" onSubmit={rename}>
          <Label htmlFor="library-name">{t("maintenance.name")}</Label>
          <div className="flex gap-2">
            <Input
              id="library-name"
              value={name}
              disabled={blocked !== undefined}
              title={blocked}
              onChange={(event) => setName(event.target.value)}
            />
            <Button
              type="submit"
              variant="outline"
              disabled={busy || blocked !== undefined || name.trim() === library.name}
            >
              {t("maintenance.rename")}
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">{t("maintenance.nameHint")}</p>
        </form>

        <section className="grid gap-1.5">
          <h3 className="text-sm font-medium">{t("maintenance.locationAndSize")}</h3>
          <div className="flex items-center gap-2">
            <p className="min-w-0 flex-1 text-[13px] break-all text-muted-foreground select-text">
              {library.path}
            </p>
            <Button variant="outline" size="sm" onClick={onReveal}>
              <FolderOpen aria-hidden />
              {t("maintenance.reveal")}
            </Button>
          </div>
          {stats.data && total !== null ? (
            <p className="text-[13px]">
              {t("maintenance.onDisk", {
                total: formatBytes(total),
                originals: formatBytes(stats.data.originals),
                thumbnails: formatBytes(stats.data.thumbnails),
                database: formatBytes(stats.data.database),
                backups: formatBytes(stats.data.backups),
              })}
            </p>
          ) : (
            <p className="text-[13px] text-muted-foreground">{t("maintenance.measuring")}</p>
          )}
        </section>

        <section className="grid gap-2">
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-sm font-medium">{t("maintenance.backups")}</h3>
            <Button
              variant="outline"
              size="sm"
              disabled={busy || blocked !== undefined}
              title={blocked ?? t("maintenance.backUpNowTitle")}
              onClick={() => void backUp()}
            >
              <Save aria-hidden />
              {t("maintenance.backUpNow")}
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">{t("maintenance.backupsHint")}</p>
          {(backups.data ?? []).length === 0 ? (
            <p className="text-[13px] text-muted-foreground">{t("maintenance.noBackups")}</p>
          ) : (
            <ul aria-label={t("maintenance.backups")} className="grid gap-1">
              {(backups.data ?? []).map((backup) => (
                <li
                  key={backup.name}
                  className="flex items-center gap-2 rounded-md border px-2 py-1.5 text-[13px]"
                >
                  <span className="min-w-0 flex-1">
                    <span className="block">{formatDate(backup.createdAt)}</span>
                    <span className="block text-xs text-muted-foreground">
                      {t(KIND_KEYS[backup.kind])} · {formatBytes(backup.size)}
                    </span>
                  </span>
                  {restoring === backup.name ? (
                    <>
                      <span className="text-xs text-muted-foreground">
                        {t("maintenance.restoreConfirm")}
                      </span>
                      <Button variant="outline" size="xs" onClick={() => setRestoring(null)}>
                        {t("common.cancel")}
                      </Button>
                      <Button
                        variant="destructive"
                        size="xs"
                        disabled={busy}
                        onClick={() => void restore(backup)}
                      >
                        {t("maintenance.restoreAction")}
                      </Button>
                    </>
                  ) : (
                    <Button
                      variant="ghost"
                      size="xs"
                      disabled={busy || blocked !== undefined}
                      title={blocked ?? t("maintenance.restoreTitle")}
                      onClick={() => setRestoring(backup.name)}
                    >
                      <RotateCcw aria-hidden />
                      {t("maintenance.restore")}
                    </Button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="grid gap-2">
          <h3 className="text-sm font-medium">{t("maintenance.maintenance")}</h3>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={blocked !== undefined}
              title={blocked ?? t("maintenance.rebuildThumbnailsTitle")}
              onClick={onRebuild}
            >
              <Images aria-hidden />
              {t("maintenance.rebuildThumbnails")}
            </Button>
            <Button variant="outline" size="sm" onClick={onCheck}>
              <Stethoscope aria-hidden />
              {t("maintenance.checkLibraryAction")}
            </Button>
          </div>
        </section>

        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}
