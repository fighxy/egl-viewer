import { useQuery } from "@tanstack/react-query";
import { cn } from "cn";
import { FolderPlus, Plus, X } from "lucide-react";
import { forwardRef, type ReactNode, useState } from "react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { FolderPicker } from "@/features/folders/FolderPicker";
import { findFolder, folderPath } from "@/features/folders/tree";
import { type TagChip, TagEditor } from "@/features/tags/TagEditor";
import { type Folder, type Item, inTauri, ipc, type SelectionTags, type Tag } from "@/ipc";
import { thumbUrl } from "@/lib/thumb";
import { formatBytes, formatDate, formatDimensions } from "./format";

type Props = {
  /** How many items are selected. */
  count: number;
  /** The selected item when exactly one is selected. */
  itemId: string | null;
  /** Opens the full-size preview of the selected item. */
  onPreview: () => void;
  folders?: Folder[];
  /** `true`, or why items cannot change (read-only library). */
  writable?: true | string;
  /** The one selected item joins (`true`) or leaves a folder. */
  onFolderChange?: (folder: string, member: boolean) => void;
  /** The selected items join a folder. */
  onAddToFolder?: (folder: string) => void;
  /** Shown when nothing is selected: the open folder's properties. */
  empty?: ReactNode;
  /** Tag suggestions. */
  vocabulary?: readonly Tag[];
  /** The tags of a selection of several items, with how many carry each. */
  selectionTags?: SelectionTags | null;
  /** A tag goes on the selected items, or comes off them. */
  onAddTag?: (name: string) => void;
  onRemoveTag?: (name: string) => void;
};

/**
 * The right bar (docs/spec.md §7.3): the selected item's preview, palette, name, rating, tags,
 * folders, note, link, file info and EXIF. Tags and folders can change, for one item or a
 * selection; the other fields arrive with phase 7.
 */
export const DetailsPanel = forwardRef<HTMLDivElement, Props>(function DetailsPanel(
  {
    count,
    itemId,
    onPreview,
    folders = [],
    writable = "This library is read-only.",
    onFolderChange,
    onAddToFolder,
    empty,
    vocabulary = [],
    selectionTags = null,
    onAddTag,
    onRemoveTag,
  },
  ref,
) {
  const tagEditor = (tags: readonly TagChip[]) =>
    onAddTag && onRemoveTag ? (
      <TagEditor
        tags={tags}
        vocabulary={vocabulary}
        writable={writable}
        onAdd={onAddTag}
        onRemove={onRemoveTag}
      />
    ) : null;
  const detail = useQuery({
    queryKey: ["item", itemId],
    queryFn: () => ipc.itemDetail(itemId ?? ""),
    enabled: itemId !== null && inTauri(),
  });

  let body: React.ReactNode;
  if (count === 0) {
    body = empty ?? <p className="m-auto text-muted-foreground">Select an item</p>;
  } else if (count > 1 || itemId === null) {
    body = (
      <div className="grid content-start gap-4">
        <p className="text-center text-muted-foreground">
          {count.toLocaleString("en")} items selected
        </p>
        {onAddTag && (
          <dl className="grid gap-2">
            <Field label="Tags">
              {selectionTags ? (
                tagEditor(
                  selectionTags.tags.map((tag) =>
                    (tag.imageCount ?? 0) < selectionTags.items
                      ? { name: tag.name, on: tag.imageCount ?? 0, of: selectionTags.items }
                      : { name: tag.name },
                  ),
                )
              ) : (
                <span className="text-muted-foreground">…</span>
              )}
            </Field>
          </dl>
        )}
        {onAddToFolder && (
          <FolderButton
            label="Add to folder…"
            folders={folders}
            writable={writable}
            onPick={onAddToFolder}
          />
        )}
      </div>
    );
  } else if (detail.data) {
    body = (
      <ItemDetails
        item={detail.data}
        onPreview={onPreview}
        folders={folders}
        writable={writable}
        onFolderChange={onFolderChange}
        tags={tagEditor(detail.data.tags.map((name) => ({ name })))}
      />
    );
  } else if (detail.isError) {
    body = <p className="m-auto text-muted-foreground">This item could not be read.</p>;
  } else {
    body = null;
  }

  return (
    <div ref={ref} tabIndex={-1} className="flex min-h-0 flex-1 flex-col outline-none">
      {body}
    </div>
  );
});

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[64px_minmax(0,1fr)] gap-2">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-words select-text">{children}</dd>
    </div>
  );
}

function Stars({ value }: { value: number }) {
  return (
    <span role="img" aria-label={value === 0 ? "Not rated" : `${value} of 5 stars`}>
      {[1, 2, 3, 4, 5].map((star) => (
        <span key={star} className={star <= value ? "text-amber-500" : "text-muted-foreground/40"}>
          ★
        </span>
      ))}
    </span>
  );
}

/** EXIF strings as the media pipeline stores them (Eagle `rawMetas` keys). */
const EXIF: readonly { key: string; label: string; format?: (value: string) => string }[] = [
  { key: "camera", label: "Camera" },
  { key: "isoSpeed", label: "ISO" },
  { key: "aperture", label: "Aperture", format: (value) => `ƒ/${value}` },
  { key: "shutter", label: "Shutter" },
  { key: "focalLength", label: "Focal length", format: (value) => `${value} mm` },
  { key: "timestamp", label: "Taken" },
];

function exifRows(item: Item) {
  const exif = item.exif;
  if (typeof exif !== "object" || exif === null || Array.isArray(exif)) return [];
  return EXIF.flatMap(({ key, label, format }) => {
    const value = (exif as Record<string, unknown>)[key];
    if (typeof value !== "string" && typeof value !== "number") return [];
    const text = String(value);
    return [{ label, value: format ? format(text) : text }];
  });
}

