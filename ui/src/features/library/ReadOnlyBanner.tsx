import { Lock } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { LibraryInfo } from "@/ipc";

type Props = {
  library: LibraryInfo;
  onOpenForEditing: () => void;
};

const timeFormat = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" });

/** Why the library is read-only (docs/spec.md §7.3, docs/library-format.md §5). */
export function ReadOnlyBanner({ library, onOpenForEditing }: Props) {
  if (!library.readOnly) return null;
  const holder = library.holder;
  let text: string;
  switch (library.readOnlyReason) {
    case "otherInstance":
      text = holder
        ? `This library is open in another egl-viewer window on ${holder.computer} since ${timeFormat.format(holder.since)}. Changes are off here.`
        : "This library is open in another egl-viewer window. Changes are off here.";
      break;
    case "syncFolder":
      text =
        "This library is inside a cloud-synced folder such as OneDrive, so it opens read-only: syncing a library while it is open damages it. Close egl-viewer, move the folder out of the synced folder, then open it again.";
      break;
    default:
      text = "This library is on a read-only drive or folder, so changes are off.";
  }
  return (
    <div
      role="status"
      aria-label="Read-only library"
      className="flex items-center gap-3 border-b bg-muted/60 px-3 py-2 text-[13px]"
    >
      <Lock aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      <p className="flex-1">{text}</p>
      {library.readOnlyReason === "otherInstance" && (
        <Button size="sm" variant="outline" onClick={onOpenForEditing}>
          Open for editing
        </Button>
      )}
    </div>
  );
}
