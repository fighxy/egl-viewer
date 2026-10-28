import { useQuery } from "@tanstack/react-query";
import { cn } from "cn";
import { forwardRef, useState } from "react";
import { type Item, inTauri, ipc } from "@/ipc";
import { thumbUrl } from "@/lib/thumb";
import { formatBytes, formatDate, formatDimensions } from "./format";

type Props = {
  /** How many items are selected. */
  count: number;
  /** The selected item when exactly one is selected. */
  itemId: string | null;
  /** Opens the full-size preview of the selected item. */
  onPreview: () => void;
};

/**
 * The right bar (docs/spec.md §7.3), read-only in this version: the selected item's preview,
 * palette, name, rating, tags, note, link, file info and EXIF. Editing comes with phases 5 and 7.
 */
export const DetailsPanel = forwardRef<HTMLDivElement, Props>(function DetailsPanel(
  { count, itemId, onPreview },
  ref,
) {
  const detail = useQuery({
    queryKey: ["item", itemId],
    queryFn: () => ipc.itemDetail(itemId ?? ""),
    enabled: itemId !== null && inTauri(),
  });

  let body: React.ReactNode;
  if (count === 0) {
    body = <p className="m-auto text-muted-foreground">Select an item</p>;
  } else if (count > 1 || itemId === null) {
    body = (
      <p className="m-auto text-muted-foreground">{count.toLocaleString("en")} items selected</p>
    );
  } else if (detail.data) {
    body = <ItemDetails item={detail.data} onPreview={onPreview} />;
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

function ItemDetails({ item, onPreview }: { item: Item; onPreview: () => void }) {
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
          {item.tags.length === 0 ? (
            <span className="text-muted-foreground">None</span>
          ) : (
            <span className="flex flex-wrap gap-1">
              {item.tags.map((tag) => (
                <span key={tag} className="rounded bg-muted px-1.5 py-0.5 text-xs">
                  {tag}
                </span>
              ))}
            </span>
          )}
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
