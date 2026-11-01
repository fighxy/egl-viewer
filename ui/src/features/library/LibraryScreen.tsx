import {
  Filter,
  Images,
  LayoutGrid,
  List,
  Lock,
  PanelRight,
  Plus,
  Tag,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import type { ComponentType, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { Availability } from "@/commands/registry";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Slider } from "@/components/ui/slider";
import { formatBytes } from "@/features/browse/format";
import { SortButton } from "@/features/browse/SortButton";
import { translateAvailability } from "@/i18n/labels";
import {
  type BrowsePreferences,
  DEFAULT_PREFERENCES,
  type LibraryCounts,
  type LibraryInfo,
  type Scope,
  type SortKey,
  THUMBNAIL_SIZES,
  type TrashInfo,
  type UiLayout,
} from "@/ipc";
import { ReadOnlyBanner } from "./ReadOnlyBanner";
import { Splitter } from "./Splitter";

/** The fixed rows of the left bar; their ids are the keys of `LibraryCounts`. */
export type ScopeId = keyof LibraryCounts;

type Props = {
  library: LibraryInfo;
  layout: UiLayout;
  /** `persist` is false while a bar is being dragged. */
  onLayoutChange: (layout: UiLayout, persist: boolean) => void;
  onOpenForEditing: () => void;
  /** A message for the user, for example a failed Open Recent. */
  status?: string;
  onDismissStatus?: () => void;
  counts?: LibraryCounts | null;
  /** `true`, or why files cannot be added (read-only library). */
  canAdd?: Availability;
  onAddFiles?: () => void;
  /** The running or last import, for the status bar; a click opens the Imports panel. */
  importStatus?: string;
  onOpenImports?: () => void;
  /** Files are being dragged over the window. */
  dropping?: boolean;
  /** Where the user is: a fixed row of the left bar or a folder. */
  place?: Place;
  onPlaceChange?: (place: Place) => void;
  /** The toolbar's title, for example the path of a nested folder. */
  title?: string;
  /** The Folders section of the left bar. */
  folderTree?: ReactNode;
  /** The Locked folders section of the left bar. */
  lockedFolders?: ReactNode;
  /** The Smart folders section of the left bar. */
  smartFolders?: ReactNode;
  /** The search field of the toolbar. */
  search?: ReactNode;
  /** The sort of this screen: a folder's own sort, or the global one. */
  sort?: SortKey;
  ascending?: boolean;
  onSortChange?: (sort: SortKey, ascending: boolean) => void;
  /** Where dropped files go, for the overlay. */
  dropLabel?: string;
  browse?: BrowsePreferences;
  /** `persist` is false while the size slider is being dragged. */
  onBrowseChange?: (browse: BrowsePreferences, persist: boolean) => void;
  /** Items in the open view; `undefined` while it opens. */
  itemCount?: number;
  selectedCount?: number;
  /** The grid or the list, shown when the view has items. */
  content?: ReactNode;
  /** The right bar's contents. */
  details?: ReactNode;
  /** Why the view could not open, or how far its scope loaded. */
  viewStatus?: string;
  /** Tag groups and Recent tags in the left bar. */
  tagSections?: ReactNode;
  /** The filter bar's contents. */
  filterBar?: ReactNode;
  /** How many filters are on. */
  filterCount?: number;
  /** A search or a filter narrows the place: an empty result says so. */
  narrowed?: boolean;
  /** The Filter button shows the bar even with no filter on. */
  filterOpen?: boolean;
  onFilterOpenChange?: (open: boolean) => void;
  onClearFilters?: () => void;
  /** Buttons of the open place in the toolbar, for example Empty Trash. */
  placeActions?: ReactNode;
  /** Items and bytes of the trash, for the context menu of its row. */
  trashInfo?: TrashInfo | null;
  /** Restore All of the trash context menu: every item comes back. */
  onRestoreTrash?: () => void;
  /** Empty Trash… of the trash context menu (asks first). */
  onEmptyTrash?: () => void;
};

type Row = {
  id: ScopeId;
  labelKey: "all" | "uncategorized" | "untagged" | "trash";
  icon: ComponentType<{ className?: string }>;
};

const SCOPES: readonly Row[] = [
  { id: "all", labelKey: "all", icon: Images },
  { id: "uncategorized", labelKey: "uncategorized", icon: Images },
  { id: "untagged", labelKey: "untagged", icon: Tag },
  { id: "trash", labelKey: "trash", icon: Trash2 },
];

/** A fixed row of the left bar, a folder or a smart folder. */
export type Place =
  | { kind: ScopeId }
  | { kind: "folder"; id: string }
  | { kind: "smart"; id: string };

/** The query scope of a place. */
export function scopeOf(place: Place, includeSubfolders = false): Scope {
  switch (place.kind) {
    case "folder":
      return { kind: "folder", id: place.id, includeDescendants: includeSubfolders };
    case "smart":
      return { kind: "smart", id: place.id };
    case "all":
      return { kind: "all" };
    case "uncategorized":
      return { kind: "unfiled" };
    case "untagged":
      return { kind: "untagged" };
    case "trash":
      return { kind: "trash" };
  }
}

/**
 * The library screen: left bar, toolbar over the grid, right bar with metadata, status bar
 * (docs/spec.md §7.3).
 */
export function LibraryScreen({
  library,
  layout,
  onLayoutChange,
  onOpenForEditing,
  status,
  onDismissStatus,
  counts,
  canAdd = true,
  onAddFiles,
  importStatus,
  onOpenImports,
  dropping = false,
  place = { kind: "all" },
  onPlaceChange,
  title,
  folderTree,
  lockedFolders,
  smartFolders,
  search,
  sort,
  ascending,
  onSortChange,
  dropLabel = "All",
  browse = DEFAULT_PREFERENCES.browse,
  onBrowseChange,
  itemCount,
  selectedCount = 0,
  content,
  details,
  viewStatus,
  tagSections,
  filterBar,
  filterCount = 0,
  narrowed = filterCount > 0,
  filterOpen = false,
  onFilterOpenChange,
  onClearFilters,
  placeActions,
  trashInfo,
  onRestoreTrash,
  onEmptyTrash,
}: Props) {
  const { t, i18n } = useTranslation();
  const itemLabel = (count: number) =>
    t("common.item", { count, formatted: count.toLocaleString(i18n.language) });
  const canAddTitle = canAdd === true ? undefined : (translateAvailability(t, canAdd) as string);
  const total =
    itemCount ??
    (place.kind === "folder" || place.kind === "smart" ? undefined : counts?.[place.kind]) ??
    0;
  const libraryEmpty = (counts?.all ?? 0) === 0 && (itemCount ?? 0) === 0;
  const scopeLabel = (id: ScopeId) =>
    t(`sidebar.${SCOPES.find((row) => row.id === id)?.labelKey ?? "all"}`);
  const heading =
    title ??
    (place.kind === "folder" || place.kind === "smart" ? undefined : scopeLabel(place.kind)) ??
    t("sidebar.all");
  const shownSort = sort ?? browse.sort;
  const shownAscending = ascending ?? browse.ascending;
  const setLayout = (change: Partial<UiLayout>, persist: boolean) =>
    onLayoutChange({ ...layout, ...change }, persist);
  const setBrowse = (change: Partial<BrowsePreferences>, persist = true) =>
    onBrowseChange?.({ ...browse, ...change }, persist);
  const known = itemCount !== undefined || counts;

  const barShown = filterBar !== undefined && (filterOpen || filterCount > 0);
  const trashCount = trashInfo?.items ?? counts?.trash ?? 0;
  const emptyScope: Record<Exclude<ScopeId, "all">, string> = {
    uncategorized: t("empty.uncategorized"),
    untagged: t("empty.untagged"),
    trash: t("empty.trashEmpty"),
  };

  let body: ReactNode;
  if (total > 0 && content) {
    body = content;
  } else if (viewStatus) {
    body = <p className="m-auto text-[13px] text-muted-foreground">{viewStatus}</p>;
  } else if (narrowed && itemCount === 0) {
    body = (
      <div className="m-auto grid justify-items-center gap-2 text-center">
        <p className="text-[13px] text-muted-foreground">
          {filterCount > 0 ? t("empty.noFilterMatch") : t("empty.noSearchMatch")}
        </p>
        {onClearFilters && (
          <Button size="sm" variant="outline" onClick={onClearFilters}>
            {t("empty.clearFilters")}
          </Button>
        )}
      </div>
    );
  } else if (place.kind === "all" && libraryEmpty) {
    body = (
      <div className="m-auto grid max-w-sm justify-items-center gap-2 rounded-xl border-2 border-dashed border-border px-10 py-8 text-center">
        <Upload aria-hidden className="size-10 text-muted-foreground" />
        <p className="text-sm font-medium">{t("empty.libraryEmpty")}</p>
        <p className="text-[13px] text-muted-foreground">{t("empty.libraryEmptyHint")}</p>
        <Button
          size="sm"
          variant="outline"
          disabled={canAdd !== true}
          title={canAddTitle}
          onClick={onAddFiles}
        >
          {t("empty.addFiles")}
        </Button>
      </div>
    );
  } else if (total === 0 && place.kind !== "all") {
    body = (
      <p className="m-auto max-w-xs text-center text-[13px] text-muted-foreground">
        {place.kind === "folder"
          ? `${t("empty.folderEmpty")}${canAdd === true ? t("empty.folderEmptyHint") : ""}`
          : place.kind === "smart"
            ? t("empty.smartEmpty")
            : emptyScope[place.kind]}
      </p>
    );
  } else {
    body = null;
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <ReadOnlyBanner library={library} onOpenForEditing={onOpenForEditing} />
      {status && (
        <div
          role="alert"
          className="flex items-center gap-3 border-b bg-destructive/10 px-3 py-2 text-[13px]"
        >
          <p className="flex-1">{status}</p>
          {onDismissStatus && (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={t("common.dismiss")}
              onClick={onDismissStatus}
            >
              <X aria-hidden />
            </Button>
          )}
        </div>
      )}
      <div className="relative flex min-h-0 flex-1">
        {layout.leftOpen && (
          <>
            <nav
              aria-label={t("sidebar.library")}
              className="flex shrink-0 flex-col gap-3 overflow-y-auto bg-sidebar p-2 text-[13px] text-sidebar-foreground"
              style={{ width: layout.leftWidth }}
            >
              <div className="flex items-center gap-2 px-2 pt-1">
                <h2 className="truncate font-medium" title={library.path ?? undefined}>
                  {library.name}
                </h2>
                {library.readOnly && (
                  <span className="flex items-center gap-1 rounded bg-muted px-1.5 text-xs text-muted-foreground">
                    <Lock aria-hidden className="size-3" />
                    {t("common.readOnly")}
                  </span>
                )}
              </div>
              <ul className="grid gap-px">
                {SCOPES.map(({ id, labelKey, icon: Icon }) => {
                  const label = t(`sidebar.${labelKey}`);
                  const row = (
                    <button
                      type="button"
                      aria-current={id === place.kind ? "page" : undefined}
                      className="flex h-7 w-full items-center gap-2 rounded-md px-2 text-left outline-none hover:bg-sidebar-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:bg-sidebar-accent aria-[current=page]:font-medium"
                      onClick={() => onPlaceChange?.({ kind: id })}
                    >
                      <Icon className="size-4 text-muted-foreground" />
                      {label}
                      {counts && (
                        <span className="ml-auto text-xs text-muted-foreground tabular-nums">
                          {counts[id].toLocaleString(i18n.language)}
                        </span>
                      )}
                    </button>
                  );
                  if (id !== "trash") return <li key={id}>{row}</li>;
                  const trashDisabled = trashCount === 0 || library.readOnly;
                  const trashTitle = library.readOnly
                    ? t("availability.readOnly")
                    : trashCount === 0
                      ? t("empty.trashEmpty")
                      : undefined;
                  return (
                    <li key={id}>
                      <ContextMenu>
                        <ContextMenuTrigger asChild>{row}</ContextMenuTrigger>
                        <ContextMenuContent>
                          <p className="px-2 py-1 text-xs text-muted-foreground">
                            {trashInfo
                              ? `${itemLabel(trashInfo.items)} · ${formatBytes(trashInfo.bytes)}`
                              : counts
                                ? itemLabel(counts.trash)
                                : t("sidebar.trash")}
                          </p>
                          <ContextMenuSeparator />
                          <ContextMenuItem
                            disabled={trashDisabled}
                            title={trashTitle}
                            onSelect={() => onRestoreTrash?.()}
                          >
                            {t("sidebar.restoreAll")}
                          </ContextMenuItem>
                          <ContextMenuItem
                            disabled={trashDisabled}
                            title={trashTitle}
                            className="text-destructive focus:text-destructive"
                            onSelect={() => onEmptyTrash?.()}
                          >
                            {t("sidebar.emptyTrash")}
                          </ContextMenuItem>
                        </ContextMenuContent>
                      </ContextMenu>
                    </li>
                  );
                })}
              </ul>
              {folderTree}
              {lockedFolders}
              {smartFolders}
              {tagSections}
            </nav>
            <Splitter
              side="left"
              label={t("library.resizeSidebar")}
              width={layout.leftWidth}
              min={200}
              max={360}
              onChange={(leftWidth, persist) => setLayout({ leftWidth }, persist)}
            />
          </>
        )}

        <main aria-label={t("library.contents")} className="flex min-w-0 flex-1 flex-col">
          {/* A container: a narrow toolbar drops the size slider (Ctrl+wheel and Ctrl+= stay). */}
          <div className="@container flex h-11 shrink-0 items-center gap-2 border-b px-3">
            <div className="flex min-w-0 items-center gap-2">
              <h1 className="min-w-10 truncate text-sm font-medium" title={heading}>
                {heading}
              </h1>
              {known && (
                <span className="shrink-0 text-[13px] whitespace-nowrap text-muted-foreground">
                  {itemLabel(total)}
                </span>
              )}
            </div>
            {/* Centered in the free space: wider than a short box, not stretched across the row. */}
            <div className="flex min-w-0 flex-1 justify-center">
              {search ? <div className="w-96 min-w-0 max-w-full">{search}</div> : null}
            </div>
            <div className="flex shrink-0 items-center gap-1">
              {filterBar !== undefined && (
                <Button
                  variant="ghost"
                  size="sm"
                  aria-pressed={barShown}
                  title={t("library.showFilters")}
                  className="aria-pressed:bg-muted"
                  onClick={() => onFilterOpenChange?.(!filterOpen)}
                >
                  <Filter aria-hidden />
                  {filterCount > 0
                    ? t("library.filterCount", { count: filterCount })
                    : t("library.filter")}
                </Button>
              )}
              <SortButton
                sort={shownSort}
                ascending={shownAscending}
                manual={place.kind === "folder"}
                onSort={(next) =>
                  onSortChange ? onSortChange(next, shownAscending) : setBrowse({ sort: next })
                }
                onAscending={(next) =>
                  onSortChange ? onSortChange(shownSort, next) : setBrowse({ ascending: next })
                }
              />
              <div className="flex items-center rounded-md border p-0.5">
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={t("commands.view.grid")}
                  aria-pressed={browse.viewMode === "grid"}
                  className="aria-pressed:bg-muted"
                  onClick={() => setBrowse({ viewMode: "grid" })}
                >
                  <LayoutGrid aria-hidden />
                </Button>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={t("commands.view.list")}
                  aria-pressed={browse.viewMode === "list"}
                  className="aria-pressed:bg-muted"
                  onClick={() => setBrowse({ viewMode: "list" })}
                >
                  <List aria-hidden />
                </Button>
              </div>
              {browse.viewMode === "grid" && (
                <Slider
                  aria-label="Thumbnail size"
                  title="Thumbnail size (Ctrl+= / Ctrl+-)"
                  className="mx-1 w-20 @max-3xl:hidden"
                  min={THUMBNAIL_SIZES.min}
                  max={THUMBNAIL_SIZES.max}
                  step={10}
                  value={[browse.thumbnailSize]}
                  onValueChange={([size]) => {
                    if (size !== undefined) setBrowse({ thumbnailSize: size }, false);
                  }}
                  onValueCommit={([size]) => {
                    if (size !== undefined) setBrowse({ thumbnailSize: size });
                  }}
                />
              )}
              {placeActions}
              {place.kind !== "trash" && (
                <Button
                  size="sm"
                  disabled={canAdd !== true}
                  title={canAdd === true ? t("empty.addFiles") : canAddTitle}
                  onClick={onAddFiles}
                >
                  <Plus aria-hidden />
                  {t("empty.addFiles")}
                </Button>
              )}
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Details panel"
                aria-pressed={layout.rightOpen}
                title={`${t("commands.view.details")} (Ctrl+I)`}
                onClick={() => setLayout({ rightOpen: !layout.rightOpen }, true)}
              >
                <PanelRight aria-hidden />
              </Button>
            </div>
          </div>
          {barShown && (
            <div
              role="toolbar"
              aria-label={t("library.filters")}
              className="flex min-h-10 shrink-0 flex-wrap items-center gap-2 border-b px-3 py-1.5"
            >
              {filterBar}
            </div>
          )}
          <div className="relative flex min-h-0 flex-1 flex-col bg-grid">
            {body}
            {dropping && (
              <div
                role="status"
                aria-label={t("empty.dropFiles")}
                className="absolute inset-3 grid place-items-center rounded-xl border-2 border-dashed border-primary bg-background/80 text-sm font-medium duration-150 animate-in fade-in-0"
              >
                {canAdd === true ? `Drop to add to ${dropLabel}` : canAdd}
              </div>
            )}
          </div>
        </main>

        {layout.rightOpen && (
          <>
            <Splitter
              side="right"
              label={t("library.resizeDetails")}
              width={layout.rightWidth}
              min={280}
              max={480}
              onChange={(rightWidth, persist) => setLayout({ rightWidth }, persist)}
            />
            <aside
              aria-label={t("widgetKeys.details")}
              className="flex shrink-0 flex-col bg-background p-4 text-[13px] max-[1200px]:absolute max-[1200px]:inset-y-0 max-[1200px]:right-0 max-[1200px]:z-10 max-[1200px]:border-l max-[1200px]:shadow-lg"
              style={{ width: layout.rightWidth }}
            >
              {details ?? <p className="m-auto text-muted-foreground">{t("empty.selectItem")}</p>}
            </aside>
          </>
        )}
      </div>
      <footer className="flex h-6 shrink-0 items-center gap-3 border-t px-3 text-xs text-muted-foreground">
        {known && <span>{itemLabel(total)}</span>}
        {selectedCount > 0 && (
          <span>
            {t("common.selected", {
              count: selectedCount,
              formatted: selectedCount.toLocaleString(i18n.language),
            })}
          </span>
        )}
        <span className="ml-auto flex items-center gap-3">
          {importStatus && (
            <button
              type="button"
              className="rounded px-1 outline-none hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
              title={t("library.showImports")}
              onClick={onOpenImports}
            >
              {importStatus}
            </button>
          )}
          {library.readOnly && <span>{t("common.readOnly")}</span>}
        </span>
      </footer>
    </div>
  );
}
