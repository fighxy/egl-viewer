import { X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ColourPicker } from "@/features/folders/ColourPicker";
import type { Tag, TagGroup, TagGroupChange } from "@/ipc";
import { cleanTag } from "./match";
import { TagInput } from "./TagInput";

/** Which tag dialog is open. */
export type TagDialog =
  | { kind: "rename"; name: string }
  | { kind: "merge"; name: string }
  | { kind: "delete"; name: string }
  | { kind: "group"; id: string | null }
  | { kind: "deleteGroup"; id: string };

type Props = {
  dialog: TagDialog | null;
  vocabulary: readonly Tag[];
  groups: readonly TagGroup[];
  onClose: () => void;
  onRename: (from: string, to: string) => void;
  onMerge: (source: string, target: string) => void;
  onDelete: (name: string) => void;
  onSaveGroup: (change: TagGroupChange) => void;
  onDeleteGroup: (id: string) => void;
};

const FIELD =
  "h-8 rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50";

/** Rename, Merge Into and Delete a tag everywhere; create, edit and delete tag groups. */
export function TagDialogs({
  dialog,
  vocabulary,
  groups,
  onClose,
  onRename,
  onMerge,
  onDelete,
  onSaveGroup,
  onDeleteGroup,
}: Props) {
  const { t, i18n } = useTranslation();
  const close = (open: boolean) => {
    if (!open) onClose();
  };
  const count = (name: string) => vocabulary.find((tag) => tag.name === name)?.imageCount ?? 0;
  const itemsLabel = (n: number) =>
    t("common.item", { count: n, formatted: n.toLocaleString(i18n.language) });
  const group =
    dialog?.kind === "group" || dialog?.kind === "deleteGroup"
      ? groups.find((candidate) => candidate.id === dialog.id)
      : undefined;
  return (
    <>
      <Dialog open={dialog?.kind === "rename"} onOpenChange={close}>
        {dialog?.kind === "rename" && (
          <RenameTag
            name={dialog.name}
            vocabulary={vocabulary}
            onRename={(to) => onRename(dialog.name, to)}
          />
        )}
      </Dialog>
      <Dialog open={dialog?.kind === "merge"} onOpenChange={close}>
        {dialog?.kind === "merge" && (
          <MergeTag
            name={dialog.name}
            count={count(dialog.name)}
            vocabulary={vocabulary}
            onCancel={onClose}
            onMerge={(target) => onMerge(dialog.name, target)}
          />
        )}
      </Dialog>
      <Dialog open={dialog?.kind === "delete"} onOpenChange={close}>
        {dialog?.kind === "delete" && (
          <DialogContent showCloseButton={false}>
            <DialogHeader>
              <DialogTitle>{t("tagsUi.deleteTitle", { name: dialog.name })}</DialogTitle>
              <DialogDescription>
                {t("tagsUi.deleteBody", { items: itemsLabel(count(dialog.name)) })}
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant="outline" autoFocus onClick={onClose}>
                {t("common.cancel")}
              </Button>
              <Button variant="destructive" onClick={() => onDelete(dialog.name)}>
                {t("tagsUi.deleteTag")}
              </Button>
            </DialogFooter>
          </DialogContent>
        )}
      </Dialog>
      <Dialog open={dialog?.kind === "group"} onOpenChange={close}>
        {dialog?.kind === "group" && (
          <GroupDialog group={group} vocabulary={vocabulary} onSave={onSaveGroup} />
        )}
      </Dialog>
      <Dialog open={dialog?.kind === "deleteGroup"} onOpenChange={close}>
        {dialog?.kind === "deleteGroup" && (
          <DialogContent showCloseButton={false}>
            <DialogHeader>
              <DialogTitle>{t("tagsUi.deleteGroupTitle", { name: group?.name ?? "" })}</DialogTitle>
              <DialogDescription>{t("tagsUi.deleteGroupBody")}</DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant="outline" autoFocus onClick={onClose}>
                {t("common.cancel")}
              </Button>
              <Button variant="destructive" onClick={() => onDeleteGroup(dialog.id)}>
                {t("tagsUi.deleteGroup")}
              </Button>
            </DialogFooter>
          </DialogContent>
        )}
      </Dialog>
    </>
  );
}

