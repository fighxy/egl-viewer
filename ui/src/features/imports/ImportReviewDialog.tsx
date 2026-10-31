import { ChevronDown, ChevronRight, File as FileIcon, Folder as FolderIcon } from "lucide-react";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { formatBytes } from "@/features/browse/format";
import type { ImportPreview, PreviewEntry } from "@/ipc";
import { plural } from "./importText";

export type ImportChoice = { exclude: string[]; include: string[] };

type Props = {
  /** The picked folder and what the walk found; `null` closes the dialog. */
  review: { path: string; preview: ImportPreview } | null;
  /** Where the files go: a folder name, or the library. */
  target: string | null;
  /** `null` when the user cancelled: nothing is copied. */
  onConfirm: (choice: ImportChoice | null) => void;
};

type CheckState = "checked" | "mixed" | "unchecked";

/** `parent` with `name` below it; the picked folder's children have no prefix. */
function relJoin(parent: string, name: string): string {
  return parent === "" ? name : `${parent}/${name}`;
}

/** Every entry under `entry`, with its relative path. */
function below(
  entry: PreviewEntry,
  rel: string,
  visit: (rel: string, entry: PreviewEntry) => void,
) {
  for (const child of entry.children) {
    const childRel = relJoin(rel, child.name);
    visit(childRel, child);
    below(child, childRel, visit);
  }
}

/** True when `rel` or a folder above it was unchecked (compact exclude set). */
function excluded(rel: string, off: ReadonlySet<string>): boolean {
  if (off.has("")) return true;
  if (off.has(rel)) return true;
  for (let slash = rel.indexOf("/"); slash >= 0; slash = rel.indexOf("/", slash + 1)) {
    if (off.has(rel.slice(0, slash))) return true;
  }
  return false;
}

function fileSelected(
  entry: PreviewEntry,
  rel: string,
  off: ReadonlySet<string>,
  on: ReadonlySet<string>,
) {
  return !excluded(rel, off) && (entry.image || on.has(rel));
}

/** A folder is checked when everything below it is, unchecked when nothing is. */
function folderState(
  entry: PreviewEntry,
  rel: string,
  off: ReadonlySet<string>,
  on: ReadonlySet<string>,
): CheckState {
  if (excluded(rel, off)) return "unchecked";
  if (entry.children.length === 0) return "checked";
  let any = false;
  let all = true;
  for (const child of entry.children) {
    const childRel = relJoin(rel, child.name);
    const state = child.folder
      ? folderState(child, childRel, off, on)
      : fileSelected(child, childRel, off, on)
        ? "checked"
        : "unchecked";
    if (state !== "checked") all = false;
    if (state !== "unchecked") any = true;
  }
  return all ? "checked" : any ? "mixed" : "unchecked";
}

/**
 * The reviewed import of one picked folder (docs/spec.md §4): what is inside, with images
 * preselected and everything else a click away. Nothing is copied until Import.
 */
