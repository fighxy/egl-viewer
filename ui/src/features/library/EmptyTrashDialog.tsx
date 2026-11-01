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
import { Progress } from "@/components/ui/progress";
import type { LockProgress } from "@/ipc";

/** Progress while Empty Trash runs (D-074). Same `{ done, total }` shape as lock progress. */
export type EmptyTrashProgress = LockProgress;

/** Modal progress for emptying the trash; Cancel stops between items. */
export function EmptyTrashDialog({
  progress,
  onCancel,
}: {
  progress: EmptyTrashProgress | null;
  onCancel: () => void;
}) {
  const { t, i18n } = useTranslation();
  const total = progress?.total ?? 0;
  const done = progress?.done ?? 0;
  return (
    <Dialog open onOpenChange={() => {}}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{t("confirm.emptyingTitle")}</DialogTitle>
          <DialogDescription>{t("confirm.emptyingBody")}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-2" role="status" aria-label={t("confirm.emptyingTitle")}>
          <Progress value={total > 0 ? (done / total) * 100 : 0} />
          <p className="text-xs text-muted-foreground">
            {total > 0
              ? t("confirm.emptyingCounted", {
                  done: done.toLocaleString(i18n.language),
                  total: total.toLocaleString(i18n.language),
                })
              : t("confirm.emptyingPlain")}
          </p>
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={onCancel}>
            {t("common.cancel")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
