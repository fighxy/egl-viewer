import { Plus, X } from "lucide-react";
import { type ReactNode, useState } from "react";
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
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { FolderPicker } from "@/features/folders/FolderPicker";
import { findFolder } from "@/features/folders/tree";
import { SHAPES } from "@/features/library/FilterBar";
import { TagInput } from "@/features/tags/TagInput";
import type { ExtensionCount, Folder, JsonValue, SmartFolder, SmartFolderChange, Tag } from "@/ipc";
import {
  dayStart,
  dayText,
  editable,
  kindOf,
  METHODS,
  needsValue,
  newCondition,
  newRule,
  PROPERTIES,
  readConditions,
  type SmartCondition,
  type SmartRule,
  withMethod,
} from "./rules";
import { smartPath } from "./SmartFolders";

/** Which smart folder dialog is open. `edit` with `id` null creates one, perhaps from a draft. */
export type SmartDialog =
  | {
      kind: "edit";
      id: string | null;
      parent: string | null;
      name?: string;
      conditions?: JsonValue;
    }
  | { kind: "delete"; id: string };

type Props = {
  dialog: SmartDialog | null;
  smartFolders: readonly SmartFolder[];
  vocabulary: readonly Tag[];
  folders: readonly Folder[];
  extensions: readonly ExtensionCount[];
  onClose: () => void;
  onSave: (change: SmartFolderChange) => void;
  onDelete: (id: string) => void;
};

const SELECT =
  "h-7 rounded-md border bg-background px-1.5 text-[13px] outline-none focus:ring-2 focus:ring-ring/50";
const FIELD =
  "h-7 rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50";

/** The smart folder editor and the delete confirmation (docs/spec.md §3, §9 phase 6). */
export function SmartFolderDialogs({
  dialog,
  smartFolders,
  vocabulary,
  folders,
  extensions,
  onClose,
  onSave,
  onDelete,
}: Props) {
  const { t } = useTranslation();
  const close = (open: boolean) => {
    if (!open) onClose();
  };
  const target =
    dialog && dialog.id !== null ? smartPath(smartFolders, dialog.id).at(-1) : undefined;
  return (
    <>
      <Dialog open={dialog?.kind === "edit"} onOpenChange={close}>
        {dialog?.kind === "edit" && (
          <SmartEditor
            key={dialog.id ?? "new"}
            folder={target}
            parent={
              dialog.parent
                ? smartPath(smartFolders, dialog.parent).at(-1)
                : target && smartPath(smartFolders, target.id).at(-2)
            }
            draftName={dialog.name}
            draftConditions={dialog.conditions}
            vocabulary={vocabulary}
            folders={folders}
            extensions={extensions}
            onSave={(name, conditions) =>
              onSave(
                target
                  ? { type: "update", id: target.id, name, conditions }
                  : { type: "create", name, parent: dialog.parent, conditions },
              )
            }
          />
        )}
      </Dialog>
      <Dialog open={dialog?.kind === "delete"} onOpenChange={close}>
        {dialog?.kind === "delete" && (
          <DialogContent showCloseButton={false}>
            <DialogHeader>
              <DialogTitle>{t("smart.deleteTitle", { name: target?.name ?? "" })}</DialogTitle>
              <DialogDescription>
                {target && target.children.length > 0
                  ? t("smart.deleteBodyWithChildren")
                  : t("smart.deleteBody")}
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant="outline" autoFocus onClick={onClose}>
                {t("smart.cancel")}
              </Button>
              <Button variant="destructive" onClick={() => onDelete(dialog.id)}>
                {t("smart.deleteAction")}
              </Button>
            </DialogFooter>
          </DialogContent>
        )}
      </Dialog>
    </>
  );
}

