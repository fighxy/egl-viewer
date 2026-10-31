import { ChevronDown, ChevronRight, Plus, Sparkles, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Section, useSectionOpen } from "@/features/library/Section";
import type { SmartFolder } from "@/ipc";

/** A smart folder and the smart folders above it, top first. */
export function smartPath(tree: readonly SmartFolder[], id: string): SmartFolder[] {
  for (const folder of tree) {
    if (folder.id === id) return [folder];
    const below = smartPath(folder.children, id);
    if (below.length > 0) return [folder, ...below];
  }
  return [];
}

/** Every smart folder id of a tree. */
export function smartIds(tree: readonly SmartFolder[]): Set<string> {
  const ids = new Set<string>();
  const walk = (folders: readonly SmartFolder[]) => {
    for (const folder of folders) {
      ids.add(folder.id);
      walk(folder.children);
    }
  };
  walk(tree);
  return ids;
}

type Props = {
  folders: readonly SmartFolder[];
  /** The open smart folder. */
  selectedId: string | null;
  writable: true | string;
  onSelect: (id: string) => void;
  /** `parent` null: at the top level. */
  onCreate: (parent: string | null) => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
};

/**
 * The Smart folders section of the left bar (docs/spec.md §7.3): their own tree. A smart folder
 * with rules this build cannot evaluate shows a warning that names them.
 */
export function SmartFolders({
  folders,
  selectedId,
  writable,
  onSelect,
  onCreate,
  onEdit,
  onDelete,
}: Props) {
  const { t } = useTranslation();
  const canWrite = writable === true;
  return (
    <Section
      label={t("sidebar.smartFolders")}
      storageKey="Smart folders"
      action={
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="New smart folder"
          title={canWrite ? "New smart folder" : writable}
          disabled={!canWrite}
          onClick={() => onCreate(null)}
        >
          <Plus aria-hidden />
        </Button>
      }
    >
      {folders.length === 0 ? (
        <p className="px-2 py-1 text-xs text-muted-foreground">
          No smart folders yet. Filter the grid and choose Save as smart folder.
        </p>
      ) : (
        <ul className="grid gap-px">
          {folders.map((folder) => (
            <SmartRow
              key={folder.id}
              folder={folder}
              depth={0}
              selectedId={selectedId}
              canWrite={canWrite}
              onSelect={onSelect}
              onCreate={onCreate}
              onEdit={onEdit}
              onDelete={onDelete}
            />
          ))}
        </ul>
      )}
    </Section>
  );
}

function SmartRow({
  folder,
  depth,
  selectedId,
  canWrite,
  onSelect,
  onCreate,
  onEdit,
  onDelete,
}: {
  folder: SmartFolder;
  depth: number;
  selectedId: string | null;
  canWrite: boolean;
  onSelect: (id: string) => void;
  onCreate: (parent: string | null) => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const [open, setOpen] = useSectionOpen(`smart:${folder.id}`);
  const hasChildren = folder.children.length > 0;
  return (
    <li>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div
            className="flex h-7 items-center gap-1 rounded-md pr-2 hover:bg-sidebar-accent has-[[aria-current=page]]:bg-sidebar-accent"
            style={{ paddingLeft: 4 + depth * 14 }}
          >
            {hasChildren ? (
              <button
                type="button"
                aria-label={open ? `Collapse ${folder.name}` : `Expand ${folder.name}`}
                aria-expanded={open}
                className="rounded p-0.5 text-muted-foreground hover:text-foreground"
                onClick={() => setOpen(!open)}
              >
                {open ? (
                  <ChevronDown aria-hidden className="size-3.5" />
                ) : (
                  <ChevronRight aria-hidden className="size-3.5" />
                )}
              </button>
            ) : (
              <span className="w-4.5" />
            )}
            <button
              type="button"
              aria-current={folder.id === selectedId ? "page" : undefined}
              className="flex min-w-0 flex-1 items-center gap-2 text-left text-[13px] outline-none focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:font-medium"
              onClick={() => onSelect(folder.id)}
            >
              <Sparkles aria-hidden className="size-4 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate">{folder.name}</span>
            </button>
            {folder.unsupported.length > 0 && (
              <span
                role="img"
                aria-label="Some rules are not supported"
                title={`Not supported here, so they match nothing: ${folder.unsupported.join(", ")}`}
              >
                <TriangleAlert aria-hidden className="size-3.5 shrink-0 text-amber-500" />
              </span>
            )}
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem disabled={!canWrite} onSelect={() => onCreate(folder.id)}>
            New Smart Subfolder…
          </ContextMenuItem>
          <ContextMenuItem disabled={!canWrite} onSelect={() => onEdit(folder.id)}>
            Edit…
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem disabled={!canWrite} onSelect={() => onDelete(folder.id)}>
            Delete…
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      {hasChildren && open && (
        <ul className="grid gap-px">
          {folder.children.map((child) => (
            <SmartRow
              key={child.id}
              folder={child}
              depth={depth + 1}
              selectedId={selectedId}
              canWrite={canWrite}
              onSelect={onSelect}
              onCreate={onCreate}
              onEdit={onEdit}
              onDelete={onDelete}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
