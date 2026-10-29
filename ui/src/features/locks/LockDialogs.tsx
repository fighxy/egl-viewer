import { save } from "@tauri-apps/plugin-dialog";
import { useId, useState } from "react";
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
import { Progress } from "@/components/ui/progress";
import { ColourPicker } from "@/features/folders/ColourPicker";
import { findFolder } from "@/features/folders/tree";
import { plural } from "@/features/imports/importText";
import { type Folder, ipc, type LockProgress, type RecoveryKey, type SourceError } from "@/ipc";
import { describeError } from "@/lib/errors";

/** Which lock dialog is open, for which folder. */
export type LockDialog =
  | { kind: "lock"; id: string }
  | { kind: "unlock"; id: string }
  | { kind: "change"; id: string }
  | { kind: "renew"; id: string }
  | { kind: "remove"; id: string };

type Props = {
  dialog: LockDialog | null;
  folders: Folder[];
  onClose: () => void;
  /** A lock changed. `opened` is a folder that is now unlocked and should open. */
  onChanged: (opened?: string) => void;
};

/**
 * Locking a folder, unlocking it (with the password or the recovery key), changing the
 * password, a new recovery key, and removing the lock (docs/age-locked-folders.md, spec §3).
 */
export function LockDialogs({ dialog, folders, onClose, onChanged }: Props) {
  const folder = dialog ? findFolder(folders, dialog.id) : undefined;
  const name = folder?.name ?? "";
  const done = (opened?: string) => {
    onChanged(opened);
    onClose();
  };
  return (
    <>
      {dialog?.kind === "lock" && folder && (
        <LockFolder
          key={dialog.id}
          folder={folder}
          onCancel={onClose}
          onLocked={() => onChanged()}
          onDone={onClose}
        />
      )}
      {dialog?.kind === "unlock" && (
        <UnlockFolder key={dialog.id} id={dialog.id} name={name} onCancel={onClose} onDone={done} />
      )}
      {dialog?.kind === "change" && (
        <ChangePassword
          key={dialog.id}
          id={dialog.id}
          name={name}
          onCancel={onClose}
          onDone={onClose}
        />
      )}
      {dialog?.kind === "renew" && (
        <RenewRecovery
          key={dialog.id}
          id={dialog.id}
          name={name}
          onCancel={onClose}
          onDone={onClose}
        />
      )}
      {dialog?.kind === "remove" && folder && (
        <RemoveLock key={dialog.id} folder={folder} onCancel={onClose} onDone={() => done()} />
      )}
    </>
  );
}

function isWrongPassword(error: unknown): boolean {
  return (error as SourceError | null)?.kind === "wrongPassword";
}