function SmartEditor({
  folder,
  parent,
  draftName,
  draftConditions,
  vocabulary,
  folders,
  extensions,
  onSave,
}: {
  folder?: SmartFolder;
  parent?: SmartFolder;
  draftName?: string;
  draftConditions?: JsonValue;
  vocabulary: readonly Tag[];
  folders: readonly Folder[];
  extensions: readonly ExtensionCount[];
  onSave: (name: string, conditions: JsonValue) => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(folder?.name ?? draftName ?? "");
  const [conditions, setConditions] = useState<SmartCondition[]>(() => {
    const read = readConditions(folder?.conditions ?? draftConditions);
    return read.length > 0 ? read : [newCondition()];
  });
  const complete =
    name.trim() !== "" && conditions.every((condition) => condition.rules.length > 0);
  const setCondition = (at: number, next: SmartCondition | null) =>
    setConditions((current) =>
      next === null
        ? current.filter((_, index) => index !== at)
        : current.map((condition, index) => (index === at ? next : condition)),
    );
  return (
    <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle>
          {folder ? t("smart.editTitle", { name: folder.name }) : t("smart.newTitle")}
        </DialogTitle>
        <DialogDescription>
          {parent
            ? t("smart.insideParent", { name: parent.name })
            : t("smart.topDescription")}
        </DialogDescription>
      </DialogHeader>
      <div className="grid gap-3">
        <input
          aria-label={t("smart.nameAria")}
          placeholder={t("smart.name")}
          // biome-ignore lint/a11y/noAutofocus: the first field of the dialog.
          autoFocus
          value={name}
          onChange={(event) => setName(event.target.value)}
          className="h-8 rounded-md border bg-background px-2 text-[13px] outline-none focus:ring-2 focus:ring-ring/50"
        />
        {conditions.map((condition, at) => (
          <ConditionBlock
            // biome-ignore lint/suspicious/noArrayIndexKey: conditions have no id; order is their identity.
            key={at}
            index={at}
            condition={condition}
            removable={conditions.length > 1}
            vocabulary={vocabulary}
            folders={folders}
            extensions={extensions}
            onChange={(next) => setCondition(at, next)}
          />
        ))}
        <Button
          variant="outline"
          size="sm"
          className="justify-self-start"
          onClick={() => setConditions((current) => [...current, newCondition()])}
        >
          <Plus aria-hidden />
          {t("smart.addCondition")}
        </Button>
      </div>
      <DialogFooter>
        <Button
          disabled={!complete}
          onClick={() => onSave(name.trim(), conditions as unknown as JsonValue)}
        >
          {folder ? t("smart.save") : t("smart.create")}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}

function ConditionBlock({
  index,
  condition,
  removable,
  vocabulary,
  folders,
  extensions,
  onChange,
}: {
  index: number;
  condition: SmartCondition;
  removable: boolean;
  vocabulary: readonly Tag[];
  folders: readonly Folder[];
  extensions: readonly ExtensionCount[];
  onChange: (next: SmartCondition | null) => void;
}) {
  const { t } = useTranslation();
  const setRule = (at: number, next: SmartRule | null) =>
    onChange({
      ...condition,
      rules:
        next === null
          ? condition.rules.filter((_, position) => position !== at)
          : condition.rules.map((rule, position) => (position === at ? next : rule)),
    });
  return (
    <fieldset className="grid gap-2 rounded-md border p-2">
      <legend className="sr-only">{t("smart.conditionN", { n: index + 1 })}</legend>
      <div className="flex flex-wrap items-center gap-2 text-[13px]">
        <span>{t("smart.itemsMatch")}</span>
        <select
          aria-label={t("smart.match")}
          className={SELECT}
          value={condition.match === "OR" ? "OR" : "AND"}
          onChange={(event) => onChange({ ...condition, match: event.target.value })}
        >
          <option value="AND">{t("smart.and")}</option>
          <option value="OR">{t("smart.or")}</option>
        </select>
        <span>{t("smart.ofTheseRules")}</span>
        <label className="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground">
          <input
            type="checkbox"
            checked={condition.boolean === "FALSE"}
            onChange={(event) =>
              onChange({ ...condition, boolean: event.target.checked ? "FALSE" : "TRUE" })
            }
          />
          {t("smart.leaveOut")}
        </label>
        {removable && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={t("smart.removeCondition", { n: index + 1 })}
            onClick={() => onChange(null)}
          >
            <X aria-hidden />
          </Button>
        )}
      </div>
      {condition.rules.map((rule, at) => (
        <RuleRow
          // biome-ignore lint/suspicious/noArrayIndexKey: rules have no id; order is their identity.
          key={at}
          index={at}
          rule={rule}
          vocabulary={vocabulary}
          folders={folders}
          extensions={extensions}
          onChange={(next) => setRule(at, next)}
        />
      ))}
      {condition.rules.length === 0 && (
        <p className="text-xs text-muted-foreground">{t("smart.addAtLeastOne")}</p>
      )}
      <Button
        variant="ghost"
        size="xs"
        className="justify-self-start"
        onClick={() => onChange({ ...condition, rules: [...condition.rules, newRule("tags")] })}
      >
        <Plus aria-hidden />
        {t("smart.addRule")}
      </Button>
    </fieldset>
  );
}

function RuleRow({
  index,
  rule,
  vocabulary,
  folders,
  extensions,
  onChange,
}: {
  index: number;
  rule: SmartRule;
  vocabulary: readonly Tag[];
  folders: readonly Folder[];
  extensions: readonly ExtensionCount[];
  onChange: (next: SmartRule | null) => void;
}) {
  const { t } = useTranslation();
  const remove = (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-label={t("smart.removeRule", { n: index + 1 })}
      onClick={() => onChange(null)}
    >
      <X aria-hidden />
    </Button>
  );
  const kind = kindOf(rule.property);
  if (!editable(rule) || kind === undefined) {
    return (
      <fieldset className="flex items-center gap-2">
        <legend className="sr-only">{t("smart.ruleN", { n: index + 1 })}</legend>
        <span className="flex-1 text-[13px] text-muted-foreground">
          {t("smart.unsupportedRule", { property: rule.property, method: rule.method })}
        </span>
        {remove}
      </fieldset>
    );
  }
  return (
    <fieldset className="flex flex-wrap items-start gap-1.5">
      <legend className="sr-only">{t("smart.ruleN", { n: index + 1 })}</legend>
      <select
        aria-label={t("smart.property")}
        className={SELECT}
        value={rule.property}
        onChange={(event) => onChange(newRule(event.target.value))}
      >
        {PROPERTIES.map((entry) => (
          <option key={entry.property} value={entry.property}>
            {entry.label}
          </option>
        ))}
      </select>
      <select
        aria-label={t("smart.method")}
        className={SELECT}
        value={rule.method}
        onChange={(event) => onChange(withMethod(rule, event.target.value))}
      >
        {METHODS[kind].map((entry) => (
          <option key={entry.method} value={entry.method}>
            {entry.label}
          </option>
        ))}
      </select>
      {needsValue(rule.method) && (
        <RuleValue
          rule={rule}
          vocabulary={vocabulary}
          folders={folders}
          extensions={extensions}
          onChange={onChange}
        />
      )}
      <span className="ml-auto">{remove}</span>
    </fieldset>
  );
}

function strings(value: JsonValue | undefined): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === "string")
    : [];
}

