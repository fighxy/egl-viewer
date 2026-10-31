import { useQuery } from "@tanstack/react-query";
import { cn } from "cn";
import type { TFunction } from "i18next";
import {
  Copy,
  ExternalLink,
  FolderOpen,
  FolderPlus,
  Plus,
  RotateCcw,
  SquareArrowOutUpRight,
  Trash2,
  X,
} from "lucide-react";
import { forwardRef, type ReactNode, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { FolderPicker } from "@/features/folders/FolderPicker";
import { findFolder, folderPath } from "@/features/folders/tree";
import { type TagChip, TagEditor } from "@/features/tags/TagEditor";
import {
  type Folder,
  type Item,
  type ItemEdit,
  inTauri,
  ipc,
  type SelectionTags,
  type Tag,
} from "@/ipc";
import { thumbUrl } from "@/lib/thumb";
import { formatBytes, formatDate, formatDimensions } from "./format";
import { EditableText, isWebLink, Stars } from "./ItemFields";

/** What the right bar can do with the selected items (docs/spec.md §6, §7.3). */
export type ItemActions = {
  /** Name, note, link or rating of the one selected item. */
  onEdit: (edit: ItemEdit) => void;
  /** The rating of every selected item. */
  onSetStar: (star: number) => void;
  onTrash: () => void;
  onRestore: () => void;
  onDeletePermanently: () => void;
  onOpenOriginal: () => void;
  onReveal: () => void;
  onCopy: () => void;
  onOpenLink: (url: string) => void;
  /** Starts dragging the selected originals out of the window. */
  onDragOut: () => void;
};

/** How far the pointer moves before a press on the preview becomes a drag out. */
const DRAG_DISTANCE = 4;

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
  /** Editing, the trash and file actions; without them the fields are read-only. */
  actions?: ItemActions;
  /** The Trash is open: a selection there can be restored or removed for good. */
  inTrash?: boolean;
};

/**
 * The right bar (docs/spec.md §7.3): the selected item's preview, palette, name, rating, tags,
 * folders, note, link, file info and EXIF, edited in place; for a selection, a stack of
 * thumbnails, the total size, the rating, tags, folders and the trash.
 */
