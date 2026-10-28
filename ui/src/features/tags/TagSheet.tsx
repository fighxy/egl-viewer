import { cn } from "cn";
import { Check, X } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { FOLDER_COLORS } from "@/features/folders/tree";
import type { Tag, TagGroup, TagMode } from "@/ipc";
import { countText, matchTags } from "./match";
import { type TagActions, TagMenu } from "./TagMenu";

/** Rows of the full vocabulary drawn at once; typing narrows a longer list. */
export const VOCABULARY_SHOWN = 300;

export const TAG_MODES: readonly { mode: TagMode; label: string; hint: string }[] = [
  { mode: "any", label: "Any", hint: "At least one of the tags" },
  { mode: "all", label: "All", hint: "Every one of the tags" },
  { mode: "exact", label: "Exact", hint: "These tags and no others" },
  { mode: "none", label: "None", hint: "None of the tags" },
];

type Props = {
  vocabulary: readonly Tag[];
  groups: readonly TagGroup[];
  recent: readonly string[];
  selected: readonly string[];
  mode: TagMode;
  onChange: (tags: string[], mode: TagMode) => void;
  /** Rename, merge and delete from a row's context menu; absent in a read-only library. */
  actions?: TagActions;
};

/**
 * The Tags filter (docs/spec.md §5, §7.3): the vocabulary with counts, the match mode, recent
 * tags and tag groups. Tags are compared as exact strings; the search ignores case.
 */
export function TagSheet({ vocabulary, groups, recent, selected, mode, onChange, actions }: Props) {
  const [search, setSearch] = useState("");
  const chosen = new Set(selected);
  const counts = new Map(vocabulary.map((tag) => [tag.name, tag.imageCount]));
  const toggle = (name: string) =>
    onChange(chosen.has(name) ? selected.filter((tag) => tag !== name) : [...selected, name], mode);
  const found = matchTags(vocabulary, search);
  const recentTags = recent.filter((name) => counts.has(name));

  const rows = (label: string, names: readonly string[]) => (
    <div role="listbox" aria-label={label} aria-multiselectable className="grid gap-px">
      {names.map((name) => (
        <TagMenu key={name} name={name} actions={actions}>
          <button
            type="button"
            role="option"
            aria-selected={chosen.has(name)}
            aria-label={name}
            className="flex h-7 items-center gap-2 rounded-md px-2 text-left text-[13px] outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring/50"
            onClick={() => toggle(name)}
          >
            <Check
              aria-hidden
              className={cn("size-3.5 shrink-0", !chosen.has(name) && "invisible")}
            />
            <span className="min-w-0 flex-1 truncate">{name}</span>
            <span className="text-xs text-muted-foreground tabular-nums">
              {countText(counts.get(name))}
            </span>
          </button>
        </TagMenu>
      ))}
    </div>
  );

  return (
    <div className="grid gap-2">
      <fieldset className="grid grid-cols-4 rounded-md border p-0.5">
        <legend className="sr-only">Match</legend>
        {TAG_MODES.map((entry) => (
          <button
            key={entry.mode}
            type="button"
            aria-pressed={mode === entry.mode}
            title={entry.hint}
            className="h-6 rounded text-xs outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring/50 aria-pressed:bg-muted aria-pressed:font-medium"
            onClick={() => onChange([...selected], entry.mode)}
          >
            {entry.label}
          </button>
        ))}
      </fieldset>
      <input
        aria-label="Find a tag"
        placeholder="Find a tag"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
        className="h-7 rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50"
      />
      <div className="grid max-h-80 gap-2 overflow-y-auto">
        {search.trim() === "" && recentTags.length > 0 && (
          <section className="grid gap-1">
            <h4 className="px-2 text-xs font-medium text-muted-foreground">Recent</h4>
            {rows("Recent tags", recentTags)}
          </section>
        )}
        {search.trim() === "" &&
          groups
            .filter((group) => group.tags.length > 0)
            .map((group) => (
              <section key={group.id} className="grid gap-1">
                <h4 className="flex items-center gap-1.5 px-2 text-xs font-medium text-muted-foreground">
                  {group.color && (
                    <span
                      aria-hidden
                      className="size-2 rounded-full"
                      style={{ background: FOLDER_COLORS[group.color] }}
                    />
                  )}
                  {group.name}
                </h4>
                {rows(group.name, group.tags)}
              </section>
            ))}
        <section className="grid gap-1">
          <h4 className="px-2 text-xs font-medium text-muted-foreground">All tags</h4>
          {rows(
            "All tags",
            found.slice(0, VOCABULARY_SHOWN).map((tag) => tag.name),
          )}
          {found.length > VOCABULARY_SHOWN && (
            <p className="px-2 py-1 text-xs text-muted-foreground">
              {(found.length - VOCABULARY_SHOWN).toLocaleString("en")} more. Type to narrow the
              list.
            </p>
          )}
          {found.length === 0 && (
            <p className="px-2 py-1 text-xs text-muted-foreground">
              {vocabulary.length === 0 ? "No tags yet." : "No tag has that name."}
            </p>
          )}
        </section>
      </div>
      {/* Below the lists: picking a tag never moves the rows under the pointer. */}
      {selected.length > 0 && (
        <div className="flex flex-wrap items-center gap-1">
          {selected.map((name) => (
            <span
              key={name}
              className="flex items-center gap-0.5 rounded bg-muted py-0.5 pr-0.5 pl-1.5 text-xs"
            >
              {name}
              <button
                type="button"
                aria-label={`Stop filtering by ${name}`}
                className="rounded p-0.5 hover:bg-background"
                onClick={() => toggle(name)}
              >
                <X aria-hidden className="size-3" />
              </button>
            </span>
          ))}
          <Button variant="ghost" size="xs" onClick={() => onChange([], mode)}>
            Clear
          </Button>
        </div>
      )}
    </div>
  );
}