/** Opens the folder picker; picking a folder runs `onPick`. */
function FolderButton({
  label,
  folders,
  writable,
  checked,
  onPick,
  compact = false,
}: {
  label: string;
  folders: Folder[];
  writable: true | string;
  checked?: ReadonlySet<string>;
  onPick: (folder: string) => void;
  compact?: boolean;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant={compact ? "ghost" : "outline"}
          size={compact ? "xs" : "sm"}
          disabled={writable !== true}
          title={writable === true ? undefined : writable}
        >
          {compact ? <Plus aria-hidden /> : <FolderPlus aria-hidden />}
          {label}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="p-2">
        <FolderPicker
          label="Folders"
          folders={folders}
          checked={checked}
          onPick={(folder) => {
            onPick(folder);
            if (!checked) setOpen(false);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}

function ItemDetails({
  item,
  onPreview,
  folders,
  writable,
  onFolderChange,
  tags,
}: {
  item: Item;
  onPreview: () => void;
  folders: Folder[];
  writable: true | string;
  onFolderChange?: (folder: string, member: boolean) => void;
  /** The tag editor; without one, the tags are shown read-only. */
  tags?: ReactNode;
}) {
  const member = new Set(item.folders);
  const [failed, setFailed] = useState(false);
  const exif = exifRows(item);
  const info = [
    item.ext.toUpperCase(),
    formatBytes(item.size),
    formatDimensions(item.width, item.height),
  ];
  return (
    <div className="grid min-h-0 content-start gap-4 overflow-y-auto">
      <button
        type="button"
        title="Show full size (Space)"
        className="grid aspect-[4/3] place-items-center overflow-hidden rounded-md bg-grid outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
        onClick={onPreview}
      >
        {!item.noThumbnail && !failed ? (
          <img
            src={thumbUrl({ id: item.id, thumbVersion: item.thumbVersion })}
            alt={item.name}
            draggable={false}
            className="max-h-full max-w-full object-contain"
            onError={() => setFailed(true)}
          />
        ) : (
          <span className="text-sm font-medium text-muted-foreground uppercase">{item.ext}</span>
        )}
      </button>
      {item.palettes.length > 0 && (
        <div className="flex h-4 overflow-hidden rounded" role="img" aria-label="Palette">
          {item.palettes.map((entry) => (
            <span
              key={entry.color.join(",")}
              style={{
                background: `rgb(${entry.color.join(" ")})`,
                flexGrow: Math.max(entry.ratio, 1),
              }}
            />
          ))}
        </div>
      )}
      {item.isDeleted && (
        <p className="rounded-md bg-muted px-2 py-1 text-xs text-muted-foreground">In the trash</p>
      )}
      <dl className="grid gap-2">
        <Field label="Name">{item.name}</Field>
        <Field label="Rating">
          <Stars value={item.star} />
        </Field>
        <Field label="Tags">
          {tags ??
            (item.tags.length === 0 ? (
              <span className="text-muted-foreground">None</span>
            ) : (
              <span className="flex flex-wrap gap-1">
                {item.tags.map((tag) => (
                  <span key={tag} className="rounded bg-muted px-1.5 py-0.5 text-xs">
                    {tag}
                  </span>
                ))}
              </span>
            ))}
        </Field>
        <Field label="Folders">
          <span className="flex flex-wrap items-center gap-1">
            {item.folders.map((id) => {
              const folder = findFolder(folders, id);
              const path = folderPath(folders, id)
                .map((step) => step.name)
                .join(" › ");
              return (
                <span
                  key={id}
                  title={path}
                  className="flex items-center gap-0.5 rounded bg-muted py-0.5 pr-0.5 pl-1.5 text-xs"
                >
                  {folder?.name ?? "Unknown folder"}
                  {writable === true && onFolderChange && (
                    <button
                      type="button"
                      aria-label={`Remove from ${folder?.name ?? "folder"}`}
                      className="rounded p-0.5 hover:bg-background"
                      onClick={() => onFolderChange(id, false)}
                    >
                      <X aria-hidden className="size-3" />
                    </button>
                  )}
                </span>
              );
            })}
            {item.folders.length === 0 && writable !== true && (
              <span className="text-muted-foreground">None</span>
            )}
            {onFolderChange && writable === true && (
              <FolderButton
                compact
                label="Add"
                folders={folders}
                writable={writable}
                checked={member}
                onPick={(folder) => onFolderChange(folder, !member.has(folder))}
              />
            )}
          </span>
        </Field>
        {item.annotation && <Field label="Note">{item.annotation}</Field>}
        {item.url && <Field label="Link">{item.url}</Field>}
      </dl>
      <section className="grid gap-2">
        <h3 className="text-xs font-medium text-muted-foreground">Info</h3>
        <p className="select-text">{info.join(" · ")}</p>
        <dl className="grid gap-1">
          <Field label="Added">{formatDate(item.addedAt)}</Field>
          <Field label="Created">{formatDate(item.fileCreatedAt)}</Field>
          <Field label="Modified">{formatDate(item.fileModifiedAt)}</Field>
        </dl>
      </section>
      {exif.length > 0 && (
        <details className="group grid gap-2">
          <summary
            className={cn(
              "cursor-default text-xs font-medium text-muted-foreground outline-none",
              "focus-visible:ring-3 focus-visible:ring-ring/50",
            )}
          >
            EXIF
          </summary>
          <dl className="mt-2 grid gap-1">
            {exif.map((row) => (
              <Field key={row.label} label={row.label}>
                {row.value}
              </Field>
            ))}
          </dl>
        </details>
      )}
    </div>
  );
}
