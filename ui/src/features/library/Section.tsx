import { ChevronDown, ChevronRight } from "lucide-react";
import { type ReactNode, useCallback, useState } from "react";

const KEY = "egl-viewer.collapsed:";

/** A left bar section that stays collapsed once collapsed (docs/spec.md §7.3). */
export function useSectionOpen(name: string): [boolean, (open: boolean) => void] {
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

export function Section({
  label,
  storageKey,
  action,
  children,
}: {
  label: string;
  /** Stable id for collapse state so a translated label does not reset it (D-069). */
  storageKey?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useSectionOpen(storageKey ?? label);
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
