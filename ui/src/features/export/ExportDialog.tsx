import { useQuery } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { TFunction } from "i18next";
import { FolderOpen, HardDrive } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import { formatBytes } from "@/features/browse/format";
import { isValid, validateConnectForm } from "@/features/connect/validation";
import { allRows, FOLDER_COLORS, withoutLockedFolders } from "@/features/folders/tree";
import { countText } from "@/features/tags/match";
import type {
  Compression,
  ExportProgress,
  ExportRequest,
  ExportSelection,
  ExportState,
  Folder,
  LibraryInfo,
  Preferences,
  Tag,
} from "@/ipc";
import { EVENTS, inTauri, ipc } from "@/ipc";
import { describeError } from "@/lib/errors";

/** What an Export from a context menu starts with; the Backup menu starts free (docs/spec.md §7.4 item 12). */
export type ExportTarget =
  | { kind: "folder"; id: string; name: string }
  | { kind: "tag"; name: string };

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  library: LibraryInfo;
  target: ExportTarget | null;
  /** The folder tree, for the free choice of folders. */
  folders: Folder[];
  /** The tag vocabulary, for the free choice of tags. */
  vocabulary: Tag[];
  preferences: Preferences;
  /** Persists preferences (the last export destination). */
  onPreferences: (next: Preferences) => void;
};

const COMPRESSION_KEYS: readonly {
  value: Compression;
  labelKey:
    | "exportUi.compressionNone"
    | "exportUi.compressionFast"
    | "exportUi.compressionBalanced"
    | "exportUi.compressionSmallest";
  hintKey:
    | "exportUi.compressionNoneHint"
    | "exportUi.compressionFastHint"
    | "exportUi.compressionBalancedHint"
    | "exportUi.compressionSmallestHint";
}[] = [
  {
    value: "none",
    labelKey: "exportUi.compressionNone",
    hintKey: "exportUi.compressionNoneHint",
  },
  {
    value: "fast",
    labelKey: "exportUi.compressionFast",
    hintKey: "exportUi.compressionFastHint",
  },
  {
    value: "balanced",
    labelKey: "exportUi.compressionBalanced",
    hintKey: "exportUi.compressionBalancedHint",
  },
  {
    value: "smallest",
    labelKey: "exportUi.compressionSmallest",
    hintKey: "exportUi.compressionSmallestHint",
  },
];

const TERMINAL: readonly ExportState[] = ["done", "failed", "cancelled"];

function stateLabel(t: TFunction, state: ExportState): string {
  switch (state) {
    case "preparing":
      return t("exportUi.preparing");
    case "copying":
      return t("exportUi.copying");
    case "encrypting":
      return t("exportUi.encrypting");
    case "done":
      return t("commonExtra.done");
    case "failed":
      return t("exportUi.failed");
    case "cancelled":
      return t("exportUi.cancelled");
  }
}

/**
 * Backup (Library menu) and Export (folder and tag context menus) in one dialog: what to
 * export, in which format, where, and with what protection; then progress and the result.
 */