function RenameTag({
  name,
  vocabulary,
  onRename,
}: {
  name: string;
  vocabulary: readonly Tag[];
  onRename: (to: string) => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState(name);
  const to = cleanTag(text);
  // Exact strings: "poster" → "Poster" is a rename unless "Poster" exists too.
  const merges = to !== name && vocabulary.some((tag) => tag.name === to);
  return (
    <DialogContent>
      <form
        className="grid gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (to && to !== name) onRename(to);
        }}
      >
        <DialogHeader>
          <DialogTitle>{t("tagsUi.renameTitle", { name })}</DialogTitle>
          <DialogDescription>{t("tagsUi.renameHint")}</DialogDescription>
        </DialogHeader>
        <input
          aria-label={t("tagsUi.renameAria")}
          // biome-ignore lint/a11y/noAutofocus: the dialog asks for this one field.
          autoFocus
          value={text}
          onChange={(event) => setText(event.target.value)}
          className={FIELD}
        />
        {merges && (
          <p role="status" className="text-[13px] text-muted-foreground">
            {t("tagsUi.renameMergeHint", { name: to })}
          </p>
        )}
        <DialogFooter>
          <Button type="submit" disabled={!to || to === name}>
            {merges ? t("tagsUi.merge") : t("commonExtra.rename")}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  );
}

function MergeTag({
  name,
  count,
  vocabulary,
  onCancel,
  onMerge,
}: {
  name: string;
  count: number;
  vocabulary: readonly Tag[];
  onCancel: () => void;
  onMerge: (target: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const [target, setTarget] = useState<string | null>(null);
  const others = vocabulary.filter((tag) => tag.name !== name);
  const items = t("common.item", {
    count,
    formatted: count.toLocaleString(i18n.language),
  });
  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{t("tagsUi.mergeTitle", { name })}</DialogTitle>
        <DialogDescription>{t("tagsUi.mergeBody", { items, name })}</DialogDescription>
      </DialogHeader>
      <TagInput
        label={t("tagsUi.mergeInto")}
        placeholder={t("tagsUi.findTag")}
        vocabulary={others}
        existingOnly
        keepText
        autoFocus
        onPick={setTarget}
      />
      {target && (
        <p role="status" className="text-[13px]">
          {t("tagsUi.mergeConfirm", { name, target })}
        </p>
      )}
      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          {t("common.cancel")}
        </Button>
        <Button disabled={target === null} onClick={() => target && onMerge(target)}>
          {t("tagsUi.merge")}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}

function GroupDialog({
  group,
  vocabulary,
  onSave,
}: {
  group?: TagGroup;
  vocabulary: readonly Tag[];
  onSave: (change: TagGroupChange) => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(group?.name ?? "");
  const [color, setColor] = useState<string | null>(group?.color ?? null);
  const [tags, setTags] = useState<string[]>(group?.tags ?? []);
  const save = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    onSave(
      group
        ? { type: "update", id: group.id, name: trimmed, color, tags }
        : { type: "create", name: trimmed, color, tags },
    );
  };
  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>
          {group ? t("tagsUi.editGroup", { name: group.name }) : t("tagsUi.newGroup")}
        </DialogTitle>
        <DialogDescription>{t("tagsUi.editGroupHint")}</DialogDescription>
      </DialogHeader>
      <div className="grid gap-3">
        <input
          aria-label={t("tagsUi.groupNameAria")}
          placeholder={t("tagsUi.name")}
          // biome-ignore lint/a11y/noAutofocus: the first field of the dialog.
          autoFocus
          value={name}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") save();
          }}
          className={FIELD}
        />
        <ColourPicker legend={t("tagsUi.colour")} value={color} onChange={setColor} />
        <div className="grid gap-1.5">
          <span className="text-xs text-muted-foreground">{t("tagsUi.groupTags")}</span>
          {tags.length > 0 && (
            <ul aria-label={t("tagsUi.groupTagsAria")} className="flex flex-wrap gap-1">
              {tags.map((tag) => (
                <li
                  key={tag}
                  className="flex items-center gap-0.5 rounded bg-muted py-0.5 pr-0.5 pl-1.5 text-xs"
                >
                  {tag}
                  <button
                    type="button"
                    aria-label={t("tagsUi.removeTag", { name: tag })}
                    className="rounded p-0.5 hover:bg-background"
                    onClick={() => setTags(tags.filter((other) => other !== tag))}
                  >
                    <X aria-hidden className="size-3" />
                  </button>
                </li>
              ))}
            </ul>
          )}
          <TagInput
            label={t("tagsUi.addTagToGroup")}
            vocabulary={vocabulary}
            exclude={new Set(tags)}
            onPick={(tag) => setTags((current) => [...current, tag])}
          />
        </div>
      </div>
      <DialogFooter>
        <Button disabled={!name.trim()} onClick={save}>
          {group ? t("common.save") : t("tagsUi.create")}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
