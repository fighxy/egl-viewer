import { useQuery } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen, HardDrive } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
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

const COMPRESSIONS: readonly { value: Compression; label: string; title: string }[] = [
  { value: "none", label: "None", title: "Store the files uncompressed; fastest" },
  { value: "fast", label: "Fast", title: "Deflate, low effort" },
  { value: "balanced", label: "Balanced", title: "Deflate, the usual level" },
  { value: "smallest", label: "Smallest", title: "Deflate, high effort" },
];

const TERMINAL: readonly ExportState[] = ["done", "failed", "cancelled"];

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
  /** The free-choice scope; a preset target fixes the scope. */
  const [scope, setScope] = useState<"whole" | "folders" | "tags">("whole");
  const [selectedFolders, setSelectedFolders] = useState<ReadonlySet<string>>(new Set());
  const [selectedTags, setSelectedTags] = useState<ReadonlySet<string>>(new Set());
  const [includeSubfolders, setIncludeSubfolders] = useState(true);
  const [formatKind, setFormatKind] = useState<"mirror" | "zip">("zip");
  const [compression, setCompression] = useState<Compression>("balanced");
  const [encrypt, setEncrypt] = useState(false);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [destination, setDestination] = useState("");
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
      setProgress(null);
      setRunId(null);
      setStartError(null);
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
    if (!chosen) return "Choose what to export.";
    if (destination.trim() === "") return "Choose where to export.";
    if (formatKind === "zip" && encrypt) {
      if (password === "") return "Type a password.";
      if (password !== confirm) return "The passwords do not match.";
    }
    if (preview.data && preview.data.items === 0) return "The selection has no items.";
    return null;
  };

  const start = () => {
    const problem = problems();
    if (problem) {
      setStartError(problem);
      return;
    }
    setStartError(null);
    const request: ExportRequest = {
      selection,
      includeSubfolders:
        target?.kind === "folder" || scope === "folders" ? includeSubfolders : false,
      format: formatKind === "zip" ? { kind: "zip", compression } : { kind: "mirror" },
      destination: destination.trim(),
      encrypt: formatKind === "zip" && encrypt ? password : null,
    };
    ipc
      .startExport(request)
      .then((first) => {
        setProgress(first);
        setRunId(first.id);
        if (destination.trim() !== preferences.exportDestination) {
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
    target?.kind === "folder" ? "Export Folder" : target?.kind === "tag" ? "Export Tag" : "Backup";

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
                <Label>What</Label>
                <div className="flex gap-1">
                  {(
                    [
                      ["whole", "Entire library"],
                      ["folders", "Folders"],
                      ["tags", "Tags"],
                    ] as const
                  ).map(([value, label]) => (
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
                    <Label htmlFor="export-subfolders">Include subfolders</Label>
                  </div>
                )}
              </section>
            )}
            {target !== null && (
              <section className="grid gap-1.5">
                <Label>What</Label>
                <p className="text-[13px]">
                  {target.kind === "folder" ? "Folder" : "Tag"} “{target.name}”
                  {target.kind === "folder" && (
                    <span className="ml-3 inline-flex items-center gap-2">
                      <Switch
                        id="export-preset-subfolders"
                        checked={includeSubfolders}
                        onCheckedChange={setIncludeSubfolders}
                      />
                      <Label htmlFor="export-preset-subfolders">Include subfolders</Label>
                    </span>
                  )}
                </p>
              </section>
            )}

            <section className="grid gap-1.5">
              <div aria-live="polite" className="text-[13px]">
                {preview.data ? (
                  <>
                    {preview.data.items.toLocaleString("en")} items ·{" "}
                    {formatBytes(preview.data.bytes)}
                    {preview.data.hidden > 0 &&
                      ` · ${preview.data.hidden.toLocaleString("en")} hidden in locked folders`}
                  </>
                ) : preview.isFetching ? (
                  "Measuring…"
                ) : (
                  <span className="text-muted-foreground">
                    {chosen ? "Nothing to export." : "Choose what to export."}
                  </span>
                )}
              </div>
            </section>

            <section className="grid gap-1.5">
              <Label>Format</Label>
              <div className="flex gap-1">
                {(
                  [
                    ["mirror", "Files in folders"],
                    ["zip", "ZIP archive"],
                  ] as const
                ).map(([value, label]) => (
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
                  {COMPRESSIONS.map(({ value, label, title }) => (
                    <label
                      key={value}
                      title={title}
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
                      {label}
                    </label>
                  ))}
                </div>
              )}
              {formatKind === "zip" && (
                <div className="flex items-center gap-2 text-[13px]">
                  <Switch id="export-encrypt" checked={encrypt} onCheckedChange={setEncrypt} />
                  <Label htmlFor="export-encrypt">Encrypt with a password</Label>
                </div>
              )}
              {formatKind === "zip" && encrypt && (
                <div className="grid grid-cols-2 gap-2">
                  <Input
                    type="password"
                    placeholder="Password"
                    aria-label="Password"
                    value={password}
                    onChange={(event) => setPassword(event.target.value)}
                  />
                  <Input
                    type="password"
                    placeholder="Repeat the password"
                    aria-label="Repeat the password"
                    value={confirm}
                    onChange={(event) => setConfirm(event.target.value)}
                  />
                </div>
              )}
            </section>

            <section className="grid gap-1.5">
              <Label htmlFor="export-destination">Where</Label>
              <div className="flex gap-2">
                <Input
                  id="export-destination"
                  placeholder="A folder on this PC"
                  value={destination}
                  onChange={(event) => setDestination(event.target.value)}
                />
                <Button variant="outline" size="sm" onClick={pickDestination}>
                  <FolderOpen aria-hidden />
                  Browse…
                </Button>
              </div>
            </section>

            {(startError || preview.isError) && (
              <p role="alert" className="text-sm text-destructive">
                {startError ?? describeError(preview.error)}
              </p>
            )}

            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => close(false)}>
                Cancel
              </Button>
              <Button
                disabled={!inTauri() || preview.isFetching || problems() !== null}
                title={problems() ?? undefined}
                onClick={start}
              >
                <HardDrive aria-hidden />
                {target === null ? "Back up" : "Export"}
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
  const rows = allRows(withoutLockedFolders(folders));
  if (rows.length === 0) {
    return <p className="text-xs text-muted-foreground">No folders.</p>;
  }
  return (
    <fieldset
      aria-label="Folders to export"
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
              {(folder.descendantImageCount ?? folder.imageCount ?? 0).toLocaleString("en")}
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
  const [filter, setFilter] = useState("");
  const needle = filter.trim().toLocaleLowerCase();
  const rows = vocabulary.filter(
    (tag) => needle === "" || tag.name.toLocaleLowerCase().includes(needle),
  );
  return (
    <div className="grid gap-1">
      <Input
        aria-label="Find a tag"
        placeholder="Find a tag"
        value={filter}
        onChange={(event) => setFilter(event.target.value)}
        className="h-7"
      />
      {rows.length === 0 ? (
        <p className="text-xs text-muted-foreground">
          {vocabulary.length === 0 ? "No tags yet." : "No tag has that name."}
        </p>
      ) : (
        <fieldset
          aria-label="Tags to export"
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

const STATE_TEXT: Record<ExportState, string> = {
  preparing: "Preparing…",
  copying: "Copying…",
  encrypting: "Encrypting…",
  done: "Done",
  failed: "Failed",
  cancelled: "Cancelled",
};

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
  if (progress === null) return null;
  const running = !TERMINAL.includes(progress.state);
  const percent = progress.total > 0 ? (progress.done / progress.total) * 100 : 0;
  return (
    <section className="grid gap-3" aria-live="polite">
      {running && (
        <>
          <div className="grid gap-1.5">
            <p className="text-[13px]">
              {STATE_TEXT[progress.state]} {progress.done.toLocaleString("en")} of{" "}
              {progress.total.toLocaleString("en")} · {formatBytes(progress.bytesDone)} of{" "}
              {formatBytes(progress.bytesTotal)}
            </p>
            <Progress value={percent} />
            {progress.current && (
              <p className="truncate text-xs text-muted-foreground">{progress.current}</p>
            )}
          </div>
          <div className="flex justify-end">
            <Button variant="outline" onClick={onCancel}>
              Cancel
            </Button>
          </div>
        </>
      )}
      {progress.state === "done" && progress.summary && (
        <div className="grid gap-2">
          <p className="text-sm">
            Exported {progress.summary.items.toLocaleString("en")} items:{" "}
            {progress.summary.files.toLocaleString("en")} files,{" "}
            {formatBytes(progress.summary.bytes)}.
          </p>
          {progress.summary.missing.length > 0 && (
            <p className="text-sm text-muted-foreground">
              Skipped {progress.summary.missing.length} missing:{" "}
              {progress.summary.missing.slice(0, 5).join(", ")}
              {progress.summary.missing.length > 5 ? ", …" : ""}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => onReveal(progress.summary?.path ?? "")}>
              <FolderOpen aria-hidden />
              Show in Explorer
            </Button>
            <Button onClick={onClose}>Close</Button>
          </div>
        </div>
      )}
      {progress.state === "failed" && (
        <div className="grid gap-2">
          <p role="alert" className="text-sm text-destructive">
            {progress.error ?? "The export failed."}
          </p>
          <div className="flex justify-end">
            <Button onClick={onClose}>Close</Button>
          </div>
        </div>
      )}
      {progress.state === "cancelled" && (
        <div className="grid gap-2">
          <p className="text-sm text-muted-foreground">
            The export was cancelled; the partial output was removed.
          </p>
          <div className="flex justify-end">
            <Button onClick={onClose}>Close</Button>
          </div>
        </div>
      )}
    </section>
  );
}