export const DetailsPanel = forwardRef<HTMLDivElement, Props>(function DetailsPanel(
  {
    count,
    itemId,
    onPreview,
    folders = [],
    writable: writableProp,
    onFolderChange,
    onAddToFolder,
    empty,
    vocabulary = [],
    selectionTags = null,
    onAddTag,
    onRemoveTag,
    actions,
    inTrash = false,
  },
  ref,
) {
  const { t, i18n } = useTranslation();
  const writable: true | string =
    writableProp === undefined ? t("availability.readOnly") : writableProp;
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
    body = empty ?? <p className="m-auto text-muted-foreground">{t("details.selectItem")}</p>;
  } else if (count > 1 || itemId === null) {
    body = (
      <div className="grid min-h-0 content-start gap-4 overflow-y-auto">
        <Stack
          cells={selectionTags?.stack ?? []}
          count={count}
          onDragOut={inTrash ? undefined : actions?.onDragOut}
        />
        <p className="text-center text-muted-foreground">
          {t("details.itemsSelected", { formatted: count.toLocaleString(i18n.language) })}
          {selectionTags && ` · ${formatBytes(selectionTags.size)}`}
        </p>
        {actions && !inTrash && (
          <dl className="grid gap-2">
            <Field label={t("details.rating")}>
              <Stars
                value={null}
                onChange={actions.onSetStar}
                disabled={writable !== true}
                title={writable === true ? undefined : writable}
              />
            </Field>
          </dl>
        )}
        {onAddTag && (
          <dl className="grid gap-2">
            <Field label={t("details.tags")}>
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
        {onAddToFolder && !inTrash && (
          <FolderButton
            label={t("details.addToFolder")}
            folders={folders}
            writable={writable}
            onPick={onAddToFolder}
          />
        )}
        {actions && (
          <SelectionButtons actions={actions} writable={writable} inTrash={inTrash} several />
        )}
      </div>
    );
  } else if (detail.data) {
    body = (
      <ItemDetails
        key={detail.data.id}
        item={detail.data}
        onPreview={onPreview}
        folders={folders}
        writable={writable}
        onFolderChange={onFolderChange}
        tags={tagEditor(detail.data.tags.map((name) => ({ name })))}
        actions={actions}
      />
    );
  } else if (detail.isError) {
    body = <p className="m-auto text-muted-foreground">{t("details.couldNotRead")}</p>;
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

/** EXIF strings as the media pipeline stores them (Eagle `rawMetas` keys). */
function exifRows(item: Item, t: TFunction) {
  const defs: readonly { key: string; label: string; format?: (value: string) => string }[] = [
    { key: "camera", label: t("details.camera") },
    { key: "isoSpeed", label: t("details.iso") },
    { key: "aperture", label: t("details.aperture"), format: (value) => `ƒ/${value}` },
    { key: "shutter", label: t("details.shutter") },
    { key: "focalLength", label: t("details.focalLength"), format: (value) => `${value} mm` },
    { key: "timestamp", label: t("details.taken") },
  ];
  const exif = item.exif;
  if (typeof exif !== "object" || exif === null || Array.isArray(exif)) return [];
  return defs.flatMap(({ key, label, format }) => {
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
  const { t } = useTranslation();
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
          label={t("details.folders")}
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

/** Buttons for the selection: the trash, and the originals in other apps. */
function SelectionButtons({
  actions,
  writable,
  inTrash,
  several = false,
}: {
  actions: ItemActions;
  writable: true | string;
  inTrash: boolean;
  several?: boolean;
}) {
  const { t } = useTranslation();
  const blocked = writable === true ? undefined : writable;
  return (
    <div className="flex flex-wrap gap-1.5">
      {inTrash ? (
        <>
          <Button
            variant="outline"
            size="sm"
            disabled={blocked !== undefined}
            title={blocked}
            onClick={actions.onRestore}
          >
            <RotateCcw aria-hidden />
            {t("details.restore")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={blocked !== undefined}
            title={blocked}
            onClick={actions.onDeletePermanently}
          >
            <Trash2 aria-hidden />
            {t("details.deletePermanent")}
          </Button>
        </>
      ) : (
        <>
          <Button
            variant="outline"
            size="sm"
            title={t("details.showExplorer")}
            onClick={actions.onReveal}
          >
            <FolderOpen aria-hidden />
            {t("details.showExplorer")}
          </Button>
          {!several && (
            <Button variant="outline" size="sm" onClick={actions.onOpenOriginal}>
              <SquareArrowOutUpRight aria-hidden />
              {t("details.openDefault")}
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            title={several ? t("details.copyFilesTitle") : t("details.copyFileTitle")}
            onClick={actions.onCopy}
          >
            <Copy aria-hidden />
            {several ? t("details.copyFiles") : t("details.copyFile")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={blocked !== undefined}
            title={blocked ?? t("details.moveTrashTitle")}
            onClick={actions.onTrash}
          >
            <Trash2 aria-hidden />
            {t("details.moveTrash")}
          </Button>
        </>
      )}
    </div>
  );
}

/**
 * Reports a press on `element` that moves a few pixels as a drag out, once, and swallows the
 * click that would follow it.
 */
function useDragOut(onDragOut: (() => void) | undefined) {
  const start = useRef<{ x: number; y: number } | null>(null);
  const dragged = useRef(false);
  return {
    onPointerDown: (event: React.PointerEvent) => {
      if (event.button !== 0 || !onDragOut) return;
      start.current = { x: event.clientX, y: event.clientY };
      dragged.current = false;
    },
    onPointerMove: (event: React.PointerEvent) => {
      const from = start.current;
      if (!from || !onDragOut || (event.buttons & 1) === 0) return;
      if (Math.hypot(event.clientX - from.x, event.clientY - from.y) < DRAG_DISTANCE) return;
      start.current = null;
      dragged.current = true;
      onDragOut();
    },
    onPointerUp: () => {
      start.current = null;
    },
    /** True once per drag: the click that ends it opens nothing. */
    consumeDrag: () => {
      const was = dragged.current;
      dragged.current = false;
      return was;
    },
  };
}

/** Up to three thumbnails of a selection, fanned out; dragging them drags the files out. */
function Stack({
  cells,
  count,
  onDragOut,
}: {
  cells: SelectionTags["stack"];
  count: number;
  onDragOut?: () => void;
}) {
  const { t, i18n } = useTranslation();
  const drag = useDragOut(onDragOut);
  if (cells.length === 0) return null;
  return (
    <div
      role="img"
      aria-label={t("common.item", {
        count,
        formatted: count.toLocaleString(i18n.language),
      })}
      title={onDragOut ? t("details.dragOut") : undefined}
      className="relative mx-auto mt-2 aspect-square w-32 touch-none"
      onPointerDown={drag.onPointerDown}
      onPointerMove={drag.onPointerMove}
      onPointerUp={drag.onPointerUp}
    >
      {cells.map((cell, index) => (
        <div
          key={cell.id}
          className="absolute inset-0 grid place-items-center overflow-hidden rounded-md border bg-grid shadow-sm"
          style={{
            transform: `rotate(${(index - (cells.length - 1) / 2) * 6}deg)`,
            zIndex: cells.length - index,
          }}
        >
          {cell.hasThumbnail ? (
            <img
              src={thumbUrl(cell)}
              alt=""
              draggable={false}
              className="max-h-full max-w-full object-contain"
            />
          ) : (
            <span className="text-xs font-medium text-muted-foreground uppercase">{cell.ext}</span>
          )}
        </div>
      ))}
    </div>
  );
}

function ItemDetails({
  item,
  onPreview,
  folders,
  writable,
  onFolderChange,
  tags,
  actions,
}: {
  item: Item;
  onPreview: () => void;
  folders: Folder[];
  writable: true | string;
  onFolderChange?: (folder: string, member: boolean) => void;
  /** The tag editor; without one, the tags are shown read-only. */
  tags?: ReactNode;
  actions?: ItemActions;
}) {
  const { t } = useTranslation();
  const member = new Set(item.folders);
  const [failed, setFailed] = useState(false);
  const drag = useDragOut(item.isDeleted ? undefined : actions?.onDragOut);
  const editable = actions !== undefined && writable === true;
  const blocked = writable === true ? undefined : writable;
  const exif = exifRows(item, t);
  const info = [
    item.ext.toUpperCase(),
    formatBytes(item.size),
    formatDimensions(item.width, item.height),
  ];
  return (
    <div className="grid min-h-0 content-start gap-4 overflow-y-auto">
      <button
        type="button"
        title={actions && !item.isDeleted ? t("details.fullSizeDrag") : t("details.fullSize")}
        className="grid aspect-[4/3] touch-none place-items-center overflow-hidden rounded-md bg-grid outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
        onClick={() => {
          if (!drag.consumeDrag()) onPreview();
        }}
        onPointerDown={drag.onPointerDown}
        onPointerMove={drag.onPointerMove}
        onPointerUp={drag.onPointerUp}
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
        <div
          className="flex h-4 overflow-hidden rounded"
          role="img"
          aria-label={t("details.palette")}
        >
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
        <p className="rounded-md bg-muted px-2 py-1 text-xs text-muted-foreground">
          {item.deletedAt
            ? t("details.inTrashSince", { date: formatDate(item.deletedAt) })
            : t("details.inTrash")}
        </p>
      )}
      <dl className="grid gap-2">
        <Field label={t("details.name")}>
          {actions ? (
            <EditableText
              label={t("details.name")}
              value={item.name}
              disabled={!editable}
              title={blocked}
              check={(text) => (text === "" ? t("details.enterName") : null)}
              onSave={(name) => actions.onEdit({ type: "rename", value: name })}
            />
          ) : (
            item.name
          )}
        </Field>
        <Field label={t("details.rating")}>
          <Stars
            value={item.star}
            disabled={!editable}
            title={blocked}
            onChange={
              actions ? (star) => actions.onEdit({ type: "setStar", value: star }) : undefined
            }
          />
        </Field>
        <Field label={t("details.tags")}>
          {tags ??
            (item.tags.length === 0 ? (
              <span className="text-muted-foreground">{t("details.none")}</span>
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
        <Field label={t("details.folders")}>
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
                  {folder?.name ?? t("details.unknownFolder")}
                  {writable === true && onFolderChange && (
                    <button
                      type="button"
                      aria-label={t("details.removeFrom", {
                        name: folder?.name ?? t("details.unknownFolder"),
                      })}
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
              <span className="text-muted-foreground">{t("details.none")}</span>
            )}
            {onFolderChange && writable === true && (
              <FolderButton
                compact
                label={t("details.add")}
                folders={folders}
                writable={writable}
                checked={member}
                onPick={(folder) => onFolderChange(folder, !member.has(folder))}
              />
            )}
          </span>
        </Field>
        {actions ? (
          <>
            <Field label={t("details.note")}>
              <EditableText
                label={t("details.note")}
                multiline
                value={item.annotation}
                disabled={!editable}
                title={blocked ?? t("details.noteKeys")}
                placeholder={editable ? t("details.addNote") : undefined}
                onSave={(text) => actions.onEdit({ type: "setAnnotation", value: text })}
              />
            </Field>
            <Field label={t("details.link")}>
              <span className="flex items-start gap-1">
                <EditableText
                  label={t("details.link")}
                  value={item.url}
                  disabled={!editable}
                  title={blocked}
                  placeholder={editable ? "https://…" : undefined}
                  check={(text) =>
                    text === "" || isWebLink(text) ? null : t("details.linkInvalid")
                  }
                  onSave={(url) => actions.onEdit({ type: "setUrl", value: url })}
                />
                {isWebLink(item.url) && (
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={t("details.openLink")}
                    title={t("details.openLink")}
                    onClick={() => actions.onOpenLink(item.url)}
                  >
                    <ExternalLink aria-hidden />
                  </Button>
                )}
              </span>
            </Field>
          </>
        ) : (
          <>
            {item.annotation && <Field label={t("details.note")}>{item.annotation}</Field>}
            {item.url && <Field label={t("details.link")}>{item.url}</Field>}
          </>
        )}
      </dl>
      {actions && (
        <SelectionButtons actions={actions} writable={writable} inTrash={item.isDeleted} />
      )}
      <section className="grid gap-2">
        <h3 className="text-xs font-medium text-muted-foreground">{t("details.info")}</h3>
        <p className="select-text">{info.join(" · ")}</p>
        <dl className="grid gap-1">
          <Field label={t("details.added")}>{formatDate(item.addedAt)}</Field>
          <Field label={t("details.created")}>{formatDate(item.fileCreatedAt)}</Field>
          <Field label={t("details.modified")}>{formatDate(item.fileModifiedAt)}</Field>
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