function numbers(value: JsonValue | undefined): number[] {
  return Array.isArray(value)
    ? value.flatMap((entry) => {
        const number = typeof entry === "number" ? entry : Number(entry);
        return Number.isFinite(number) ? [number] : [];
      })
    : [];
}

function text(value: JsonValue | undefined): string {
  return typeof value === "string" || typeof value === "number" ? String(value) : "";
}

function Chip({ label, onRemove }: { label: string; onRemove: () => void }) {
  const { t } = useTranslation();
  return (
    <span className="flex items-center gap-0.5 rounded bg-muted py-0.5 pr-0.5 pl-1.5 text-xs">
      {label}
      <button
        type="button"
        aria-label={t("smart.removeChip", { label })}
        className="rounded p-0.5 hover:bg-background"
        onClick={onRemove}
      >
        <X aria-hidden className="size-3" />
      </button>
    </span>
  );
}

function RuleValue({
  rule,
  vocabulary,
  folders,
  extensions,
  onChange,
}: {
  rule: SmartRule;
  vocabulary: readonly Tag[];
  folders: readonly Folder[];
  extensions: readonly ExtensionCount[];
  onChange: (next: SmartRule) => void;
}) {
  const { t } = useTranslation();
  const set = (value: JsonValue) => onChange({ ...rule, value });
  const kind = kindOf(rule.property);
  let editor: ReactNode = null;
  if (kind === "set" && rule.property === "tags") {
    const values = strings(rule.value);
    editor = (
      <div className="grid min-w-48 flex-1 gap-1">
        {values.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {values.map((tag) => (
              <Chip key={tag} label={tag} onRemove={() => set(values.filter((v) => v !== tag))} />
            ))}
          </div>
        )}
        <TagInput
          label={t("smart.ruleTags")}
          vocabulary={vocabulary}
          exclude={new Set(values)}
          onPick={(tag) => set([...values, tag])}
        />
      </div>
    );
  } else if (kind === "set") {
    const values = strings(rule.value);
    const checked = new Set(values);
    editor = (
      <div className="flex min-w-48 flex-1 flex-wrap items-center gap-1">
        {values.map((id) => (
          <Chip
            key={id}
            label={findFolder([...folders], id)?.name ?? t("smart.unknownFolder")}
            onRemove={() => set(values.filter((value) => value !== id))}
          />
        ))}
        <Popover>
          <PopoverTrigger asChild>
            <Button variant="outline" size="xs">
              <Plus aria-hidden />
              {t("smart.pickFolders")}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="p-2">
            <FolderPicker
              label={t("smart.ruleFolders")}
              folders={[...folders]}
              checked={checked}
              onPick={(id) =>
                set(checked.has(id) ? values.filter((value) => value !== id) : [...values, id])
              }
            />
          </PopoverContent>
        </Popover>
      </div>
    );
  } else if (kind === "text") {
    editor = (
      <input
        aria-label={t("smart.value")}
        className={`${FIELD} min-w-40 flex-1`}
        value={text(rule.value)}
        onChange={(event) => set(event.target.value)}
      />
    );
  } else if (kind === "type") {
    const current = text(rule.value);
    const known = ["image", ...extensions.map((entry) => entry.ext)];
    editor = (
      <select
        aria-label={t("smart.value")}
        className={SELECT}
        value={current}
        onChange={(event) => set(event.target.value)}
      >
        {!known.includes(current) && <option value={current}>{current.toUpperCase()}</option>}
        <option value="image">{t("smart.images")}</option>
        {extensions.map((entry) => (
          <option key={entry.ext} value={entry.ext}>
            {entry.ext.toUpperCase()}
          </option>
        ))}
      </select>
    );
  } else if (kind === "rating") {
    editor = (
      <select
        aria-label={t("smart.value")}
        className={SELECT}
        value={text(rule.value)}
        onChange={(event) => set(event.target.value)}
      >
        <option value="none">{t("smart.unrated")}</option>
        {[1, 2, 3, 4, 5].map((star) => (
          <option key={star} value={String(star)}>
            {"★".repeat(star)}
          </option>
        ))}
      </select>
    );
  } else if (kind === "shape") {
    editor = (
      <select
        aria-label={t("smart.value")}
        className={SELECT}
        value={text(rule.value)}
        onChange={(event) => set(event.target.value)}
      >
        {SHAPES.map((entry) => (
          <option key={entry.value} value={entry.value}>
            {entry.label}
          </option>
        ))}
      </select>
    );
  } else if (kind === "date" && rule.method === "within") {
    editor = (
      <span className="flex items-center gap-1 text-[13px]">
        <input
          aria-label={t("smart.daysAria")}
          type="number"
          min={1}
          className={`${FIELD} w-20`}
          value={numbers(rule.value)[0] ?? 7}
          onChange={(event) => set([Math.max(1, Number(event.target.value) || 1)])}
        />
        {t("smart.days")}
      </span>
    );
  } else if (kind === "date") {
    const values = numbers(rule.value);
    const day = (at: number, label: string) => (
      <input
        aria-label={label}
        type="date"
        className={FIELD}
        value={values[at] === undefined ? "" : dayText(values[at])}
        onChange={(event) => {
          const start = dayStart(event.target.value);
          if (start === null) return;
          const next = [...values];
          next[at] = start;
          set(next);
        }}
      />
    );
    editor =
      rule.method === "between" ? (
        <span className="flex items-center gap-1 text-[13px]">
          {day(0, t("smart.from"))} {t("smart.andJoin")} {day(1, t("smart.to"))}
        </span>
      ) : (
        day(0, t("smart.date"))
      );
  } else if (kind === "number") {
    const values = numbers(rule.value);
    const field = (at: number, label: string) => (
      <input
        aria-label={label}
        type="number"
        min={0}
        className={`${FIELD} w-24`}
        value={values[at] ?? 0}
        onChange={(event) => {
          const next = [...values];
          next[at] = Number(event.target.value) || 0;
          set(next);
        }}
      />
    );
    editor = (
      <span className="flex items-center gap-1 text-[13px]">
        {rule.method === "between" ? (
          <>
            {field(0, t("smart.from"))} {t("smart.andJoin")} {field(1, t("smart.to"))}
          </>
        ) : (
          field(0, t("smart.value"))
        )}
        {rule.property === "fileSize" ? (
          <select
            aria-label={t("smart.unit")}
            className={SELECT}
            value={rule.unit === "kb" ? "kb" : "mb"}
            onChange={(event) => onChange({ ...rule, unit: event.target.value })}
          >
            <option value="kb">KB</option>
            <option value="mb">MB</option>
          </select>
        ) : (
          "px"
        )}
      </span>
    );
  }
  return editor;
}
