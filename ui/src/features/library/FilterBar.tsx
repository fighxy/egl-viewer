import { ChevronDown, FolderSearch, Star, Tag as TagIcon } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { FolderPicker } from "@/features/folders/FolderPicker";
import { findFolder } from "@/features/folders/tree";
import type { TagActions } from "@/features/tags/TagMenu";
import {
  folderModeHintKey,
  TAG_MODES,
  TagSheet,
  tagModeLabelKey,
} from "@/features/tags/TagSheet";
import type { ExtensionCount, Folder, Tag, TagGroup, TagMode } from "@/ipc";

/** The filters of a screen, on top of its place (docs/spec.md §5). All combine with AND. */
export type Filters = {
  tags: string[];
  tagMode: TagMode;
  folders: string[];
  folderMode: TagMode;
  /** An extension or a bucket (`image`). */
  ext: string | null;
  /** 0 is unrated. */
  rating: number | null;
  shape: string | null;
};

export const NO_FILTERS: Filters = {
  tags: [],
  tagMode: "any",
  folders: [],
  folderMode: "any",
  ext: null,
  rating: null,
  shape: null,
};

/** How many filters are on, for the toolbar's Filter button. */
export function activeFilters(filters: Filters): number {
  return [
    filters.tags.length > 0,
    filters.folders.length > 0,
    filters.ext !== null,
    filters.rating !== null,
    filters.shape !== null,
  ].filter(Boolean).length;
}

/** Eagle's shapes (docs/architecture.md §16.6). Labels are English fallbacks for non-i18n callers. */
export const SHAPES: readonly { value: string; label: string }[] = [
  { value: "square", label: "Square" },
  { value: "landscape", label: "Landscape" },
  { value: "portrait", label: "Portrait" },
  { value: "panoramic-landscape", label: "Panoramic landscape" },
  { value: "panoramic-portrait", label: "Panoramic portrait" },
];

const SHAPE_KEY: Record<string, "filters.square" | "filters.landscape" | "filters.portrait" | "filters.panoramicLandscape" | "filters.panoramicPortrait"> = {
  square: "filters.square",
  landscape: "filters.landscape",
  portrait: "filters.portrait",
  "panoramic-landscape": "filters.panoramicLandscape",
  "panoramic-portrait": "filters.panoramicPortrait",
};

const ANY = "any";

type Props = {
  filters: Filters;
  onChange: (filters: Filters) => void;
  vocabulary: readonly Tag[];
  groups: readonly TagGroup[];
  recent: readonly string[];
  folders: readonly Folder[];
  extensions: readonly ExtensionCount[];
  actions?: TagActions;
  /** Saves the screen as a smart folder; `true`, or why it cannot be saved. */
  canSaveSmart?: true | string;
  onSaveSmart?: () => void;
};

/**
 * The filter bar under the toolbar (docs/spec.md §7.3): Tags, Folders, Type, Rating and Shape,
 * Clear all, and Save as smart folder.
 */
