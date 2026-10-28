import { ArrowDown, ArrowUp } from "lucide-react";
import { SORT_KEYS } from "@/commands/registry";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { SortKey } from "@/ipc";

type Props = {
  sort: SortKey;
  ascending: boolean;
  onSort: (key: SortKey) => void;
  onAscending: (ascending: boolean) => void;
};

/** The toolbar's sort: the same keys as View → Sort By (docs/spec.md §5). */
export function SortButton({ sort, ascending, onSort, onAscending }: Props) {
  const label = SORT_KEYS.find((entry) => entry.key === sort)?.label ?? "Date Added";
  const Arrow = ascending ? ArrowUp : ArrowDown;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" aria-label={`Sort by ${label}`} title="Sort">
          {label}
          <Arrow aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup value={sort} onValueChange={(key) => onSort(key as SortKey)}>
          {SORT_KEYS.map((entry) => (
            <DropdownMenuRadioItem key={entry.key} value={entry.key}>
              {entry.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
        <DropdownMenuSeparator />
        <DropdownMenuRadioGroup
          value={ascending ? "ascending" : "descending"}
          onValueChange={(value) => onAscending(value === "ascending")}
        >
          <DropdownMenuRadioItem value="ascending">Ascending</DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="descending">Descending</DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