/** A labelled password field. */
function PasswordField({
  label,
  value,
  onChange,
  autoFocus = false,
  invalid = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoFocus?: boolean;
  invalid?: boolean;
}) {
  const id = useId();
  return (
    <div className="grid gap-1">
      <label htmlFor={id} className="text-xs text-muted-foreground">
        {label}
      </label>
      <Input
        id={id}
        type="password"
        autoComplete="off"
        autoFocus={autoFocus}
        aria-invalid={invalid || undefined}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function ErrorText({ children }: { children: React.ReactNode }) {
  return (
    <p role="alert" className="text-[13px] text-destructive">
      {children}
    </p>
  );
}

function Working({ progress, verb }: { progress: LockProgress | null; verb: string }) {
  const total = progress?.total ?? 0;
  const done = progress?.done ?? 0;
  return (
    <div className="grid gap-2" role="status" aria-label={`${verb} items`}>
      <Progress value={total > 0 ? (done / total) * 100 : 0} />
      <p className="text-xs text-muted-foreground">
        {total > 0
          ? `${verb} ${done.toLocaleString("en")} of ${total.toLocaleString("en")}…`
          : `${verb}…`}
      </p>
    </div>
  );
}

function LockFolder({
  folder,
  onCancel,
  onLocked,
  onDone,
}: {
  folder: Folder;
  onCancel: () => void;
  onLocked: () => void;
  onDone: () => void;
}) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [label, setLabel] = useState("");
  const [color, setColor] = useState<string | null>(null);
  const [working, setWorking] = useState(false);
  const [progress, setProgress] = useState<LockProgress | null>(null);
  const [error, setError] = useState<string>();
  const [key, setKey] = useState<RecoveryKey | null>(null);
  const labelId = useId();
  const mismatch = confirm !== "" && confirm !== password;
  const items = folder.descendantImageCount ?? 0;

  if (key) {
    return <RecoveryKeyDialog folderName={folder.name} recovery={key} onDone={onDone} />;
  }
  return (
    <Dialog open onOpenChange={(open) => !open && !working && onCancel()}>
      <DialogContent showCloseButton={!working}>
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (!password || mismatch || working) return;
            setWorking(true);
            setError(undefined);
            ipc
              .lockFolder(folder.id, password, label.trim() || null, color, setProgress)
              .then((recovery) => {
                setKey(recovery);
                onLocked();
              })
              .catch((reason: unknown) => setError(describeError(reason)))
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>Lock “{folder.name}”</DialogTitle>
            <DialogDescription>
              Locking encrypts the folder on disk: its files, previews, names and notes.
              {items > 0 ? ` ${plural(items, "item")} will be encrypted.` : ""} Without the password
              or the recovery key nobody can open them, not even you.
            </DialogDescription>
          </DialogHeader>
          {working ? (
            <Working progress={progress} verb="Encrypting" />
          ) : (
            <>
              <PasswordField label="Password" value={password} onChange={setPassword} autoFocus />
              <PasswordField
                label="Repeat the password"
                value={confirm}
                onChange={setConfirm}
                invalid={mismatch}
              />
              {mismatch && <ErrorText>The passwords are not the same.</ErrorText>}
              <div className="grid gap-1">
                <label htmlFor={labelId} className="text-xs text-muted-foreground">
                  Label (optional, shown on the lock and not encrypted)
                </label>
                <Input
                  id={labelId}
                  value={label}
                  maxLength={60}
                  onChange={(event) => setLabel(event.target.value)}
                />
              </div>
              <ColourPicker legend="Label colour" value={color} onChange={setColor} />
            </>
          )}
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              Cancel
            </Button>
            <Button type="submit" disabled={!password || mismatch || confirm === "" || working}>
              Lock
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/**
 * The recovery key, shown once (docs/age-locked-folders.md §7): as a key or as 12 words, to
 * copy or save to a file outside the library. It closes only after the user says it is saved.
 */
export function RecoveryKeyDialog({
  folderName,
  recovery,
  onDone,
}: {
  folderName: string;
  recovery: RecoveryKey;
  onDone: () => void;
}) {
  const [words, setWords] = useState(false);
  const [saved, setSaved] = useState(false);
  const [status, setStatus] = useState<string>();
  const shown = words ? recovery.phrase : recovery.key;
  return (
    <Dialog open>
      <DialogContent
        showCloseButton={false}
        onEscapeKeyDown={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>Save the recovery key</DialogTitle>
          <DialogDescription>
            If you forget the password of “{folderName}”, this key is the only way to open it. It is
            shown now and never again. Keep it somewhere safe, away from the library.
          </DialogDescription>
        </DialogHeader>
        <figure aria-label="Recovery key">
          <p className="rounded-md border bg-muted px-3 py-2 text-center font-mono text-sm break-words select-all">
            {shown}
          </p>
        </figure>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" onClick={() => setWords(!words)}>
            {words ? "Show as a key" : "Show as 12 words"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              navigator.clipboard
                ?.writeText(`${recovery.key}\n${recovery.phrase}`)
                .then(() => setStatus("Copied."))
                .catch(() => setStatus("Copying failed: write the key down instead."));
            }}
          >
            Copy
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              save({
                title: "Save the recovery key",
                defaultPath: `${folderName} recovery key.txt`,
                filters: [{ name: "Text", extensions: ["txt"] }],
              })
                .then((path) =>
                  path
                    ? ipc.saveRecoveryFile(path, folderName, recovery).then(() => "Saved.")
                    : undefined,
                )
                .then((message) => message && setStatus(message))
                .catch((error: unknown) => setStatus(describeError(error)));
            }}
          >
            Save to a file…
          </Button>
        </div>
        {status && (
          <p role="status" className="text-xs text-muted-foreground">
            {status}
          </p>
        )}
        <label className="flex items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            checked={saved}
            onChange={(event) => setSaved(event.target.checked)}
          />
          I saved the recovery key
        </label>
        <DialogFooter>
          <Button disabled={!saved} onClick={onDone}>
            Done
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function UnlockFolder({
  id,
  name,
  onCancel,
  onDone,
}: {
  id: string;
  name: string;
  onCancel: () => void;
  onDone: (opened?: string) => void;
}) {
  const [mode, setMode] = useState<"password" | "recovery">("password");
  const [password, setPassword] = useState("");
  const [recovery, setRecovery] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  const recoveryId = useId();
  const mismatch = confirm !== "" && confirm !== next;
  const ready =
    mode === "password"
      ? password !== ""
      : recovery.trim() !== "" && next !== "" && confirm !== "" && !mismatch;

  const run = (call: Promise<void>, wrong: string) => {
    setWorking(true);
    setError(undefined);
    call
      .then(() => onDone(id))
      .catch((reason: unknown) => setError(isWrongPassword(reason) ? wrong : describeError(reason)))
      .finally(() => setWorking(false));
  };
  return (
    <Dialog open onOpenChange={(open) => !open && !working && onCancel()}>
      <DialogContent>
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (!ready || working) return;
            if (mode === "password") {
              run(ipc.unlockFolder(id, password), "The password is not right.");
            } else {
              run(
                ipc.recoverFolder(id, recovery, next),
                "This recovery key does not open the folder.",
              );
            }
          }}
        >
          <DialogHeader>
            <DialogTitle>Unlock “{name}”</DialogTitle>
            <DialogDescription>
              {mode === "password"
                ? "The folder opens until you lock it again or close the library. Other locked folders stay locked."
                : "Enter the recovery key or its 12 words, then a new password. The old password stops working."}
            </DialogDescription>
          </DialogHeader>
          {mode === "password" ? (
            <PasswordField label="Password" value={password} onChange={setPassword} autoFocus />
          ) : (
            <>
              <div className="grid gap-1">
                <label htmlFor={recoveryId} className="text-xs text-muted-foreground">
                  Recovery key or words
                </label>
                <textarea
                  id={recoveryId}
                  rows={2}
                  autoComplete="off"
                  spellCheck={false}
                  autoFocus
                  value={recovery}
                  onChange={(event) => setRecovery(event.target.value)}
                  className="rounded-lg border bg-transparent px-2.5 py-1 font-mono text-[13px] outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
                />
              </div>
              <PasswordField label="New password" value={next} onChange={setNext} />
              <PasswordField
                label="Repeat the new password"
                value={confirm}
                onChange={setConfirm}
                invalid={mismatch}
              />
              {mismatch && <ErrorText>The passwords are not the same.</ErrorText>}
            </>
          )}
          {error && <ErrorText>{error}</ErrorText>}
          {working && <p className="text-xs text-muted-foreground">Checking…</p>}
          <DialogFooter className="sm:justify-between">
            <Button
              type="button"
              variant="link"
              className="px-0"
              onClick={() => {
                setMode(mode === "password" ? "recovery" : "password");
                setError(undefined);
              }}
            >
              {mode === "password" ? "Forgot the password?" : "Use the password"}
            </Button>
            <div className="flex gap-2">
              <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
                Cancel
              </Button>
              <Button type="submit" disabled={!ready || working}>
                Unlock
              </Button>
            </div>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ChangePassword({
  id,
  name,
  onCancel,
  onDone,
}: {
  id: string;
  name: string;
  onCancel: () => void;
  onDone: () => void;
}) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  const mismatch = confirm !== "" && confirm !== next;
  const ready = current !== "" && next !== "" && confirm !== "" && !mismatch;
  return (
    <Dialog open onOpenChange={(open) => !open && !working && onCancel()}>
      <DialogContent>
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (!ready || working) return;
            setWorking(true);
            setError(undefined);
            ipc
              .changeFolderPassword(id, current, next)
              .then(onDone)
              .catch((reason: unknown) =>
                setError(
                  isWrongPassword(reason)
                    ? "The current password is not right."
                    : describeError(reason),
                ),
              )
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>Change the password of “{name}”</DialogTitle>
            <DialogDescription>
              The files stay as they are. The recovery key keeps working.
            </DialogDescription>
          </DialogHeader>
          <PasswordField label="Current password" value={current} onChange={setCurrent} autoFocus />
          <PasswordField label="New password" value={next} onChange={setNext} />
          <PasswordField
            label="Repeat the new password"
            value={confirm}
            onChange={setConfirm}
            invalid={mismatch}
          />
          {mismatch && <ErrorText>The passwords are not the same.</ErrorText>}
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              Cancel
            </Button>
            <Button type="submit" disabled={!ready || working}>
              Change password
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RenewRecovery({
  id,
  name,
  onCancel,
  onDone,
}: {
  id: string;
  name: string;
  onCancel: () => void;
  onDone: () => void;
}) {
  const [current, setCurrent] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  const [key, setKey] = useState<RecoveryKey | null>(null);
  if (key) return <RecoveryKeyDialog folderName={name} recovery={key} onDone={onDone} />;
  return (
    <Dialog open onOpenChange={(open) => !open && !working && onCancel()}>
      <DialogContent>
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (!current || working) return;
            setWorking(true);
            setError(undefined);
            ipc
              .renewRecoveryKey(id, current)
              .then(setKey)
              .catch((reason: unknown) =>
                setError(
                  isWrongPassword(reason) ? "The password is not right." : describeError(reason),
                ),
              )
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>New recovery key for “{name}”</DialogTitle>
            <DialogDescription>
              The old recovery key stops working. The new one is shown once.
            </DialogDescription>
          </DialogHeader>
          <PasswordField label="Password" value={current} onChange={setCurrent} autoFocus />
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              Cancel
            </Button>
            <Button type="submit" disabled={!current || working}>
              Make a new key
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RemoveLock({
  folder,
  onCancel,
  onDone,
}: {
  folder: Folder;
  onCancel: () => void;
  onDone: () => void;
}) {
  const [password, setPassword] = useState("");
  const [working, setWorking] = useState(false);
  const [progress, setProgress] = useState<LockProgress | null>(null);
  const [error, setError] = useState<string>();
  return (
    <Dialog open onOpenChange={(open) => !open && !working && onCancel()}>
      <DialogContent showCloseButton={!working}>
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (!password || working) return;
            setWorking(true);
            setError(undefined);
            ipc
              .removeFolderLock(folder.id, password, setProgress)
              .then(onDone)
              .catch((reason: unknown) =>
                setError(
                  isWrongPassword(reason) ? "The password is not right." : describeError(reason),
                ),
              )
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>Remove the lock of “{folder.name}”?</DialogTitle>
            <DialogDescription>
              Its files are decrypted on disk and the folder is no longer hidden. The password and
              the recovery key stop mattering.
            </DialogDescription>
          </DialogHeader>
          {working ? (
            <Working progress={progress} verb="Decrypting" />
          ) : (
            <PasswordField label="Password" value={password} onChange={setPassword} autoFocus />
          )}
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              Cancel
            </Button>
            <Button type="submit" variant="destructive" disabled={!password || working}>
              Remove lock
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
