import { cn } from "cn";
import { X } from "lucide-react";
import type { Tag } from "@/ipc";
import { TagInput } from "./TagInput";

/** A tag of the item, or of a selection: `of` is set when only some items carry it. */
export type TagChip = { name: string; on?: number; of?: number };

type Props = {
  tags: readonly TagChip[];
  vocabulary: readonly Tag[];
  /** `true`, or why tags cannot change (read-only library). */
  writable: true | string;
  onAdd: (name: string) => void;
  onRemove: (name: string) => void;
};

/**
 * The tags of the right bar (docs/spec.md §7.3): chips with a remove button and a field with
 * vocabulary suggestions. With a selection, a tag that only some items carry is dimmed; a click
 * puts it on every selected item.
 */
export function TagEditor({ tags, vocabulary, writable, onAdd, onRemove }: Props) {
  const canWrite = writable === true;
  const own = new Set(tags.filter((tag) => tag.of === undefined).map((tag) => tag.name));
  return (
    <div className="grid gap-1.5">
      {tags.length > 0 ? (
        <ul aria-label="Tags" className="flex flex-wrap gap-1">
          {tags.map((tag) => {
            const partial = tag.of !== undefined;
            const title = partial ? `On ${tag.on} of ${tag.of} items` : undefined;
            return (
              <li
                key={tag.name}
                title={title}
                className={cn(
                  "flex max-w-full items-center gap-0.5 rounded bg-muted py-0.5 pr-0.5 pl-1.5 text-xs",
                  partial && "text-muted-foreground opacity-70",
                )}
              >
                {partial && canWrite ? (
                  <button
                    type="button"
                    aria-label={`Add ${tag.name} to every selected item`}
                    className="min-w-0 truncate rounded text-left hover:text-foreground"
                    onClick={() => onAdd(tag.name)}
                  >
                    {tag.name}
                  </button>
                ) : (
                  <span className="min-w-0 truncate">{tag.name}</span>
                )}
                {canWrite ? (
                  <button
                    type="button"
                    aria-label={`Remove ${tag.name}`}
                    className="rounded p-0.5 hover:bg-background"
                    onClick={() => onRemove(tag.name)}
                  >
                    <X aria-hidden className="size-3" />
                  </button>
                ) : (
                  <span className="w-1" />
                )}
              </li>
            );
          })}
        </ul>
      ) : (
        !canWrite && <span className="text-muted-foreground">None</span>
      )}
      {canWrite && (
        <TagInput label="Add a tag" vocabulary={vocabulary} exclude={own} onPick={onAdd} />
      )}
    </div>
  );
}