export function ExportDialog({
  open,
  onOpenChange,
  library,
  target,
  folders,
  vocabulary,
  preferences,
  onPreferences,
}: Props) {
  const { t, i18n } = useTranslation();
  /** The free-choice scope; a preset target fixes the scope. */
  const [scope, setScope] = useState<"whole" | "folders" | "tags">("whole");
  const [selectedFolders, setSelectedFolders] = useState<ReadonlySet<string>>(new Set());
  const [selectedTags, setSelectedTags] = useState<ReadonlySet<string>>(new Set());
  const [includeSubfolders, setIncludeSubfolders] = useState(true);
  const [formatKind, setFormatKind] = useState<"mirror" | "zip" | "eagle">("zip");
  const [compression, setCompression] = useState<Compression>("balanced");
  const [encrypt, setEncrypt] = useState(false);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [destination, setDestination] = useState("");
  /** Into a running Eagle (docs/spec.md §8.4). */
  const [eagleHost, setEagleHost] = useState("127.0.0.1");
  const [eaglePort, setEaglePort] = useState("41595");
  const [eagleToken, setEagleToken] = useState("");
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  /** The export the dialog follows; separate state so the listener does not re-subscribe per event. */
  const [runId, setRunId] = useState<number | null>(null);
  const [startError, setStartError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setScope("whole");
      setSelectedFolders(new Set());
      setSelectedTags(new Set());
      setIncludeSubfolders(true);
      setFormatKind("zip");
      setCompression("balanced");
      setEncrypt(false);
      setPassword("");
      setConfirm("");
      setDestination(preferences.exportDestination ?? "");
      setEagleToken("");
      setProgress(null);
      setRunId(null);
      setStartError(null);
      ipc
        .connectionDefaults()
        .then((defaults) => {
          setEagleHost(defaults.host);
          setEaglePort(String(defaults.port));
        })
        .catch(() => {});
    }
  }, [open, preferences.exportDestination]);

  const selection: ExportSelection = useMemo(
    () =>
      target?.kind === "folder"
        ? { whole: false, folders: [target.id], tags: [] }
        : target?.kind === "tag"
          ? { whole: false, folders: [], tags: [target.name] }
          : scope === "whole"
            ? { whole: true, folders: [], tags: [] }
            : scope === "folders"
              ? { whole: false, folders: [...selectedFolders], tags: [] }
              : { whole: false, folders: [], tags: [...selectedTags] },
    [target, scope, selectedFolders, selectedTags],
  );
  const chosen = selection.whole || selection.folders.length > 0 || selection.tags.length > 0;
  const running = progress !== null && !TERMINAL.includes(progress.state);

  const preview = useQuery({
    queryKey: ["exportPreview", selection, includeSubfolders],
    queryFn: () => ipc.exportPreview(selection, includeSubfolders),
    enabled: open && inTauri() && !running && chosen,
    staleTime: 5000,
  });

  useEffect(() => {
    if (!open || !inTauri() || runId === null) return;
    const id = runId;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<ExportProgress>(EVENTS.exportProgress, (event) => {
      if (event.payload.id === id) setProgress(event.payload);
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
  }, [open, runId]);

  const toggle = (set: ReadonlySet<string>, value: string, on: boolean) => {
    const next = new Set(set);
    if (on) next.add(value);
    else next.delete(value);
    return next;
  };

  const pickDestination = () => {
    void openDialog({ directory: true, defaultPath: destination || undefined }).then((path) => {
      if (typeof path === "string") setDestination(path);
    });
  };

  const problems = (): string | null => {
    if (!chosen) return t("exportUi.chooseWhat");
    if (formatKind === "eagle") {
      const errors = validateConnectForm({ host: eagleHost, port: eaglePort, token: eagleToken });
      if (!isValid(errors)) return errors.host ?? errors.port ?? t("errors.generic");
    } else if (destination.trim() === "") {
      return t("exportUi.chooseWhere");
    }
    if (formatKind === "zip" && encrypt) {
      if (password === "") return t("exportUi.typePassword");
      if (password !== confirm) return t("exportUi.passwordMismatch");
    }
    if (preview.data && preview.data.items === 0) return t("exportUi.noItems");
    return null;
  };

  const start = () => {
    const problem = problems();
    if (problem) {
      setStartError(problem);
      return;
    }
    setStartError(null);
    const intoEagle = formatKind === "eagle";
    const request: ExportRequest = {
      selection,
      includeSubfolders:
        target?.kind === "folder" || scope === "folders" ? includeSubfolders : false,
      format: formatKind === "zip" ? { kind: "zip", compression } : { kind: "mirror" },
      destination: intoEagle ? "" : destination.trim(),
      encrypt: formatKind === "zip" && encrypt ? password : null,
      eagle: intoEagle
        ? {
            host: eagleHost.trim(),
            port: Number(eaglePort.trim()),
            token: eagleToken || null,
          }
        : null,
    };
    ipc
      .startExport(request)
      .then((first) => {
        setProgress(first);
        setRunId(first.id);
        if (!intoEagle && destination.trim() !== preferences.exportDestination) {
          onPreferences({ ...preferences, exportDestination: destination.trim() });
        }
      })
      .catch((failure: unknown) => setStartError(describeError(failure)));
  };

  const close = (next: boolean) => {
    if (running) return;
    onOpenChange(next);
  };

  const title =
    target?.kind === "folder"
      ? t("exportUi.titleExportFolder")
      : target?.kind === "tag"
        ? t("exportUi.titleExportTag")
        : t("exportUi.titleBackup");

  const scopes = [
    ["whole", t("exportUi.entireLibrary")],
    ["folders", t("exportUi.folders")],
    ["tags", t("exportUi.tags")],
  ] as const;

  const formats = [
    ["mirror", t("exportUi.mirror")],
    ["zip", t("exportUi.zip")],
    ["eagle", t("exportUi.intoEagle")],
  ] as const;

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="grid max-h-[85vh] grid-cols-[minmax(0,1fr)] gap-4 overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{library.name}</DialogDescription>
        </DialogHeader>

        {progress === null ? (
          <>
            {target === null && (
              <section className="grid gap-1.5">
                <Label>{t("exportUi.what")}</Label>
                <div className="flex gap-1">
                  {scopes.map(([value, label]) => (
                    <label
                      key={value}
                      className={
                        scope === value
                          ? "rounded-md border border-primary bg-primary px-3 py-1 text-[13px] text-primary-foreground"
                          : "rounded-md border px-3 py-1 text-[13px] hover:bg-muted"
                      }
                    >
                      <input
                        type="radio"
                        name="export-scope"
                        value={value}
                        className="sr-only"
                        checked={scope === value}
                        onChange={() => setScope(value)}
                      />
                      {label}
                    </label>
                  ))}
                </div>
                {scope === "folders" && (
                  <FolderChoice
                    folders={folders}
                    selected={selectedFolders}
                    onToggle={(id, on) => setSelectedFolders((s) => toggle(s, id, on))}
                  />
                )}
                {scope === "tags" && (
                  <TagChoice
                    vocabulary={vocabulary}
                    selected={selectedTags}
                    onToggle={(name, on) => setSelectedTags((s) => toggle(s, name, on))}
                  />
                )}
                {scope === "folders" && (
                  <div className="flex items-center gap-2 text-[13px]">
                    <Switch
                      id="export-subfolders"
                      checked={includeSubfolders}
                      onCheckedChange={setIncludeSubfolders}
                    />
                    <Label htmlFor="export-subfolders">{t("foldersUi.includeSubfolders")}</Label>
                  </div>
                )}
              </section>
            )}
            {target !== null && (
              <section className="grid gap-1.5">
                <Label>{t("exportUi.what")}</Label>
                <p className="text-[13px]">
                  {target.kind === "folder"
                    ? t("exportUi.folderNamed", { name: target.name })
                    : t("exportUi.tagNamed", { name: target.name })}
                  {target.kind === "folder" && (
                    <span className="ml-3 inline-flex items-center gap-2">
                      <Switch
                        id="export-preset-subfolders"
                        checked={includeSubfolders}
                        onCheckedChange={setIncludeSubfolders}
                      />
                      <Label htmlFor="export-preset-subfolders">
                        {t("foldersUi.includeSubfolders")}
                      </Label>
                    </span>
                  )}
                </p>
              </section>
            )}

            <section className="grid gap-1.5">
              <div aria-live="polite" className="text-[13px]">
                {preview.data ? (
                  <>
                    {t("exportUi.preview", {
                      items: preview.data.items.toLocaleString(i18n.language),
                      bytes: formatBytes(preview.data.bytes),
                    })}
                    {preview.data.hidden > 0 &&
                      t("exportUi.previewHidden", {
                        hidden: preview.data.hidden.toLocaleString(i18n.language),
                      })}
                  </>
                ) : preview.isFetching ? (
                  t("maintenance.measuring")
                ) : (
                  <span className="text-muted-foreground">
                    {chosen ? t("exportUi.nothingToExport") : t("exportUi.chooseWhat")}
                  </span>
                )}
              </div>
            </section>

            <section className="grid gap-1.5">
              <Label>{t("exportUi.format")}</Label>
              <div className="flex gap-1">
                {formats.map(([value, label]) => (
                  <label
                    key={value}
                    className={
                      formatKind === value
                        ? "rounded-md border border-primary bg-primary px-3 py-1 text-[13px] text-primary-foreground"
                        : "rounded-md border px-3 py-1 text-[13px] hover:bg-muted"
                    }
                  >
                    <input
                      type="radio"
                      name="export-format"
                      value={value}
                      className="sr-only"
                      checked={formatKind === value}
                      onChange={() => setFormatKind(value)}
                    />
                    {label}
                  </label>
                ))}
              </div>
              {formatKind === "zip" && (
                <div className="flex flex-wrap items-center gap-1">
                  {COMPRESSION_KEYS.map(({ value, labelKey, hintKey }) => (
                    <label
                      key={value}
                      title={t(hintKey)}
                      className={
                        compression === value
                          ? "rounded-md border border-primary bg-primary px-3 py-1 text-[13px] text-primary-foreground"
                          : "rounded-md border px-3 py-1 text-[13px] hover:bg-muted"
                      }
                    >
                      <input
                        type="radio"
                        name="export-compression"
                        value={value}
                        className="sr-only"
                        checked={compression === value}
                        onChange={() => setCompression(value)}
                      />
                      {t(labelKey)}
                    </label>
                  ))}
                </div>
              )}
              {formatKind === "zip" && (
                <div className="flex items-center gap-2 text-[13px]">
                  <Switch id="export-encrypt" checked={encrypt} onCheckedChange={setEncrypt} />
                  <Label htmlFor="export-encrypt">{t("exportUi.encrypt")}</Label>
                </div>
              )}
              {formatKind === "zip" && encrypt && (
                <div className="grid grid-cols-2 gap-2">
                  <Input
                    type="password"
                    placeholder={t("commonExtra.password")}
                    aria-label={t("commonExtra.password")}
                    value={password}
                    onChange={(event) => setPassword(event.target.value)}
                  />
                  <Input
                    type="password"
                    placeholder={t("locksUi.repeatPassword")}
                    aria-label={t("locksUi.repeatPassword")}
                    value={confirm}
                    onChange={(event) => setConfirm(event.target.value)}
                  />
                </div>
              )}
              {formatKind === "eagle" && (
                <div className="grid grid-cols-[1fr_7rem] gap-2">
                  <div className="grid gap-1">
                    <Label htmlFor="export-eagle-host">{t("connect.host")}</Label>
                    <Input
                      id="export-eagle-host"
                      value={eagleHost}
                      onChange={(event) => setEagleHost(event.target.value)}
                      autoComplete="off"
                      spellCheck={false}
                    />
                  </div>
                  <div className="grid gap-1">
                    <Label htmlFor="export-eagle-port">{t("connect.port")}</Label>
                    <Input
                      id="export-eagle-port"
                      inputMode="numeric"
                      value={eaglePort}
                      onChange={(event) => setEaglePort(event.target.value)}
                    />
                  </div>
                  <div className="col-span-2 grid gap-1">
                    <Label htmlFor="export-eagle-token">{t("connect.token")}</Label>
                    <Input
                      id="export-eagle-token"
                      type="password"
                      value={eagleToken}
                      onChange={(event) => setEagleToken(event.target.value)}
                      autoComplete="off"
                    />
                    <p className="text-xs text-muted-foreground">{t("connect.tokenHint")}</p>
                  </div>
                  <p className="col-span-2 text-xs text-muted-foreground">
                    {t("exportUi.eagleLockedWarning")}
                  </p>
                </div>
              )}
            </section>

            {formatKind !== "eagle" && (
              <section className="grid gap-1.5">
                <Label htmlFor="export-destination">{t("exportUi.where")}</Label>
                <div className="flex gap-2">
                  <Input
                    id="export-destination"
                    placeholder={t("exportUi.wherePlaceholder")}
                    value={destination}
                    onChange={(event) => setDestination(event.target.value)}
                  />
                  <Button variant="outline" size="sm" onClick={pickDestination}>
                    <FolderOpen aria-hidden />
                    {t("common.browse")}
                  </Button>
                </div>
              </section>
            )}

            {(startError || preview.isError) && (
              <p role="alert" className="text-sm text-destructive">
                {startError ?? describeError(preview.error)}
              </p>
            )}

            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => close(false)}>
                {t("common.cancel")}
              </Button>
              <Button
                disabled={!inTauri() || preview.isFetching || problems() !== null}
                title={problems() ?? undefined}
                onClick={start}
              >
                <HardDrive aria-hidden />
                {target === null ? t("exportUi.backUp") : t("exportUi.export")}
              </Button>
            </div>
          </>
        ) : (
          <ExportResult
            progress={progress}
            onCancel={() => progress && void ipc.cancelExport(progress.id)}
            onReveal={(path) => void ipc.revealPath(path)}
            onClose={() => close(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

/** The folder checkbox list of the free choice. */
function FolderChoice({
  folders,
  selected,
  onToggle,
}: {
  folders: Folder[];
  selected: ReadonlySet<string>;
  onToggle: (id: string, on: boolean) => void;
}) {
  const { t, i18n } = useTranslation();
  const rows = allRows(withoutLockedFolders(folders));
  if (rows.length === 0) {
    return <p className="text-xs text-muted-foreground">{t("foldersUi.emptyRead")}</p>;
  }
  return (
    <fieldset
      aria-label={t("exportUi.foldersToExport")}
      className="m-0 grid min-w-0 max-h-48 gap-px overflow-y-auto rounded-md border p-1"
    >
      {rows.map(({ folder, depth }) => {
        const color = folder.iconColor ? FOLDER_COLORS[folder.iconColor] : undefined;
        return (
          <div
            key={folder.id}
            className="flex h-7 cursor-default items-center gap-2 rounded px-1 text-[13px] hover:bg-sidebar-accent"
            style={{ paddingLeft: 8 + depth * 14 }}
          >
            <Checkbox
              checked={selected.has(folder.id)}
              onCheckedChange={(on) => onToggle(folder.id, on === true)}
              aria-label={folder.name}
            />
            {color ? (
              <span
                aria-hidden
                className="size-2.5 shrink-0 rounded-full"
                style={{ background: color }}
              />
            ) : null}
            <span className="min-w-0 flex-1 truncate">{folder.name}</span>
            <span className="text-xs text-muted-foreground tabular-nums">
              {(folder.descendantImageCount ?? folder.imageCount ?? 0).toLocaleString(
                i18n.language,
              )}
            </span>
          </div>
        );
      })}
    </fieldset>
  );
}

/** The tag checkbox list of the free choice, with a filter. */
function TagChoice({
  vocabulary,
  selected,
  onToggle,
}: {
  vocabulary: Tag[];
  selected: ReadonlySet<string>;
  onToggle: (name: string, on: boolean) => void;
}) {
  const { t } = useTranslation();
  const [filter, setFilter] = useState("");
  const needle = filter.trim().toLocaleLowerCase();
  const rows = vocabulary.filter(
    (tag) => needle === "" || tag.name.toLocaleLowerCase().includes(needle),
  );
  return (
    <div className="grid gap-1">
      <Input
        aria-label={t("exportUi.findTag")}
        placeholder={t("exportUi.findTag")}
        value={filter}
        onChange={(event) => setFilter(event.target.value)}
        className="h-7"
      />
      {rows.length === 0 ? (
        <p className="text-xs text-muted-foreground">
          {vocabulary.length === 0 ? t("tagsUi.noTagsYet") : t("tagsUi.noTagMatch")}
        </p>
      ) : (
        <fieldset
          aria-label={t("exportUi.tagsToExport")}
          className="m-0 grid min-w-0 max-h-48 gap-px overflow-y-auto rounded-md border p-1"
        >
          {rows.map((tag) => (
            <div
              key={tag.name}
              className="flex h-7 cursor-default items-center gap-2 rounded px-1 text-[13px] hover:bg-sidebar-accent"
            >
              <Checkbox
                checked={selected.has(tag.name)}
                onCheckedChange={(on) => onToggle(tag.name, on === true)}
                aria-label={tag.name}
              />
              <span className="min-w-0 flex-1 truncate">#{tag.name}</span>
              <span className="text-xs text-muted-foreground tabular-nums">
                {countText(tag.imageCount)}
              </span>
            </div>
          ))}
        </fieldset>
      )}
    </div>
  );
}

/** The progress bar while an export runs, and the result afterwards. */
function ExportResult({
  progress,
  onCancel,
  onReveal,
  onClose,
}: {
  progress: ExportProgress | null;
  onCancel: () => void;
  onReveal: (path: string) => void;
  onClose: () => void;
}) {
  const { t, i18n } = useTranslation();
  if (progress === null) return null;
  const running = !TERMINAL.includes(progress.state);
  const percent = progress.total > 0 ? (progress.done / progress.total) * 100 : 0;
  return (
    <section className="grid gap-3" aria-live="polite">
      {running && (
        <>
          <div className="grid gap-1.5">
            <p className="text-[13px]">
              {t("exportUi.progressCounted", {
                state: stateLabel(t, progress.state),
                done: progress.done.toLocaleString(i18n.language),
                total: progress.total.toLocaleString(i18n.language),
                bytesDone: formatBytes(progress.bytesDone),
                bytesTotal: formatBytes(progress.bytesTotal),
              })}
            </p>
            <Progress value={percent} />
            {progress.current && (
              <p className="truncate text-xs text-muted-foreground">{progress.current}</p>
            )}
          </div>
          <div className="flex justify-end">
            <Button variant="outline" onClick={onCancel}>
              {t("common.cancel")}
            </Button>
          </div>
        </>
      )}
      {progress.state === "done" && progress.summary && (
        <div className="grid gap-2">
          <p className="text-sm">
            {t("exportUi.exportedSummary", {
              items: progress.summary.items.toLocaleString(i18n.language),
              files: progress.summary.files.toLocaleString(i18n.language),
              bytes: formatBytes(progress.summary.bytes),
            })}
          </p>
          {progress.summary.missing.length > 0 && (
            <p className="text-sm text-muted-foreground">
              {t("exportUi.skippedMissing", {
                count: progress.summary.missing.length,
                list: `${progress.summary.missing.slice(0, 5).join(", ")}${
                  progress.summary.missing.length > 5 ? ", …" : ""
                }`,
              })}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => onReveal(progress.summary?.path ?? "")}>
              <FolderOpen aria-hidden />
              {t("details.showExplorer")}
            </Button>
            <Button onClick={onClose}>{t("commonExtra.close")}</Button>
          </div>
        </div>
      )}
      {progress.state === "failed" && (
        <div className="grid gap-2">
          <p role="alert" className="text-sm text-destructive">
            {progress.error ?? t("exportUi.exportFailed")}
          </p>
          <div className="flex justify-end">
            <Button onClick={onClose}>{t("commonExtra.close")}</Button>
          </div>
        </div>
      )}
      {progress.state === "cancelled" && (
        <div className="grid gap-2">
          <p className="text-sm text-muted-foreground">{t("exportUi.exportCancelled")}</p>
          <div className="flex justify-end">
            <Button onClick={onClose}>{t("commonExtra.close")}</Button>
          </div>
        </div>
      )}
    </section>
  );
}
