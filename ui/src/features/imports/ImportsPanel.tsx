import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import type { ImportProgress } from "@/ipc";
import { importSummary, isRunning, percent } from "./importText";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  imports: ImportProgress[];
  onCancel: (id: number) => void;
};

/** Running and finished imports with their problems and skipped duplicates (docs/spec.md §7.4). */
export function ImportsPanel({ open, onOpenChange, imports, onCancel }: Props) {
  const { t, i18n } = useTranslation();
  const timeFormat = useMemo(
    () => new Intl.DateTimeFormat(i18n.language, { timeStyle: "short" }),
    [i18n.language],
  );
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("imports.title")}</DialogTitle>
          <DialogDescription>{t("imports.sessionHint")}</DialogDescription>
        </DialogHeader>
        {imports.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("imports.nothingYet")}</p>
        ) : (
          <ul className="grid max-h-[60vh] gap-3 overflow-y-auto">
            {imports.map((progress) => (
              <li
                key={progress.id}
                aria-label={t("imports.importLabel", { id: progress.id })}
                className="grid gap-2 rounded-lg border p-3"
              >
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-medium">
                    {progress.kind === "previews"
                      ? t("imports.kindPreviews")
                      : t("imports.kindAdding")}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {timeFormat.format(progress.startedAt)}
                  </span>
                  {isRunning(progress) && (
                    <Button
                      size="xs"
                      variant="outline"
                      className="ml-auto"
                      onClick={() => onCancel(progress.id)}
                    >
                      {t("imports.cancel")}
                    </Button>
                  )}
                </div>
                <Progress value={percent(progress)} aria-label={t("imports.progress")} />
                <p className="text-[13px]">{importSummary(progress)}</p>
                {progress.failures.length > 0 && (
                  <details className="text-[13px]">
                    <summary className="cursor-default text-destructive">
                      {t("imports.problems", { count: progress.failed })}
                    </summary>
                    <ul className="mt-1 grid gap-1">
                      {progress.failures.map((failure) => (
                        <li key={`${failure.path}:${failure.reason}`} className="select-text">
                          <span className="font-medium break-all">{failure.path}</span>
                          <span className="text-muted-foreground"> — {failure.reason}</span>
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
                {progress.skipped.length > 0 && (
                  <details className="text-[13px]">
                    <summary className="cursor-default text-muted-foreground">
                      {t("imports.skipped", { count: progress.duplicates })}
                    </summary>
                    <ul className="mt-1 grid gap-1">
                      {progress.skipped.map((duplicate) => (
                        <li key={duplicate.path} className="break-all select-text">
                          {duplicate.path}
                          {duplicate.inTrash && (
                            <span className="text-muted-foreground"> {t("imports.inTrash")}</span>
                          )}
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
              </li>
            ))}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  );
}
