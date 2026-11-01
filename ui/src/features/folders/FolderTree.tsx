import { cn } from "cn";
import {
  ChevronDown,
  ChevronRight,
  Folder as FolderIcon,
  FolderTree as Nested,
  Plus,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { DROP_INSIDE_CLASS, dropHint, useDrag } from "@/features/dnd/drag";
import type { Folder } from "@/ipc";
import { FOLDER_COLORS, folderCount, visibleRows, withoutLockedFolders } from "./tree";

type Props = {
  folders: Folder[];
  /** The open folder. */
  selectedId: string | null;
  includeSubfolders: boolean;
  /** `true`, or why folders cannot change (read-only library). */
  writable: true | string;
  expanded: ReadonlySet<string>;
  onExpandedChange: (id: string, open: boolean) => void;
  onSelect: (id: string) => void;
  onIncludeSubfoldersChange: () => void;
  onCreate: (parent: string | null) => void;
  onRename: (id: string, name: string) => void;
  onColor: (id: string, color: string | null) => void;
  onMoveTo: (id: string) => void;
  onDelete: (id: string) => void;
  /** Export the folder (and its subfolders) to a folder tree or an archive. */
  onExport: (id: string, name: string) => void;
  /** A running Eagle: the API has no folder delete and cannot set a password. */
  eagle?: boolean;
  /** Lock… on an unlocked folder (other lock commands live under Locked folders, D-059). */
  onLock?: (id: string, action: LockAction) => void;
};

/** What the folder menu can do with a lock. */
export type LockAction =
  | "lock"
  | "unlock"
  | "relock"
  | "change"
  | "renew"
  | "remove"
  | "yubikey"
  | "unyubikey";

/**
 * The Folders section of the left bar (docs/spec.md §7.3): the tree with colour dots and
 * counts, the Include subfolders switch, and folder commands from a context menu. Locked
 * folders are omitted here and listed under Locked folders (D-059). Arrow keys and Enter move
 * through the rows.
 */
export function FolderTree({
  folders,
  selectedId,
  includeSubfolders,
  writable,
  expanded,
  onExpandedChange,
  onSelect,
  onIncludeSubfoldersChange,
  onCreate,
  onRename,
  onColor,
  onMoveTo,
  onDelete,
  onExport,
  eagle = false,
  onLock,
}: Props) {
  const { begin, active } = useDrag();
  const rows = visibleRows(withoutLockedFolders(folders), expanded);
  const [focused, setFocused] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());
  const { t } = useTranslation();
  const canWrite = writable === true;
  const focusId =
    focused && rows.some((row) => row.folder.id === focused)
      ? focused
      : (selectedId ?? rows[0]?.folder.id ?? null);

  useEffect(() => {
    if (focused) rowRefs.current.get(focused)?.focus();
  }, [focused]);

  const onKeyDown = (event: React.KeyboardEvent, index: number) => {
    const row = rows[index];
    if (!row || renaming) return;
    const move = (to: number) => {
      const next = rows[Math.max(0, Math.min(rows.length - 1, to))];
      if (next) setFocused(next.folder.id);
    };
    switch (event.key) {
      case "ArrowDown":
        move(index + 1);
        break;
      case "ArrowUp":
        move(index - 1);
        break;
      case "ArrowRight":
        if (row.hasChildren && !expanded.has(row.folder.id)) onExpandedChange(row.folder.id, true);
        else move(index + 1);
        break;
      case "ArrowLeft":
        if (expanded.has(row.folder.id)) onExpandedChange(row.folder.id, false);
        else {
          // The nearest row above with a smaller depth is the parent.
          for (let above = index - 1; above >= 0; above -= 1) {
            const candidate = rows[above];
            if (candidate && candidate.depth < row.depth) {
              setFocused(candidate.folder.id);
              break;
            }
          }
        }
        break;
      case "Enter":
        onSelect(row.folder.id);
        break;
      case "Home":
        move(0);
        break;
      case "End":
        move(rows.length - 1);
        break;
      case "F2":
        if (canWrite) setRenaming(row.folder.id);
        break;
      case "Delete":
        // Eagle's API cannot delete folders (docs/spec.md §8.3 "Writes").
        if (canWrite && !eagle) onDelete(row.folder.id);
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  return (
    <section aria-label={t("sidebar.folders")} className="grid gap-px">
      <div
        data-drop="root"
        className={cn(
          "flex h-7 items-center gap-1 rounded-md pr-1 pl-2 text-xs font-medium text-muted-foreground",
          dropHint(active, (target) => target.kind === "root") && "bg-primary/15",
        )}
      >
        <span className="flex-1">{t("sidebar.folders")}</span>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="New folder"
          title={canWrite ? "New folder (Ctrl+Shift+N)" : writable}
          disabled={!canWrite}
          onClick={() => onCreate(null)}
        >
          <Plus aria-hidden />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Include subfolders"
          aria-pressed={includeSubfolders}
          title="Include subfolders: a folder also shows the items of its subfolders"
          className="aria-pressed:bg-sidebar-accent aria-pressed:text-foreground"
          onClick={onIncludeSubfoldersChange}
        >
          <Nested aria-hidden />
        </Button>
      </div>
      {rows.length === 0 ? (
        <p className="px-2 py-1 text-xs text-muted-foreground">
          {canWrite ? "No folders yet. Create one with +." : "No folders."}
        </p>
      ) : (
        <div role="tree" aria-label="Folder tree" className="grid gap-px">
          {rows.map(({ folder, depth, hasChildren }, index) => {
            const open = expanded.has(folder.id);
            const hint = dropHint(
              active,
              (target) => target.kind === "folder" && target.id === folder.id,
            );
            const zone = hint?.kind === "folder" ? hint.zone : null;
            const count = folderCount(folder, includeSubfolders);
            const color = folder.iconColor ? FOLDER_COLORS[folder.iconColor] : undefined;
            return (
              <ContextMenu key={folder.id}>
                <ContextMenuTrigger asChild>
                  <div
                    ref={(element) => {
                      if (element) rowRefs.current.set(folder.id, element);
                      else rowRefs.current.delete(folder.id);
                    }}
                    role="treeitem"
                    aria-level={depth + 1}
                    aria-expanded={hasChildren ? open : undefined}
                    aria-selected={folder.id === selectedId}
                    aria-label={folder.name}
                    tabIndex={folder.id === focusId ? 0 : -1}
                    data-drop="folder"
                    data-id={folder.id}
                    className={cn(
                      "relative flex h-7 cursor-default items-center gap-1.5 rounded-md pr-2 text-[13px] outline-none select-none hover:bg-sidebar-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-selected:bg-sidebar-accent aria-selected:font-medium",
                      zone === "inside" && DROP_INSIDE_CLASS,
                      zone === "before" && "shadow-[inset_0_2px_0_var(--color-primary)]",
                      zone === "after" && "shadow-[inset_0_-2px_0_var(--color-primary)]",
                    )}
                    style={{ paddingLeft: 4 + depth * 14 }}
                    onClick={() => {
                      setFocused(folder.id);
                      onSelect(folder.id);
                    }}
                    onKeyDown={(event) => onKeyDown(event, index)}
                    onMouseDown={(event) => {
                      if (renaming === folder.id) return;
                      begin(event, () =>
                        canWrite ? { kind: "folder", id: folder.id, label: folder.name } : null,
                      );
                    }}
                    onDoubleClick={() => canWrite && setRenaming(folder.id)}
                  >
                    {hasChildren ? (
                      <button
                        type="button"
                        tabIndex={-1}
                        aria-label={open ? `Collapse ${folder.name}` : `Expand ${folder.name}`}
                        className="grid size-4 place-items-center rounded text-muted-foreground hover:text-foreground"
                        onClick={(event) => {
                          event.stopPropagation();
                          onExpandedChange(folder.id, !open);
                        }}
                        onMouseDown={(event) => event.stopPropagation()}
                      >
                        {open ? (
                          <ChevronDown className="size-3.5" />
                        ) : (
                          <ChevronRight className="size-3.5" />
                        )}
                      </button>
                    ) : (
                      <span className="size-4" />
                    )}
                    {color ? (
                      <span
                        aria-hidden
                        className="ml-0.5 size-2.5 shrink-0 rounded-full"
                        style={{ background: color }}
                      />
                    ) : (
                      <FolderIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
                    )}
                    {renaming === folder.id ? (
                      <RenameField
                        name={folder.name}
                        onDone={(name) => {
                          setRenaming(null);
                          setFocused(folder.id);
                          if (name?.trim() && name !== folder.name) {
                            onRename(folder.id, name);
                          }
                        }}
                      />
                    ) : (
                      <span className="min-w-0 flex-1 truncate">{folder.name}</span>
                    )}
                    {count !== null && renaming !== folder.id && (
                      <span className="text-xs text-muted-foreground tabular-nums">
                        {count.toLocaleString("en")}
                      </span>
                    )}
                  </div>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem disabled={!canWrite} onSelect={() => onCreate(folder.id)}>
                    New Subfolder
                  </ContextMenuItem>
                  <ContextMenuItem disabled={!canWrite} onSelect={() => setRenaming(folder.id)}>
                    Rename
                    <ContextMenuShortcut>F2</ContextMenuShortcut>
                  </ContextMenuItem>
                  <ContextMenuSub>
                    <ContextMenuSubTrigger disabled={!canWrite}>Colour</ContextMenuSubTrigger>
                    <ContextMenuSubContent>
                      <ContextMenuItem onSelect={() => onColor(folder.id, null)}>
                        <span className="size-2.5 rounded-full border" />
                        None
                      </ContextMenuItem>
                      {Object.entries(FOLDER_COLORS).map(([name, css]) => (
                        <ContextMenuItem key={name} onSelect={() => onColor(folder.id, name)}>
                          <span className="size-2.5 rounded-full" style={{ background: css }} />
                          {name[0]?.toUpperCase()}
                          {name.slice(1)}
                        </ContextMenuItem>
                      ))}
                    </ContextMenuSubContent>
                  </ContextMenuSub>
                  <ContextMenuItem disabled={!canWrite} onSelect={() => onMoveTo(folder.id)}>
                    Move To…
                  </ContextMenuItem>
                  <ContextMenuItem onSelect={() => onExport(folder.id, folder.name)}>
                    Export…
                  </ContextMenuItem>
                  {onLock && !eagle && (
                    <>
                      <ContextMenuSeparator />
                      <ContextMenuItem
                        disabled={!canWrite}
                        onSelect={() => onLock(folder.id, "lock")}
                      >
                        Lock…
                      </ContextMenuItem>
                    </>
                  )}
                  {!eagle && (
                    <>
                      <ContextMenuSeparator />
                      <ContextMenuItem
                        disabled={!canWrite}
                        className="text-destructive focus:text-destructive"
                        onSelect={() => onDelete(folder.id)}
                      >
                        Delete…
                        <ContextMenuShortcut>Del</ContextMenuShortcut>
                      </ContextMenuItem>
                    </>
                  )}
                </ContextMenuContent>
              </ContextMenu>
            );
          })}
        </div>
      )}
    </section>
  );
}

/** The name in place: Enter or leaving the field saves, Esc keeps the old name. */
function RenameField({ name, onDone }: { name: string; onDone: (name: string | null) => void }) {
  const done = useRef(false);
  const finish = (value: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(value);
  };
  return (
    <input
      aria-label="Folder name"
      defaultValue={name}
      // biome-ignore lint/a11y/noAutofocus: renaming starts typing in the field.
      autoFocus
      onFocus={(event) => event.currentTarget.select()}
      className="h-6 min-w-0 flex-1 rounded border bg-background px-1 text-[13px] outline-none focus:ring-2 focus:ring-ring/50"
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Enter") finish(event.currentTarget.value);
        else if (event.key === "Escape") finish(null);
      }}
      onBlur={(event) => finish(event.currentTarget.value)}
      onMouseDown={(event) => event.stopPropagation()}
      onClick={(event) => event.stopPropagation()}
    />
  );
}