export function ImportReviewDialog({ review, target, onConfirm }: Props) {
  // Unchecked folders and image files; checked non-image files. Overrides below an unchecked
  // folder are dropped, so the sets stay compact.
  const [off, setOff] = useState<ReadonlySet<string>>(() => new Set());
  const [on, setOn] = useState<ReadonlySet<string>>(() => new Set());
  const [open, setOpen] = useState<ReadonlySet<string>>(() => new Set([""]));

  const preview = review?.preview ?? null;

  const selected = useMemo(() => {
    if (!preview) return { files: 0, bytes: 0 };
    let files = 0;
    let bytes = 0;
    const count = (entry: PreviewEntry, rel: string) => {
      if (!entry.folder && fileSelected(entry, rel, off, on)) {
        files += 1;
        bytes += entry.bytes;
      }
      for (const child of entry.children) count(child, relJoin(rel, child.name));
    };
    for (const entry of preview.tree) count(entry, "");
    return { files, bytes };
  }, [preview, off, on]);

  if (!review || !preview) return null;
  const { path } = review;

  const apply = (update: (off: Set<string>, on: Set<string>) => void) => {
    const nextOff = new Set(off);
    const nextOn = new Set(on);
    update(nextOff, nextOn);
    setOff(nextOff);
    setOn(nextOn);
  };

  const clearUnder = (sets: Set<string>[], parent: string) => {
    for (const set of sets) {
      for (const key of [...set]) {
        if (key.startsWith(`${parent}/`)) set.delete(key);
      }
    }
  };

  const toggleOpen = (rel: string) => {
    setOpen((current) => {
      const next = new Set(current);
      if (next.has(rel)) next.delete(rel);
      else next.add(rel);
      return next;
    });
  };

  const toggleFile = (entry: PreviewEntry, rel: string) =>
    apply((offSet, onSet) => {
      if (entry.image) {
        if (offSet.has(rel)) offSet.delete(rel);
        else offSet.add(rel);
      } else if (onSet.has(rel)) onSet.delete(rel);
      else onSet.add(rel);
    });

  const toggleFolder = (entry: PreviewEntry, rel: string) =>
    apply((offSet, onSet) => {
      clearUnder([offSet, onSet], rel);
      if (folderState(entry, rel, offSet, onSet) === "checked") {
        offSet.add(rel);
        return;
      }
      offSet.delete(rel);
      // Checking a folder selects everything listed below it.
      below(entry, rel, (childRel, child) => {
        if (!child.folder && !child.image) onSet.add(childRel);
      });
    });

  const name = path.replaceAll("\\", "/").split("/").filter(Boolean).at(-1) ?? path;

  return (
    <Dialog open onOpenChange={(isOpen) => !isOpen && onConfirm(null)}>
      <DialogContent className="flex max-h-[80vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Import “{name}”</DialogTitle>
          <DialogDescription>
            {plural(preview.files, "file")} · {formatBytes(preview.bytes)}
            {preview.unlisted > 0 &&
              ` · ${plural(preview.unlisted, "file")} more not listed; they follow their folders`}
            {target ? ` · into “${target}”` : " · into the library"}
          </DialogDescription>
        </DialogHeader>
        <div
          role="tree"
          aria-label="Folder contents"
          className="min-h-0 flex-1 overflow-y-auto rounded-md border p-1"
        >
          {preview.tree.map((entry) => (
            <Node
              key={entry.name}
              entry={entry}
              rel=""
              depth={0}
              off={off}
              on={on}
              open={open}
              onToggleOpen={toggleOpen}
              onToggleFile={toggleFile}
              onToggleFolder={toggleFolder}
            />
          ))}
        </div>
        <div className="flex items-center gap-2 pt-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              setOff(new Set());
              setOn(new Set());
            }}
          >
            Images only
          </Button>
          <span className="ml-auto text-xs text-muted-foreground">
            {plural(selected.files, "file")} · {formatBytes(selected.bytes)} selected
          </span>
          <Button
            size="sm"
            disabled={selected.files === 0}
            onClick={() => onConfirm({ exclude: [...off], include: [...on] })}
          >
            Import
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

type NodeProps = {
  entry: PreviewEntry;
  rel: string;
  depth: number;
  off: ReadonlySet<string>;
  on: ReadonlySet<string>;
  open: ReadonlySet<string>;
  onToggleOpen: (rel: string) => void;
  onToggleFile: (entry: PreviewEntry, rel: string) => void;
  onToggleFolder: (entry: PreviewEntry, rel: string) => void;
};

function Node({
  entry,
  rel,
  depth,
  off,
  on,
  open,
  onToggleOpen,
  onToggleFile,
  onToggleFolder,
}: NodeProps) {
  const checked: CheckState = entry.folder
    ? folderState(entry, rel, off, on)
    : fileSelected(entry, rel, off, on)
      ? "checked"
      : "unchecked";
  const expanded = open.has(rel);
  return (
    <div role="treeitem" tabIndex={0} aria-expanded={entry.folder ? expanded : undefined}>
      <div
        className="flex h-7 items-center gap-1.5 rounded px-1 text-[13px] hover:bg-accent"
        style={{ paddingLeft: depth * 16 + 4 }}
      >
        {entry.folder && entry.children.length > 0 ? (
          <button
            type="button"
            aria-label={expanded ? `Collapse ${entry.name}` : `Expand ${entry.name}`}
            className="grid size-4 place-items-center rounded text-muted-foreground hover:text-foreground"
            onClick={() => onToggleOpen(rel)}
          >
            {expanded ? (
              <ChevronDown className="size-3.5" />
            ) : (
              <ChevronRight className="size-3.5" />
            )}
          </button>
        ) : (
          <span className="size-4" />
        )}
        <Checkbox
          aria-label={entry.name}
          checked={checked === "checked" ? true : checked === "mixed" ? "indeterminate" : false}
          onCheckedChange={() =>
            entry.folder ? onToggleFolder(entry, rel) : onToggleFile(entry, rel)
          }
        />
        {entry.folder ? (
          <FolderIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        ) : (
          <FileIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        )}
        <span className="min-w-0 flex-1 truncate">{entry.name}</span>
        {!entry.image && !entry.folder && (
          <span className="text-xs text-muted-foreground">not an image</span>
        )}
        <span className="text-xs text-muted-foreground tabular-nums">
          {formatBytes(entry.bytes)}
        </span>
      </div>
      {entry.folder && expanded && (
        // Tree children group (ARIA); not a form fieldset.
        // biome-ignore lint/a11y/useSemanticElements: role="group" is the tree pattern
        <div role="group">
          {entry.children.map((child) => (
            <Node
              key={child.name}
              entry={child}
              rel={relJoin(rel, child.name)}
              depth={depth + 1}
              off={off}
              on={on}
              open={open}
              onToggleOpen={onToggleOpen}
              onToggleFile={onToggleFile}
              onToggleFolder={onToggleFolder}
            />
          ))}
        </div>
      )}
    </div>
  );
}
