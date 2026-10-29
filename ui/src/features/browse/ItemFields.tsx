import { cn } from "cn";
import { useRef, useState } from "react";

/** Eagle's limit for names and notes, in UTF-16 units like JavaScript counts. */
export const MAX_TEXT = 10_000;

/** Links open in the system browser, so only web links are stored (docs/spec.md §6). */
export function isWebLink(url: string): boolean {
  return /^https?:\/\//i.test(url.trim());
}

type EditableProps = {
  label: string;
  value: string;
  /** Saves the new text; called only when it changed. */
  onSave: (text: string) => void;
  disabled?: boolean;
  /** Why the field cannot change (a read-only library). */
  title?: string;
  multiline?: boolean;
  placeholder?: string;
  /** The reason a draft cannot be saved, or `null` when it can. */
  check?: (text: string) => string | null;
};

/**
 * A field edited in place (docs/spec.md §7.3): it saves on Enter (Ctrl+Enter in a note) or when
 * focus leaves it, and Esc brings back the stored text. There is no Save button.
 */
export function EditableText({
  label,
  value,
  onSave,
  disabled = false,
  title,
  multiline = false,
  placeholder,
  check,
}: EditableProps) {
  // While editing, the field shows the draft; otherwise always the stored value, so a save
  // that Rust refuses shows the rolled-back value however fast the answer comes.
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const [problem, setProblem] = useState<string | null>(null);
  const reverting = useRef(false);

  const commit = () => {
    if (reverting.current) {
      reverting.current = false;
      return;
    }
    const text = multiline ? draft : draft.trim();
    if (text === value) {
      setEditing(false);
      setProblem(null);
      return;
    }
    const reason = check?.(text) ?? null;
    if (reason !== null) {
      setProblem(reason);
      return;
    }
    setProblem(null);
    setEditing(false);
    onSave(text);
  };
  const revert = (field: HTMLElement) => {
    reverting.current = true;
    setEditing(false);
    setProblem(null);
    field.blur();
  };
  const shared = {
    "aria-label": label,
    "aria-invalid": problem !== null || undefined,
    value: editing ? draft : value,
    disabled,
    title,
    placeholder,
    maxLength: MAX_TEXT,
    onFocus: () => {
      if (editing) return;
      setDraft(value);
      setEditing(true);
    },
    onBlur: commit,
    className: cn(
      "w-full min-w-0 rounded-md border border-transparent bg-transparent px-1.5 py-0.5 outline-none select-text",
      "hover:border-input focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40",
      "disabled:hover:border-transparent aria-invalid:border-destructive",
    ),
  };
  return (
    <span className="grid gap-0.5">
      {multiline ? (
        <textarea
          {...shared}
          rows={Math.min(8, Math.max(2, (editing ? draft : value).split("\n").length))}
          className={cn(shared.className, "resize-none")}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.stopPropagation();
              revert(event.currentTarget);
            } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
              event.preventDefault();
              event.currentTarget.blur();
            }
          }}
        />
      ) : (
        <input
          {...shared}
          type="text"
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.stopPropagation();
              revert(event.currentTarget);
            } else if (event.key === "Enter") {
              event.preventDefault();
              event.currentTarget.blur();
            }
          }}
        />
      )}
      {problem && (
        <span role="alert" className="px-1.5 text-xs text-destructive">
          {problem}
        </span>
      )}
    </span>
  );
}

/**
 * The rating as five stars. With `onChange`, a click sets it and a click on the current rating
 * clears it. `value` null is a selection: no stars lit, a click rates every item.
 */
export function Stars({
  value,
  onChange,
  disabled = false,
  title,
}: {
  value: number | null;
  onChange?: (star: number) => void;
  disabled?: boolean;
  title?: string;
}) {
  const shown = value ?? 0;
  const description =
    value === null
      ? "set it for every selected item"
      : value === 0
        ? "Not rated"
        : `${value} of 5 stars`;
  if (!onChange) {
    return (
      <span role="img" aria-label={description}>
        {[1, 2, 3, 4, 5].map((star) => (
          <span
            key={star}
            className={star <= shown ? "text-amber-500" : "text-muted-foreground/40"}
          >
            ★
          </span>
        ))}
      </span>
    );
  }
  return (
    <fieldset className="flex" aria-label={`Rating: ${description}`} title={title}>
      {[1, 2, 3, 4, 5].map((star) => (
        <button
          key={star}
          type="button"
          disabled={disabled}
          aria-label={star === value ? `Clear the rating of ${star}` : `Rate ${star} of 5`}
          aria-pressed={star === value}
          className={cn(
            "rounded px-0.5 text-base leading-5 outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
            star <= shown ? "text-amber-500" : "text-muted-foreground/40",
            !disabled && "hover:text-amber-400",
          )}
          onClick={() => onChange(star === value ? 0 : star)}
        >
          ★
        </button>
      ))}
    </fieldset>
  );
}
