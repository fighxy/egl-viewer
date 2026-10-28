import { ChevronDown, ChevronRight, Hash, Plus } from "lucide-react";
import { type ReactNode, useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { FOLDER_COLORS } from "@/features/folders/tree";
import type { Tag, TagGroup } from "@/ipc";
import { countText } from "./match";
import { type TagActions, TagMenu } from "./TagMenu";

const KEY = "egl-viewer.collapsed:";

/** A left bar section that stays collapsed once collapsed (docs/spec.md §7.3). */
function useSectionOpen(name: string): [boolean, (open: boolean) => void] {
  const [open, setOpenState] = useState(() => {
    try {
      return localStorage.getItem(KEY + name) !== "1";
    } catch {
      return true;
    }
  });
  const setOpen = useCallback(
    (next: boolean) => {
      setOpenState(next);
      try {
        if (next) localStorage.removeItem(KEY + name);
        else localStorage.setItem(KEY + name, "1");
      } catch {
        // Storage can be blocked; the section then opens again next time.
      }
    },
    [name],
  );
  return [open, setOpen];
}

function Section({
  label,
  action,
  children,
}: {
  label: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useSectionOpen(label);
  return (
    <section aria-label={label} className="grid gap-px">
      <div className="flex items-center">
        <button
          type="button"
          aria-expanded={open}
          className="flex h-7 flex-1 items-center gap-1 rounded-md px-1 text-left text-xs font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
          onClick={() => setOpen(!open)}
        >
          {open ? (
            <ChevronDown aria-hidden className="size-3.5" />
          ) : (
            <ChevronRight aria-hidden className="size-3.5" />
          )}
          {label}
        </button>
        {action}
      </div>
      {open && children}
    </section>
  );
}

const ROW =
  "flex h-7 w-full items-center gap-2 rounded-md px-2 text-left text-[13px] outline-none hover:bg-sidebar-accent focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=true]:bg-sidebar-accent aria-[current=true]:font-medium";

type Props = {
  groups: readonly TagGroup[];
  recent: readonly string[];
  vocabulary: readonly Tag[];
  /** The tag filter of the screen, for the highlighted row. */
  filtered: readonly string[];
  writable: true | string;
  /** Opens All with this tag as the filter. */
  onPick: (tag: string) => void;
  onCreateGroup: () => void;
  onEditGroup: (id: string) => void;
  onDeleteGroup: (id: string) => void;
  actions?: TagActions;
};

/**
 * Tag groups and Recent tags in the left bar. The full vocabulary is in the Tags filter;
 * choosing a tag here opens All with it as a filter, since tags are filters, not scopes.
 */
export function TagSections({
  groups,
  recent,
  vocabulary,
  filtered,
  writable,
  onPick,
  onCreateGroup,
  onEditGroup,
  onDeleteGroup,
  actions,
}: Props) {
  const counts = new Map(vocabulary.map((tag) => [tag.name, tag.imageCount]));
  const current = filtered.length === 1 ? filtered[0] : undefined;
  const canWrite = writable === true;
  const tagRow = (name: string, label = name) => (
    <li key={name}>
      <TagMenu name={name} actions={actions}>
        <button
          type="button"
          aria-current={name === current ? "true" : undefined}
          aria-label={label}
          className={ROW}
          onClick={() => onPick(name)}
        >
          <Hash aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="min-w-0 flex-1 truncate">{name}</span>
          <span className="text-xs text-muted-foreground tabular-nums">
            {countText(counts.get(name))}
          </span>
        </button>
      </TagMenu>
    </li>
  );
  const recentTags = recent.filter((name) => counts.has(name));
  return (
    <>
      <Section
        label="Tag groups"
        action={
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="New tag group"
            title={canWrite ? "New tag group" : writable}
            disabled={!canWrite}
            onClick={onCreateGroup}
          >
            <Plus aria-hidden />
          </Button>
        }
      >
        {groups.length === 0 ? (
          <p className="px-2 py-1 text-xs text-muted-foreground">No tag groups yet.</p>
        ) : (
          <ul className="grid gap-px">
            {groups.map((group) => (
              <TagGroupRow
                key={group.id}
                group={group}
                canWrite={canWrite}
                onEdit={() => onEditGroup(group.id)}
                onDelete={() => onDeleteGroup(group.id)}
              >
                {group.tags.map((name) => tagRow(name, `${name} in ${group.name}`))}
              </TagGroupRow>
            ))}
          </ul>
        )}
      </Section>
      <Section label="Recent tags">
        {recentTags.length === 0 ? (
          <p className="px-2 py-1 text-xs text-muted-foreground">Tags you add show here.</p>
        ) : (
          <ul className="grid gap-px">{recentTags.map((name) => tagRow(name))}</ul>
        )}
      </Section>
    </>
  );
}

function TagGroupRow({
  group,
  canWrite,
  onEdit,
  onDelete,
  children,
}: {
  group: TagGroup;
  canWrite: boolean;
  onEdit: () => void;
  onDelete: () => void;
  children: ReactNode;
}) {
  const [open, setOpen] = useSectionOpen(`group:${group.id}`);
  const color = group.color ? FOLDER_COLORS[group.color] : undefined;
  return (
    <li>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <button
            type="button"
            aria-expanded={open}
            aria-label={group.name}
            className={ROW}
            onClick={() => setOpen(!open)}
          >
            {open ? (
              <ChevronDown aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
            ) : (
              <ChevronRight aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
            )}
            {color && (
              <span
                aria-hidden
                className="size-2.5 shrink-0 rounded-full"
                style={{ background: color }}
              />
            )}
            <span className="min-w-0 flex-1 truncate">{group.name}</span>
            <span className="text-xs text-muted-foreground tabular-nums">{group.tags.length}</span>
          </button>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem disabled={!canWrite} onSelect={onEdit}>
            Edit Group…
          </ContextMenuItem>
          <ContextMenuItem disabled={!canWrite} onSelect={onDelete}>
            Delete Group…
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      {open && <ul className="grid gap-px pl-4">{children}</ul>}
    </li>
  );
}
