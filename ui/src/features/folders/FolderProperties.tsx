import { cn } from "cn";
import { X } from "lucide-react";
import { useId, useState } from "react";
import { MANUAL, SORT_KEYS } from "@/commands/registry";
import type { Folder, FolderChange, SortKey } from "@/ipc";
import { FOLDER_COLORS } from "./tree";

type Props = {
  folder: Folder;
  /** `true`, or why the folder cannot change (read-only library). */
  writable: true | string;
  /** Include subfolders is on: subfolder items cannot be reordered here. */
  includeSubfolders?: boolean;
  onSave: (change: Extract<FolderChange, { type: "update" }>) => void;
};

/**
 * The open folder's properties in the right bar when nothing is selected (docs/spec.md §7.3):
 * name, description, colour, folder tags and its own sort. A field saves on Enter or when focus
 * leaves it; Esc reverts it.
 */
export function FolderProperties({ folder, writable, includeSubfolders = false, onSave }: Props) {
  const disabled = writable !== true;
  const id = useId();
  const save = (change: Partial<Extract<FolderChange, { type: "update" }>>) =>
    onSave({
      type: "update",
      id: folder.id,
      name: folder.name,
      description: folder.description,
      tags: folder.tags,
      iconColor: folder.iconColor,
      orderBy: folder.orderBy,
      sortAscending: folder.sortAscending,
      ...change,
    });
  const inherited = folder.extendTags.filter((tag) => !folder.tags.includes(tag));
  const sortValue = folder.orderBy
    ? `${folder.orderBy}:${folder.sortAscending ? "asc" : "desc"}`
    : "";

  return (
    <div className="grid content-start gap-4 overflow-y-auto" key={folder.id}>
      <p className="text-xs text-muted-foreground">
        {(folder.imageCount ?? 0).toLocaleString("en")} items in this folder,{" "}
        {(folder.descendantImageCount ?? 0).toLocaleString("en")} with subfolders
      </p>
      <div className="grid gap-1">
        <label htmlFor={`${id}-name`} className="text-xs text-muted-foreground">
          Name
        </label>
        <TextField
          id={`${id}-name`}
          value={folder.name}
          disabled={disabled}
          onCommit={(name) => name.trim() && save({ name: name.trim() })}
        />
      </div>
      <div className="grid gap-1">
        <label htmlFor={`${id}-description`} className="text-xs text-muted-foreground">
          Description
        </label>
        <TextField
          id={`${id}-description`}
          multiline
          value={folder.description}
          disabled={disabled}
          onCommit={(description) => save({ description })}
        />
      </div>
      <fieldset className="grid gap-1" disabled={disabled}>
        <legend className="mb-1 text-xs text-muted-foreground">Colour</legend>
        <div className="flex flex-wrap gap-1.5">
          <button
            type="button"
            aria-label="No colour"
            aria-pressed={folder.iconColor === null}
            className="size-5 rounded-full border aria-pressed:ring-2 aria-pressed:ring-ring"
            onClick={() => save({ iconColor: null })}
          />
          {Object.entries(FOLDER_COLORS).map(([name, css]) => (
            <button
              key={name}
              type="button"
              aria-label={name}
              aria-pressed={folder.iconColor === name}
              className="size-5 rounded-full aria-pressed:ring-2 aria-pressed:ring-ring aria-pressed:ring-offset-1"
              style={{ background: css }}
              onClick={() => save({ iconColor: name })}
            />
          ))}
        </div>
      </fieldset>
      <div className="grid gap-1">
        <span className="text-xs text-muted-foreground" id={`tags-${folder.id}`}>
          Folder tags — added to files added here
        </span>
        <TagField
          labelledBy={`tags-${folder.id}`}
          tags={folder.tags}
          disabled={disabled}
          onChange={(tags) => save({ tags })}
        />
        {inherited.length > 0 && (
          <p className="text-xs text-muted-foreground">
            From parent folders: {inherited.join(", ")}
          </p>
        )}
      </div>
      <label className="grid gap-1">
        <span className="text-xs text-muted-foreground">Sort</span>
        <select
          value={sortValue}
          disabled={disabled}
          className="h-8 rounded-md border bg-background px-1 text-[13px] outline-none focus:ring-2 focus:ring-ring/50"
          onChange={(event) => {
            const [key, direction] = event.target.value.split(":");
            if (!key) save({ orderBy: null, sortAscending: null });
            else save({ orderBy: key as SortKey, sortAscending: direction === "asc" });
          }}
        >
          <option value="">The global sort</option>
          {[...SORT_KEYS, MANUAL].flatMap(({ key, label }) => [
            <option key={`${key}:asc`} value={`${key}:asc`}>
              {label}, ascending
            </option>,
            <option key={`${key}:desc`} value={`${key}:desc`}>
              {label}, descending
            </option>,
          ])}
        </select>
        {folder.orderBy === "manual" && !disabled && (
          <span className="text-xs text-muted-foreground">
            {includeSubfolders && folder.children.length > 0
              ? "Turn off Include subfolders to drag items into a new order."
              : "Drag items in the grid to put them in order."}
          </span>
        )}
      </label>
    </div>
  );
}

