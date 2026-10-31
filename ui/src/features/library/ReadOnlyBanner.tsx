import { Lock } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import type { LibraryInfo } from "@/ipc";

type Props = {
  library: LibraryInfo;
  onOpenForEditing: () => void;
};

/** Why the library is read-only (docs/spec.md §7.3, docs/library-format.md §5). */
export function ReadOnlyBanner({ library, onOpenForEditing }: Props) {
  const { t, i18n } = useTranslation();
  if (!library.readOnly) return null;
  const holder = library.holder;
  const timeFormat = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: "medium",
    timeStyle: "short",
  });
  let text: string;
  switch (library.readOnlyReason) {
    case "otherInstance":
      text = holder
        ? t("readOnly.otherInstanceNamed", {
            computer: holder.computer,
            since: timeFormat.format(holder.since),
          })
        : t("readOnly.otherInstance");
      break;
    case "syncFolder":
      text = t("readOnly.syncFolder");
      break;
    default:
      text = t("readOnly.medium");
  }
  return (
    <div
      role="status"
      aria-label={t("readOnly.banner")}
      className="flex items-center gap-3 border-b bg-muted/60 px-3 py-2 text-[13px]"
    >
      <Lock aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      <p className="flex-1">{text}</p>
      {library.readOnlyReason === "otherInstance" && (
        <Button size="sm" variant="outline" onClick={onOpenForEditing}>
          {t("readOnly.openForEditing")}
        </Button>
      )}
    </div>
  );
}
