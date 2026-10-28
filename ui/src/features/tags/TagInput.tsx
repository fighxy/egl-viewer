import { cn } from "cn";
import { Plus } from "lucide-react";
import { useId, useState } from "react";
import type { Tag } from "@/ipc";
import { cleanTag, countText, matchTags } from "./match";

/** How many suggestions show under the field. */
const SUGGESTIONS = 8;

type Props = {
  /** The field's accessible name. */
  label: string;
  placeholder?: string;
  /** Suggestions come from here. */
  vocabulary: readonly Tag[];
  /** Names not to suggest, for example the item's own tags. */
  exclude?: ReadonlySet<string>;
  /** Only names of the vocabulary can be picked (Merge into). */
  existingOnly?: boolean;
  onPick: (name: string) => void;
  disabled?: boolean;
  /** Leaves the text after a pick, for a field that holds one name. */
  keepText?: boolean;
  autoFocus?: boolean;
};

/**
 * A tag field with vocabulary suggestions (docs/spec.md §7.3): `Enter` takes the highlighted
 * suggestion, or the text as typed, case and all, as a new tag.
 */
export function TagInput({
  label,
  placeholder = "Add a tag",
  vocabulary,
  exclude,
  existingOnly = false,
  onPick,
  disabled = false,
  keepText = false,
  autoFocus = false,
}: Props) {
  const [text, setText] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const listId = useId();
  const name = cleanTag(text);
  const suggestions = name
    ? matchTags(
        vocabulary.filter((tag) => !exclude?.has(tag.name)),
        name,
      ).slice(0, SUGGESTIONS)
    : [];
  const exact = vocabulary.some((tag) => tag.name === name);
  const offerNew = name !== "" && !exact && !existingOnly && !exclude?.has(name);
  const options = [
    ...suggestions.map((tag) => ({ name: tag.name, count: tag.imageCount, isNew: false })),
    ...(offerNew ? [{ name, count: null, isNew: true }] : []),
  ];
  const shown = open && options.length > 0;

  const pick = (picked: string) => {
    onPick(picked);
    setText(keepText ? picked : "");
    setActive(-1);
    setOpen(keepText ? false : open);
  };

  return (
    <div className="relative">
      <input
        role="combobox"
        aria-label={label}
        aria-expanded={shown}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={shown && active >= 0 ? `${listId}-${active}` : undefined}
        // biome-ignore lint/a11y/noAutofocus: a dialog that asks for this one field.
        autoFocus={autoFocus}
        disabled={disabled}
        placeholder={placeholder}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          setActive(-1);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" && options.length > 0) {
            event.preventDefault();
            setOpen(true);
            setActive((current) => (current + 1) % options.length);
          } else if (event.key === "ArrowUp" && options.length > 0) {
            event.preventDefault();
            setActive((current) => (current <= 0 ? options.length - 1 : current - 1));
          } else if (event.key === "Enter") {
            event.preventDefault();
            const chosen = options[active]?.name;
            if (chosen !== undefined) pick(chosen);
            else if (name && (!existingOnly || exact) && !exclude?.has(name)) pick(name);
          } else if (event.key === "Escape" && text !== "") {
            event.stopPropagation();
            setText("");
            setOpen(false);
          }
        }}
        className="h-7 w-full rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50 disabled:opacity-50"
      />
      {shown && (
        <div
          id={listId}
          role="listbox"
          aria-label="Suggestions"
          className="absolute inset-x-0 top-full z-50 mt-1 grid max-h-64 gap-px overflow-y-auto rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
        >
          {options.map((option, index) => (
            // biome-ignore lint/a11y/useKeyWithClickEvents: the keyboard picks through the field (arrows and Enter).
            <div
              key={`${option.isNew ? "new:" : ""}${option.name}`}
              id={`${listId}-${index}`}
              role="option"
              aria-selected={index === active}
              aria-label={option.isNew ? `Create “${option.name}”` : option.name}
              tabIndex={-1}
              className={cn(
                "flex h-7 cursor-default items-center gap-2 rounded px-2 text-[13px]",
                index === active ? "bg-accent" : "hover:bg-accent",
              )}
              // Before the field's blur closes the list.
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => pick(option.name)}
            >
              {option.isNew && <Plus aria-hidden className="size-3.5 text-muted-foreground" />}
              <span className="min-w-0 flex-1 truncate">
                {option.isNew ? `Create “${option.name}”` : option.name}
              </span>
              {!option.isNew && (
                <span className="text-xs text-muted-foreground tabular-nums">
                  {countText(option.count)}
                </span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
