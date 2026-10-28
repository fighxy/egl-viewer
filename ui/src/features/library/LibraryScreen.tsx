import { Images, Lock, PanelRight, Plus, Tag, Trash2, Upload, X } from "lucide-react";
import type { ComponentType } from "react";
import type { Availability } from "@/commands/registry";
import { Button } from "@/components/ui/button";
import { plural } from "@/features/imports/importText";
import type { LibraryCounts, LibraryInfo, UiLayout } from "@/ipc";
import { ReadOnlyBanner } from "./ReadOnlyBanner";
import { Splitter } from "./Splitter";

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
};

type Row = {
  id: keyof LibraryCounts;
  label: string;
  icon: ComponentType<{ className?: string }>;
};

const SCOPES: readonly Row[] = [
  { id: "all", label: "All", icon: Images },
  { id: "uncategorized", label: "Uncategorized", icon: Images },
  { id: "untagged", label: "Untagged", icon: Tag },
  { id: "trash", label: "Trash", icon: Trash2 },
];

/**
 * The library screen: left bar, toolbar over the grid, right bar with metadata, status bar
 * (docs/spec.md §7.3). Phase 1 builds the frame; later phases fill the zones.
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
}: Props) {
  const total = counts?.all ?? 0;
  const setLayout = (change: Partial<UiLayout>, persist: boolean) =>
    onLayoutChange({ ...layout, ...change }, persist);

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
            <Button variant="ghost" size="icon-xs" aria-label="Dismiss" onClick={onDismissStatus}>
              <X aria-hidden />
            </Button>
          )}
        </div>
      )}
      <div className="relative flex min-h-0 flex-1">
        {layout.leftOpen && (
          <>
            <nav
              aria-label="Library"
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
                    Read-only
                  </span>
                )}
              </div>
              <ul className="grid gap-px">
                {SCOPES.map(({ id, label, icon: Icon }) => (
                  <li key={id}>
                    <button
                      type="button"
                      aria-current={id === "all" ? "page" : undefined}
                      className="flex h-7 w-full items-center gap-2 rounded-md px-2 text-left outline-none hover:bg-sidebar-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:bg-sidebar-accent aria-[current=page]:font-medium"
                    >
                      <Icon className="size-4 text-muted-foreground" />
                      {label}
                      {counts && (
                        <span className="ml-auto text-xs text-muted-foreground tabular-nums">
                          {counts[id].toLocaleString("en")}
                        </span>
                      )}
                    </button>
                  </li>
                ))}
              </ul>
            </nav>
            <Splitter
              side="left"
              label="Resize the sidebar"
              width={layout.leftWidth}
              min={200}
              max={360}
              onChange={(leftWidth, persist) => setLayout({ leftWidth }, persist)}
            />
          </>
        )}

        <main aria-label="Library contents" className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-11 shrink-0 items-center gap-2 border-b px-3">
            <h1 className="text-sm font-medium">All</h1>
            {counts && (
              <span className="text-[13px] text-muted-foreground">{plural(total, "item")}</span>
            )}
            <div className="ml-auto flex items-center gap-1">
              <Button
                size="sm"
                disabled={canAdd !== true}
                title={canAdd === true ? "Add files" : canAdd}
                onClick={onAddFiles}
              >
                <Plus aria-hidden />
                Add files
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Details panel"
                aria-pressed={layout.rightOpen}
                title="Details panel (Ctrl+I)"
                onClick={() => setLayout({ rightOpen: !layout.rightOpen }, true)}
              >
                <PanelRight aria-hidden />
              </Button>
            </div>
          </div>
          <div className="relative grid flex-1 place-items-center bg-grid p-6">
            {total === 0 ? (
              <div className="grid max-w-sm justify-items-center gap-2 rounded-xl border-2 border-dashed border-border px-10 py-8 text-center">
                <Upload aria-hidden className="size-10 text-muted-foreground" />
                <p className="text-sm font-medium">This library is empty</p>
                <p className="text-[13px] text-muted-foreground">
                  Drop images here, paste with Ctrl+V, or add files.
                </p>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={canAdd !== true}
                  title={canAdd === true ? undefined : canAdd}
                  onClick={onAddFiles}
                >
                  Add files
                </Button>
              </div>
            ) : (
              <div className="grid max-w-sm justify-items-center gap-2 text-center">
                <Images aria-hidden className="size-10 text-muted-foreground" />
                <p className="text-sm font-medium">{plural(total, "item")}</p>
                <p className="text-[13px] text-muted-foreground">
                  The grid that shows them comes in a later version.
                </p>
              </div>
            )}
            {dropping && (
              <div
                role="status"
                aria-label="Drop files"
                className="absolute inset-3 grid place-items-center rounded-xl border-2 border-dashed border-primary bg-background/80 text-sm font-medium duration-150 animate-in fade-in-0"
              >
                {canAdd === true ? "Drop to add to All" : canAdd}
              </div>
            )}
          </div>
        </main>

        {layout.rightOpen && (
          <>
            <Splitter
              side="right"
              label="Resize the details panel"
              width={layout.rightWidth}
              min={280}
              max={480}
              onChange={(rightWidth, persist) => setLayout({ rightWidth }, persist)}
            />
            <aside
              aria-label="Details"
              className="flex shrink-0 flex-col bg-background p-4 text-[13px] max-[1200px]:absolute max-[1200px]:inset-y-0 max-[1200px]:right-0 max-[1200px]:z-10 max-[1200px]:border-l max-[1200px]:shadow-lg"
              style={{ width: layout.rightWidth }}
            >
              <p className="m-auto text-muted-foreground">Select an item</p>
            </aside>
          </>
        )}
      </div>
      <footer className="flex h-6 shrink-0 items-center gap-3 border-t px-3 text-xs text-muted-foreground">
        <span className="truncate">{library.path}</span>
        <span className="ml-auto flex items-center gap-3">
          {importStatus && (
            <button
              type="button"
              className="rounded px-1 outline-none hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
              title="Show imports"
              onClick={onOpenImports}
            >
              {importStatus}
            </button>
          )}
          {library.readOnly && <span>Read-only</span>}
        </span>
      </footer>
    </div>
  );
}
