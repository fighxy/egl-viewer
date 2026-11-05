import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Unlock } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { formatBytes } from "@/features/browse/format";
import { ARCHIVE_PASSWORD_MIN_LEN } from "@/features/export/password";
import { inTauri, ipc } from "@/ipc";
import { describeError } from "@/lib/errors";
import { useOpenings } from "@/lib/useOpenings";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

/**
 * Decrypts a passphrase `.zip.age` export to a ZIP on disk (D-102). Available wherever Backup /
 * Export is, so locked-folder archives can be opened without the age CLI.
 */
export function DecryptAgeDialog(props: Props) {
  return <DecryptAgeDialogContent key={useOpenings(props.open)} {...props} />;
}

function DecryptAgeDialogContent({ open, onOpenChange }: Props) {
  const { t } = useTranslation();
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [password, setPassword] = useState("");
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [written, setWritten] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const suggestDestination = (path: string) => {
    const cut = path.replace(/\.age$/i, "");
    return /\.zip$/i.test(cut) ? cut : `${cut}.zip`;
  };

  const pickSource = () => {
    openDialog({
      multiple: false,
      filters: [{ name: "age", extensions: ["age"] }],
      defaultPath: source || undefined,
    })
      .then((path) => {
        if (typeof path !== "string") return;
        setSource(path);
        setDestination((current) => current || suggestDestination(path));
      })
      .catch((failure: unknown) => setError(describeError(failure)));
  };

  const pickDestination = () => {
    openDialog({
      multiple: false,
      defaultPath: destination || suggestDestination(source) || undefined,
    })
      .then((path) => {
        if (typeof path === "string") setDestination(path);
      })
      .catch((failure: unknown) => setError(describeError(failure)));
  };

  const problems = (): string | null => {
    if (source.trim() === "") return t("exportUi.chooseAgeArchive");
    if (destination.trim() === "") return t("exportUi.chooseDecryptWhere");
    if (password === "") return t("exportUi.typePassword");
    if ([...password].length < ARCHIVE_PASSWORD_MIN_LEN) {
      return t("exportUi.passwordTooShort", { min: ARCHIVE_PASSWORD_MIN_LEN });
    }
    return null;
  };

  const start = () => {
    const problem = problems();
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    setWritten(null);
    setRunning(true);
    setProgress({ done: 0, total: 0 });
    ipc
      .decryptAgeArchive(source.trim(), destination.trim(), password, (next) => {
        setProgress(next);
      })
      .then((path) => {
        setPassword("");
        if (path) setWritten(path);
        else setError(t("exportUi.decryptCancelled"));
      })
      .catch((failure: unknown) => setError(describeError(failure)))
      .finally(() => setRunning(false));
  };

  const close = (next: boolean) => {
    if (running) return;
    onOpenChange(next);
  };

  const percent =
    progress && progress.total > 0 ? Math.min(100, (progress.done / progress.total) * 100) : 0;

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="grid max-h-[85vh] grid-cols-[minmax(0,1fr)] gap-4 overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("exportUi.decryptTitle")}</DialogTitle>
          <DialogDescription>{t("exportUi.decryptHint")}</DialogDescription>
        </DialogHeader>

        {written === null ? (
          <>
            <section className="grid gap-1.5">
              <Label htmlFor="decrypt-source">{t("exportUi.ageArchive")}</Label>
              <div className="flex gap-2">
                <Input
                  id="decrypt-source"
                  value={source}
                  onChange={(event) => setSource(event.target.value)}
                  disabled={running}
                />
                <Button variant="outline" size="sm" onClick={pickSource} disabled={running}>
                  <FolderOpen aria-hidden />
                  {t("common.browse")}
                </Button>
              </div>
            </section>
            <section className="grid gap-1.5">
              <Label htmlFor="decrypt-destination">{t("exportUi.decryptTo")}</Label>
              <div className="flex gap-2">
                <Input
                  id="decrypt-destination"
                  value={destination}
                  onChange={(event) => setDestination(event.target.value)}
                  disabled={running}
                />
                <Button variant="outline" size="sm" onClick={pickDestination} disabled={running}>
                  <FolderOpen aria-hidden />
                  {t("common.browse")}
                </Button>
              </div>
            </section>
            <section className="grid gap-1.5">
              <Label htmlFor="decrypt-password">{t("commonExtra.password")}</Label>
              <Input
                id="decrypt-password"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                disabled={running}
                autoComplete="off"
              />
            </section>
            {running && progress && (
              <div className="grid gap-1.5">
                <p className="text-[13px]">
                  {t("exportUi.decrypting")}
                  {progress.total > 0
                    ? ` · ${formatBytes(progress.done)} / ${formatBytes(progress.total)}`
                    : null}
                </p>
                <Progress value={percent} />
                <div className="flex justify-end">
                  <Button variant="outline" onClick={() => void ipc.cancelDecryptAge()}>
                    {t("common.cancel")}
                  </Button>
                </div>
              </div>
            )}
            {error && (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            )}
            {!running && (
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => close(false)}>
                  {t("common.cancel")}
                </Button>
                <Button
                  disabled={!inTauri() || problems() !== null}
                  title={problems() ?? undefined}
                  onClick={start}
                >
                  <Unlock aria-hidden />
                  {t("exportUi.decrypt")}
                </Button>
              </div>
            )}
          </>
        ) : (
          <div className="grid gap-2">
            <p className="text-sm">{t("exportUi.decryptDone")}</p>
            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() =>
                  ipc
                    .revealPath(written)
                    .catch((failure: unknown) => setError(describeError(failure)))
                }
              >
                <FolderOpen aria-hidden />
                {t("details.showExplorer")}
              </Button>
              <Button onClick={() => close(false)}>{t("commonExtra.close")}</Button>
            </div>
            {error && (
              <p role="alert" className="text-sm text-destructive">
                {error}
              </p>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
