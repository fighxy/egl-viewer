import { Images, Lock, PanelRight, Tag, Trash2, X } from "lucide-react";
import type { ComponentType } from "react";
import { Button } from "@/components/ui/button";
import type { LibraryInfo, UiLayout } from "@/ipc";
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
};

type Row = { id: string; label: string; icon: ComponentType<{ className?: string }> };

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
}: Props) {
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
            <div className="ml-auto">
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
          <div className="grid flex-1 place-items-center bg-grid p-6">
            <div className="grid max-w-sm justify-items-center gap-2 text-center">
              <Images aria-hidden className="size-10 text-muted-foreground" />
              <p className="text-sm font-medium">This library is empty</p>
              <p className="text-[13px] text-muted-foreground">
                Adding files comes in a later version.
              </p>
            </div>
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
        {library.readOnly && <span className="ml-auto">Read-only</span>}
      </footer>
    </div>
  );
}
