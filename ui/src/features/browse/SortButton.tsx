import { ArrowDown, ArrowUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { MANUAL, SORT_KEYS } from "@/commands/registry";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { translateSortLabel } from "@/i18n/labels";
import type { SortKey } from "@/ipc";

type Props = {
  sort: SortKey;
  ascending: boolean;
  /** Offer Manual: a folder is open. */
  manual?: boolean;
  onSort: (key: SortKey) => void;
  onAscending: (ascending: boolean) => void;
};

/** The toolbar's sort: the same keys as View → Sort By (docs/spec.md §5). */
export function SortButton({ sort, ascending, manual = false, onSort, onAscending }: Props) {
  const { t } = useTranslation();
  const keys = manual ? [...SORT_KEYS, MANUAL] : SORT_KEYS;
  const activeKey = keys.some((entry) => entry.key === sort) ? sort : "added";
  const label = translateSortLabel(t, activeKey);
  const Arrow = ascending ? ArrowUp : ArrowDown;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          aria-label={t("sort.byAria", { label })}
          title={t("sort.title")}
        >
          {label}
          <Arrow aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup value={sort} onValueChange={(key) => onSort(key as SortKey)}>
          {keys.map((entry) => (
            <DropdownMenuRadioItem key={entry.key} value={entry.key}>
              {translateSortLabel(t, entry.key)}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <DropdownMenuRadioGroup
          value={ascending ? "ascending" : "descending"}
          onValueChange={(value) => onAscending(value === "ascending")}
        >
          <DropdownMenuRadioItem value="ascending">{t("sort.ascending")}</DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="descending">{t("sort.descending")}</DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
