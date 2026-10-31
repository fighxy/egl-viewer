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
  if (!state) return null;

  const title =
    state.kind === "checking"
      ? "Checking for updates"
      : state.kind === "upToDate"
        ? "You're up to date"
        : state.kind === "error"
          ? "Update check failed"
          : state.kind === "downloading"
            ? "Downloading update"
            : "Update available";

  const description =
    state.kind === "checking"
      ? "Looking for a newer release…"
      : state.kind === "upToDate"
        ? `egl-viewer ${state.version} is the latest version.`
        : state.kind === "error"
          ? state.message
          : state.kind === "downloading"
            ? progressLabel(state.progress)
            : [
                `Version ${state.update.version} is available (you have ${state.update.currentVersion}).`,
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
                Later
              </Button>
              <Button onClick={onInstall}>Install and Restart</Button>
            </>
          ) : (
            <Button variant="outline" disabled={busy} autoFocus onClick={onClose}>
              {busy ? "Please wait…" : "OK"}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
