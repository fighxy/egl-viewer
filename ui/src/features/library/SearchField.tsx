import { Search, X } from "lucide-react";
import { forwardRef, useEffect, useState } from "react";

/** How long typing pauses before the search runs (docs/spec.md §5). */
export const SEARCH_DELAY_MS = 300;

type Props = {
  /** The search the screen shows. */
  value: string;
  /** Called after typing pauses, and at once on Enter, Escape or the clear button. */
  onChange: (text: string) => void;
};

/**
 * The toolbar's search field (docs/spec.md §5): names, notes, links, tags and folder names,
 * from 2 characters, within the current place. `OR`, parentheses and a leading `-` work as in
 * Eagle.
 */
export const SearchField = forwardRef<HTMLInputElement, Props>(function SearchField(
  { value, onChange },
  ref,
) {
  const [text, setText] = useState(value);
  // Another place or library may reset the search from outside.
  useEffect(() => setText(value), [value]);
  useEffect(() => {
    if (text === value) return;
    const timer = window.setTimeout(() => onChange(text), SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [text, value, onChange]);
  return (
    <div className="relative flex items-center">
      <Search
        aria-hidden
        className="pointer-events-none absolute left-2 size-3.5 text-muted-foreground"
      />
      <input
        ref={ref}
        type="search"
        aria-label="Search"
        placeholder="Search…"
        title="Search names, notes, links, tags and folders (Ctrl+F). OR, ( ) and -word work."
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") onChange(text);
          if (event.key === "Escape" && text !== "") {
            event.stopPropagation();
            setText("");
            onChange("");
          }
        }}
        className="h-7 w-36 rounded-md border bg-background pr-6 pl-7 text-[13px] outline-none focus:ring-2 focus:ring-ring/50 [&::-webkit-search-cancel-button]:hidden"
      />
      {text !== "" && (
        <button
          type="button"
          aria-label="Clear the search"
          className="absolute right-1 rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
          onClick={() => {
            setText("");
            onChange("");
          }}
        >
          <X aria-hidden className="size-3" />
        </button>
      )}
    </div>
  );
});
