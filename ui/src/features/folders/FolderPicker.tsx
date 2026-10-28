import { cn } from "cn";
import { Check, Folder as FolderIcon } from "lucide-react";
import { useState } from "react";
import type { Folder } from "@/ipc";
import { allRows, FOLDER_COLORS } from "./tree";

type Props = {
  folders: Folder[];
  /** Folders shown with a check mark. */
  checked?: ReadonlySet<string>;
  /** Folders that cannot be picked, for example a folder and its own subfolders in a move. */
  disabled?: ReadonlySet<string>;
  onPick: (id: string) => void;
  /** An extra first row, such as "Top level" for a move. */
  topLabel?: string;
  onPickTop?: () => void;
  label: string;
};

/** Every folder in tree order, with a filter by name: the right bar's folder picker and Move To. */
export function FolderPicker({
  folders,
  checked,
  disabled,
  onPick,
  topLabel,
  onPickTop,
  label,
}: Props) {
  const [filter, setFilter] = useState("");
  const needle = filter.trim().toLocaleLowerCase();
  const rows = allRows(folders).filter(
    (row) => needle === "" || row.folder.name.toLocaleLowerCase().includes(needle),
  );
  return (
    <div className="grid gap-1">
      <input
        aria-label="Find a folder"
        placeholder="Find a folder"
        value={filter}
        onChange={(event) => setFilter(event.target.value)}
        className="h-7 rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50"
      />
      <div role="listbox" aria-label={label} className="grid max-h-64 gap-px overflow-y-auto">
        {topLabel && onPickTop && needle === "" && (
          <button
            type="button"
            role="option"
            aria-selected={false}
            className="flex h-7 items-center gap-2 rounded-md px-2 text-left text-[13px] hover:bg-accent"
            onClick={onPickTop}
          >
            {topLabel}
          </button>
        )}
        {rows.map(({ folder, depth }) => {
          const isChecked = checked?.has(folder.id) ?? false;
          const isDisabled = disabled?.has(folder.id) ?? false;
          const color = folder.iconColor ? FOLDER_COLORS[folder.iconColor] : undefined;
          return (
            <button
              key={folder.id}
              type="button"
              role="option"
              aria-selected={isChecked}
              disabled={isDisabled}
              className={cn(
                "flex h-7 items-center gap-2 rounded-md pr-2 text-left text-[13px] hover:bg-accent disabled:opacity-40 disabled:hover:bg-transparent",
              )}
              style={{ paddingLeft: 8 + (needle === "" ? depth * 14 : 0) }}
              onClick={() => onPick(folder.id)}
            >
              {color ? (
                <span className="size-2.5 shrink-0 rounded-full" style={{ background: color }} />
              ) : (
                <FolderIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
              )}
              <span className="min-w-0 flex-1 truncate">{folder.name}</span>
              {isChecked && <Check aria-hidden className="size-4" />}
            </button>
          );
        })}
        {rows.length === 0 && (
          <p className="px-2 py-1 text-xs text-muted-foreground">
            {folders.length === 0 ? "No folders yet." : "No folder has that name."}
          </p>
        )}
      </div>
    </div>
  );
}
