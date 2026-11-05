import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { formatBytes } from "@/features/browse/format";
import { type ByteProgress, type DecryptedArchive, ipc } from "@/ipc";
import { describeError } from "@/lib/errors";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

/** The folder of a path, for saving the result next to the archive. */
function folderOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : path;
}

function nameOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut >= 0 ? path.slice(cut + 1) : path;
}

/**
 * File → Decrypt Archive… (D-110): opens an encrypted export (`name.zip.age`) with its
 * password into an ordinary ZIP, saved next to the archive unless another folder is chosen.
 * The password is sent to Rust like a lock password and never stored.
 */
export function DecryptArchiveDialog({ open, onOpenChange }: Props) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="grid grid-cols-[minmax(0,1fr)] gap-4 sm:max-w-lg">
        {/* A new form on every opening: no password survives a close. */}
        {open && <DecryptForm onClose={() => onOpenChange(false)} />}
        {!open && (
          <DialogHeader>
            <DialogTitle>{t("decryptUi.title")}</DialogTitle>
          </DialogHeader>
        )}
      </DialogContent>
    </Dialog>
  );
}

function DecryptForm({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<DecryptedArchive | null>(null);
  const [progress, setProgress] = useState<ByteProgress | null>(null);
  const [cancelled, setCancelled] = useState(false);

  const pickArchive = () => {
    openDialog({
      multiple: false,
      directory: false,
      title: t("decryptUi.pickTitle"),
      filters: [{ name: t("decryptUi.archives"), extensions: ["age"] }],
    })
      .then((path) => {
        if (typeof path !== "string") return;
        setSource(path);
        setDestination((current) => current || folderOf(path));
        setResult(null);
      })
      .catch((failure: unknown) => setError(describeError(failure)));
  };

  const pickFolder = () => {
    openDialog({ directory: true, multiple: false, defaultPath: destination || undefined })
      .then((path) => {
        if (typeof path === "string") setDestination(path);
      })
      .catch((failure: unknown) => setError(describeError(failure)));
  };

  const problem = (): string | null => {
    if (source.trim() === "") return t("decryptUi.chooseArchive");
    if (destination.trim() === "") return t("decryptUi.chooseFolder");
    if (password === "") return t("exportUi.typePassword");
    return null;
  };

  const decrypt = async () => {
    const reason = problem();
    if (reason) {
      setError(reason);
      return;
    }
    setBusy(true);
    setError(undefined);
    setCancelled(false);
    setProgress(null);
    try {
      const decrypted = await ipc.decryptArchive(source, destination, password, setProgress);
      setResult(decrypted);
      setCancelled(decrypted === null);
      setPassword("");
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  return (
    <>
      <DialogHeader>
        <DialogTitle>{t("decryptUi.title")}</DialogTitle>
        <DialogDescription>{t("decryptUi.description")}</DialogDescription>
      </DialogHeader>
      <div className="grid gap-3 text-[13px]">
        <div className="grid gap-1">
          <Label htmlFor="decrypt-source">{t("decryptUi.archive")}</Label>
          <div className="flex gap-2">
            <Input
              id="decrypt-source"
              value={source}
              onChange={(event) => setSource(event.target.value)}
              placeholder="Library.zip.age"
              spellCheck={false}
            />
            <Button type="button" variant="outline" onClick={pickArchive} disabled={busy}>
              {t("decryptUi.choose")}
            </Button>
          </div>
        </div>
        <div className="grid gap-1">
          <Label htmlFor="decrypt-destination">{t("decryptUi.saveIn")}</Label>
          <div className="flex gap-2">
            <Input
              id="decrypt-destination"
              value={destination}
              onChange={(event) => setDestination(event.target.value)}
              spellCheck={false}
            />
            <Button type="button" variant="outline" onClick={pickFolder} disabled={busy}>
              {t("decryptUi.choose")}
            </Button>
          </div>
        </div>
        <div className="grid gap-1">
          <Label htmlFor="decrypt-password">{t("commonExtra.password")}</Label>
          <Input
            id="decrypt-password"
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !busy) void decrypt();
            }}
          />
        </div>
        {busy && (
          <div className="grid gap-1">
            <p className="text-muted-foreground">{t("decryptUi.decrypting")}</p>
            <Progress
              aria-label={t("decryptUi.decrypting")}
              value={progress && progress.total > 0 ? (progress.done / progress.total) * 100 : 0}
            />
          </div>
        )}
        {cancelled && <p role="status">{t("decryptUi.cancelled")}</p>}
        {result && (
          <p role="status">
            {t("decryptUi.done", { name: nameOf(result.path), bytes: formatBytes(result.bytes) })}
          </p>
        )}
        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
      </div>
      <DialogFooter>
        {result && (
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              void ipc.revealPath(result.path).catch((failure: unknown) => {
                setError(describeError(failure));
              })
            }
          >
            {t("details.showExplorer")}
          </Button>
        )}
        {busy ? (
          <Button
            type="button"
            variant="outline"
            onClick={() => void ipc.cancelDecrypt().catch(() => {})}
          >
            {t("common.cancel")}
          </Button>
        ) : (
          <Button type="button" variant="outline" onClick={onClose}>
            {t("commonExtra.close")}
          </Button>
        )}
        <Button type="button" onClick={() => void decrypt()} disabled={busy}>
          {t("decryptUi.decrypt")}
        </Button>
      </DialogFooter>
    </>
  );
}
