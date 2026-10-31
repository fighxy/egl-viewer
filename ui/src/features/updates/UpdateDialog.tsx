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
import type { AvailableUpdate } from "./updater";
import { progressLabel, type UpdateProgress } from "./updater";

export type UpdateDialogState =
  | { kind: "checking" }
  | { kind: "upToDate"; version: string }
  | { kind: "available"; update: AvailableUpdate }
  | { kind: "downloading"; update: AvailableUpdate; progress: UpdateProgress }
  | { kind: "error"; message: string };

type Props = {
  state: UpdateDialogState | null;
  onClose: () => void;
  onInstall: () => void;
};

/** Check for Updates… and the optional launch prompt (docs/decisions.md D-062). */
export function UpdateDialog({ state, onClose, onInstall }: Props) {
  const { t } = useTranslation();
  if (!state) return null;

  const title =
    state.kind === "checking"
      ? t("updates.checking")
      : state.kind === "upToDate"
        ? t("updates.upToDate")
        : state.kind === "error"
          ? t("updates.failed")
          : state.kind === "downloading"
            ? t("updates.downloading")
            : t("updates.available");

  const description =
    state.kind === "checking"
      ? t("updates.looking")
      : state.kind === "upToDate"
        ? t("updates.latest", { version: state.version })
        : state.kind === "error"
          ? state.message
          : state.kind === "downloading"
            ? progressLabel(state.progress)
            : [
                t("updates.versionAvailable", {
                  version: state.update.version,
                  current: state.update.currentVersion,
                }),
                state.update.body?.trim() || null,
              ]
                .filter(Boolean)
                .join("\n\n");

  const busy = state.kind === "checking" || state.kind === "downloading";
  const canInstall = state.kind === "available";

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <DialogContent showCloseButton={!busy}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription className="whitespace-pre-wrap">{description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          {canInstall ? (
            <>
              <Button variant="outline" onClick={onClose}>
                {t("updates.later")}
              </Button>
              <Button onClick={onInstall}>{t("updates.installRestart")}</Button>
            </>
          ) : (
            <Button variant="outline" disabled={busy} autoFocus onClick={onClose}>
              {busy ? t("updates.pleaseWait") : t("updates.ok")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