/** Saves on Enter (Ctrl+Enter in a multi-line field) or when focus leaves; Esc reverts. */
function TextField({
  id,
  value,
  disabled,
  multiline = false,
  onCommit,
}: {
  id: string;
  value: string;
  disabled: boolean;
  multiline?: boolean;
  onCommit: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  const [shown, setShown] = useState(value);
  if (shown !== value) {
    setShown(value);
    setDraft(value);
  }
  const commit = () => {
    if (draft !== value) onCommit(draft);
  };
  const props = {
    id,
    value: draft,
    disabled,
    onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) =>
      setDraft(event.target.value),
    onBlur: commit,
    onKeyDown: (event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
      if (event.key === "Escape") {
        setDraft(value);
        event.currentTarget.blur();
      } else if (event.key === "Enter" && (!multiline || event.ctrlKey)) {
        event.preventDefault();
        event.currentTarget.blur();
      }
    },
    className:
      "rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50 disabled:opacity-60",
  };
  return multiline ? (
    <textarea rows={3} {...props} className={cn(props.className, "py-1")} />
  ) : (
    <input {...props} className={cn(props.className, "h-8")} />
  );
}

/** Tags as chips; Enter or a comma adds what was typed. Exact, case-sensitive strings. */
export function TagField({
  tags,
  disabled,
  labelledBy,
  onChange,
}: {
  tags: string[];
  disabled: boolean;
  labelledBy: string;
  onChange: (tags: string[]) => void;
}) {
  const [draft, setDraft] = useState("");
  const add = () => {
    const tag = draft.trim();
    setDraft("");
    if (tag && !tags.includes(tag)) onChange([...tags, tag]);
  };
  return (
    <div className="flex flex-wrap items-center gap-1 rounded-md border bg-background p-1">
      {tags.map((tag) => (
        <span
          key={tag}
          className="flex items-center gap-0.5 rounded bg-muted py-0.5 pr-0.5 pl-1.5 text-xs"
        >
          {tag}
          {!disabled && (
            <button
              type="button"
              aria-label={`Remove ${tag}`}
              className="rounded p-0.5 hover:bg-background"
              onClick={() => onChange(tags.filter((own) => own !== tag))}
            >
              <X aria-hidden className="size-3" />
            </button>
          )}
        </span>
      ))}
      {!disabled && (
        <input
          aria-labelledby={labelledBy}
          placeholder={tags.length === 0 ? "Add a tag" : ""}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === ",") {
              event.preventDefault();
              add();
            } else if (event.key === "Backspace" && draft === "" && tags.length > 0) {
              onChange(tags.slice(0, -1));
            }
          }}
          onBlur={add}
          className="h-6 min-w-16 flex-1 bg-transparent px-1 text-[13px] outline-none"
        />
      )}
    </div>
  );
}