export function FilterBar({
  filters,
  onChange,
  vocabulary,
  groups,
  recent,
  folders,
  extensions,
  actions,
  canSaveSmart = true,
  onSaveSmart,
}: Props) {
  const { t, i18n } = useTranslation();
  const set = (change: Partial<Filters>) => onChange({ ...filters, ...change });
  const modeLabel = (mode: TagMode) => t(tagModeLabelKey(mode));
  const typeLabel = (ext: string) => (ext === "image" ? t("filters.images") : ext.toUpperCase());
  const ratingLabel = (rating: number) =>
    rating === 0 ? t("filters.unrated") : `${rating} ★`;
  const shapeLabel = (value: string) => {
    const key = SHAPE_KEY[value];
    return key ? t(key) : value;
  };
  const tagsLabel =
    filters.tags.length === 0
      ? t("filters.tags")
      : t("filters.tagsNamed", { list: filters.tags.join(", "), mode: modeLabel(filters.tagMode) });
  const folderNames = filters.folders.map(
    (id) => findFolder(folders, id)?.name ?? t("filters.unknown"),
  );
  const foldersLabel =
    filters.folders.length === 0
      ? t("filters.folders")
      : t("filters.foldersNamed", {
          list: folderNames.join(", "),
          mode: modeLabel(filters.folderMode),
        });
  const checked = new Set(filters.folders);
  return (
    <>
      <Popover>
        <PopoverTrigger asChild>
          <FilterButton
            active={filters.tags.length > 0}
            label={tagsLabel}
            icon={<TagIcon aria-hidden />}
          />
        </PopoverTrigger>
        <PopoverContent aria-label={t("filters.tagsFilter")} className="w-72 p-2">
          <TagSheet
            vocabulary={vocabulary}
            groups={groups}
            recent={recent}
            selected={filters.tags}
            mode={filters.tagMode}
            onChange={(tags, tagMode) => set({ tags, tagMode })}
            actions={actions}
          />
        </PopoverContent>
      </Popover>
      <Popover>
        <PopoverTrigger asChild>
          <FilterButton
            active={filters.folders.length > 0}
            label={foldersLabel}
            icon={<FolderSearch aria-hidden />}
          />
        </PopoverTrigger>
        <PopoverContent aria-label={t("filters.foldersFilter")} className="w-72 p-2">
          <div className="grid gap-2">
            <fieldset className="grid grid-cols-4 rounded-md border p-0.5">
              <legend className="sr-only">{t("filters.match")}</legend>
              {TAG_MODES.map((entry) => (
                <button
                  key={entry}
                  type="button"
                  aria-pressed={filters.folderMode === entry}
                  title={t(folderModeHintKey(entry))}
                  className="h-6 rounded text-xs outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring/50 aria-pressed:bg-muted aria-pressed:font-medium"
                  onClick={() => set({ folderMode: entry })}
                >
                  {t(tagModeLabelKey(entry))}
                </button>
              ))}
            </fieldset>
            <FolderPicker
              label={t("filters.folders")}
              folders={[...folders]}
              checked={checked}
              onPick={(id) =>
                set({
                  folders: checked.has(id)
                    ? filters.folders.filter((other) => other !== id)
                    : [...filters.folders, id],
                })
              }
            />
          </div>
        </PopoverContent>
      </Popover>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <FilterButton
            active={filters.ext !== null}
            label={
              filters.ext === null
                ? t("filters.type")
                : t("filters.typeNamed", { label: typeLabel(filters.ext) })
            }
          />
        </DropdownMenuTrigger>
        <DropdownMenuContent aria-label={t("filters.typeFilter")} className="max-h-80 overflow-y-auto">
          <DropdownMenuRadioGroup
            value={filters.ext ?? ANY}
            onValueChange={(value) => set({ ext: value === ANY ? null : value })}
          >
            <DropdownMenuRadioItem value={ANY}>{t("filters.anyType")}</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="image">{t("filters.images")}</DropdownMenuRadioItem>
            {extensions.length > 0 && <DropdownMenuSeparator />}
            {extensions.map((entry) => (
              <DropdownMenuRadioItem key={entry.ext} value={entry.ext}>
                {entry.ext.toUpperCase()}
                <span className="ml-auto pl-4 text-xs text-muted-foreground tabular-nums">
                  {entry.count.toLocaleString(i18n.language)}
                </span>
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <FilterButton
            active={filters.rating !== null}
            label={
              filters.rating === null
                ? t("filters.rating")
                : t("filters.ratingNamed", { label: ratingLabel(filters.rating) })
            }
            icon={<Star aria-hidden />}
          />
        </DropdownMenuTrigger>
        <DropdownMenuContent aria-label={t("filters.ratingFilter")}>
          <DropdownMenuRadioGroup
            value={filters.rating === null ? ANY : String(filters.rating)}
            onValueChange={(value) => set({ rating: value === ANY ? null : Number(value) })}
          >
            <DropdownMenuRadioItem value={ANY}>{t("filters.anyRating")}</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="0">{t("filters.unrated")}</DropdownMenuRadioItem>
            {[1, 2, 3, 4, 5].map((star) => (
              <DropdownMenuRadioItem
                key={star}
                value={String(star)}
                aria-label={t("filters.stars", { count: star })}
              >
                {"★".repeat(star)}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <FilterButton
            active={filters.shape !== null}
            label={
              filters.shape === null
                ? t("filters.shape")
                : t("filters.shapeNamed", { label: shapeLabel(filters.shape) })
            }
          />
        </DropdownMenuTrigger>
        <DropdownMenuContent aria-label={t("filters.shapeFilter")}>
          <DropdownMenuRadioGroup
            value={filters.shape ?? ANY}
            onValueChange={(value) => set({ shape: value === ANY ? null : value })}
          >
            <DropdownMenuRadioItem value={ANY}>{t("filters.anyShape")}</DropdownMenuRadioItem>
            {SHAPES.map((entry) => (
              <DropdownMenuRadioItem key={entry.value} value={entry.value}>
                {shapeLabel(entry.value)}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <span className="ml-auto flex items-center gap-1">
        {activeFilters(filters) > 0 && (
          <Button variant="ghost" size="sm" onClick={() => onChange(NO_FILTERS)}>
            {t("filters.clearAll")}
          </Button>
        )}
        {onSaveSmart && (
          <Button
            variant="ghost"
            size="sm"
            disabled={canSaveSmart !== true}
            title={canSaveSmart === true ? t("filters.saveSmartTitle") : canSaveSmart}
            onClick={onSaveSmart}
          >
            {t("filters.saveSmart")}
          </Button>
        )}
      </span>
    </>
  );
}

function FilterButton({
  active,
  label,
  icon,
  ...props
}: {
  active: boolean;
  label: string;
  icon?: ReactNode;
} & React.ComponentProps<typeof Button>) {
  return (
    <Button
      variant={active ? "secondary" : "outline"}
      size="sm"
      className="max-w-72"
      title={label}
      {...props}
    >
      {icon}
      <span className="truncate">{label}</span>
      <ChevronDown aria-hidden />
    </Button>
  );
}
