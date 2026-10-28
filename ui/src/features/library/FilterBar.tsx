import { ChevronDown, Tag as TagIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { TagActions } from "@/features/tags/TagMenu";
import { TAG_MODES, TagSheet } from "@/features/tags/TagSheet";
import type { Tag, TagGroup, TagMode } from "@/ipc";

/** The tag filter of a screen: exact names and how they match. */
export type TagFilter = { tags: string[]; mode: TagMode };

export const NO_TAG_FILTER: TagFilter = { tags: [], mode: "any" };

/** How many filters are on, for the toolbar's Filter button. */
export function activeFilters(filter: TagFilter): number {
  return filter.tags.length > 0 ? 1 : 0;
}

type Props = {
  filter: TagFilter;
  onChange: (filter: TagFilter) => void;
  vocabulary: readonly Tag[];
  groups: readonly TagGroup[];
  recent: readonly string[];
  actions?: TagActions;
};

/**
 * The filter bar under the toolbar (docs/spec.md §7.3). Phase 5 has the Tags filter; type,
 * rating, shape and folders join this row in phase 6.
 */
export function FilterBar({ filter, onChange, vocabulary, groups, recent, actions }: Props) {
  const mode = TAG_MODES.find((entry) => entry.mode === filter.mode)?.label ?? "Any";
  const label = filter.tags.length === 0 ? "Tags" : `Tags: ${filter.tags.join(", ")} · ${mode}`;
  return (
    <>
      <Popover>
        <PopoverTrigger asChild>
          <Button
            variant={filter.tags.length > 0 ? "secondary" : "outline"}
            size="sm"
            className="max-w-80"
            title={label}
          >
            <TagIcon aria-hidden />
            <span className="truncate">{label}</span>
            <ChevronDown aria-hidden />
          </Button>
        </PopoverTrigger>
        <PopoverContent aria-label="Tags filter" className="w-72 p-2">
          <TagSheet
            vocabulary={vocabulary}
            groups={groups}
            recent={recent}
            selected={filter.tags}
            mode={filter.mode}
            onChange={(tags, next) => onChange({ tags, mode: next })}
            actions={actions}
          />
        </PopoverContent>
      </Popover>
      {activeFilters(filter) > 0 && (
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto"
          onClick={() => onChange({ ...filter, tags: [] })}
        >
          Clear all
        </Button>
      )}
    </>
  );
}
