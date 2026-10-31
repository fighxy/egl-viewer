import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useId, useState } from "react";
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
import { Progress } from "@/components/ui/progress";
import { ColourPicker } from "@/features/folders/ColourPicker";
import { findFolder } from "@/features/folders/tree";
import {
  type Folder,
  ipc,
  type LockProgress,
  type PivPolicy,
  type PivWrap,
  type RecoveryKey,
  type SourceError,
} from "@/ipc";
import { describeError } from "@/lib/errors";

/** Which lock dialog is open, for which folder. */
export type LockDialog =
  | { kind: "lock"; id: string }
  | { kind: "unlock"; id: string }
  | { kind: "change"; id: string }
  | { kind: "renew"; id: string }
  | { kind: "remove"; id: string }
  | { kind: "yubikey"; id: string }
  | { kind: "unyubikey"; id: string };

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
      {dialog?.kind === "yubikey" && (
        <AddYubiKey
          key={dialog.id}
          id={dialog.id}
          name={name}
          onCancel={onClose}
          onDone={onClose}
        />
      )}
      {dialog?.kind === "unyubikey" && (
        <RemoveYubiKey
          key={dialog.id}
          id={dialog.id}
          name={name}
          onCancel={onClose}
          onDone={onClose}
        />
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
  const { t, i18n } = useTranslation();
  const total = progress?.total ?? 0;
  const done = progress?.done ?? 0;
  return (
    <div className="grid gap-2" role="status" aria-label={t("locksUi.workingItems", { verb })}>
      <Progress value={total > 0 ? (done / total) * 100 : 0} />
      <p className="text-xs text-muted-foreground">
        {total > 0
          ? t("locksUi.progressCounted", {
              verb,
              done: done.toLocaleString(i18n.language),
              total: total.toLocaleString(i18n.language),
            })
          : t("locksUi.progressPlain", { verb })}
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
  const { t, i18n } = useTranslation();
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
            <DialogTitle>{t("locksUi.lockTitle", { name: folder.name })}</DialogTitle>
            <DialogDescription>
              {t("locksUi.lockIntro")}
              {items > 0
                ? ` ${t("locksUi.lockEncryptCount", {
                    items: t("common.item", {
                      count: items,
                      formatted: items.toLocaleString(i18n.language),
                    }),
                  })}`
                : ""}{" "}
              {t("locksUi.lockOutro")}
            </DialogDescription>
          </DialogHeader>
          {working ? (
            <Working progress={progress} verb={t("locksUi.encrypting")} />
          ) : (
            <>
              <PasswordField
                label={t("locksUi.password")}
                value={password}
                onChange={setPassword}
                autoFocus
              />
              <PasswordField
                label={t("locksUi.repeatPassword")}
                value={confirm}
                onChange={setConfirm}
                invalid={mismatch}
              />
              {mismatch && <ErrorText>{t("locksUi.mismatch")}</ErrorText>}
              <div className="grid gap-1">
                <label htmlFor={labelId} className="text-xs text-muted-foreground">
                  {t("locksUi.labelOptional")}
                </label>
                <Input
                  id={labelId}
                  value={label}
                  maxLength={60}
                  onChange={(event) => setLabel(event.target.value)}
                />
              </div>
              <ColourPicker legend={t("locksUi.labelColour")} value={color} onChange={setColor} />
            </>
          )}
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!password || mismatch || confirm === "" || working}>
              {t("commonExtra.lock")}
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
  const { t } = useTranslation();
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
          <DialogTitle>{t("locksUi.saveRecoveryTitle")}</DialogTitle>
          <DialogDescription>
            {t("locksUi.saveRecoveryBody", { name: folderName })}
          </DialogDescription>
        </DialogHeader>
        <figure aria-label={t("locksUi.recoveryKeyAria")}>
          <p className="rounded-md border bg-muted px-3 py-2 text-center font-mono text-sm break-words select-all">
            {shown}
          </p>
        </figure>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" onClick={() => setWords(!words)}>
            {words ? t("locksUi.showAsKey") : t("locksUi.showAsWords")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              navigator.clipboard
                ?.writeText(`${recovery.key}\n${recovery.phrase}`)
                .then(() => setStatus(t("locksUi.copied")))
                .catch(() => setStatus(t("locksUi.copyFailed")));
            }}
          >
            {t("commonExtra.copy")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              save({
                title: t("locksUi.saveRecoveryTitle"),
                defaultPath: t("locksUi.recoveryFileName", { name: folderName }),
                filters: [{ name: t("locksUi.textFilter"), extensions: ["txt"] }],
              })
                .then((path) =>
                  path
                    ? ipc.saveRecoveryFile(path, folderName, recovery).then(() => t("locksUi.saved"))
                    : undefined,
                )
                .then((message) => message && setStatus(message))
                .catch((error: unknown) => setStatus(describeError(error)));
            }}
          >
            {t("locksUi.saveToFile")}
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
          {t("locksUi.savedRecoveryConfirm")}
        </label>
        <DialogFooter>
          <Button disabled={!saved} onClick={onDone}>
            {t("commonExtra.done")}
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
  const { t } = useTranslation();
  const [mode, setMode] = useState<"password" | "recovery" | "yubikey">("password");
  const [password, setPassword] = useState("");
  const [recovery, setRecovery] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [wraps, setWraps] = useState<PivWrap[]>([]);
  const [serial, setSerial] = useState("");
  const [pin, setPin] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  const recoveryId = useId();
  const mismatch = confirm !== "" && confirm !== next;
  const selected = wraps.find((wrap) => wrap.serial === serial) ?? wraps[0];
  const ready =
    mode === "password"
      ? password !== ""
      : mode === "recovery"
        ? recovery.trim() !== "" && next !== "" && confirm !== "" && !mismatch
        : selected !== undefined && (selected.policy === "touch" || pin !== "");

  useEffect(() => {
    let cancelled = false;
    ipc
      .pivWraps(id)
      .then((rows) => {
        if (cancelled) return;
        setWraps(rows);
        const first = rows[0];
        if (first) setSerial(first.serial);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [id]);

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
              run(ipc.unlockFolder(id, password), t("locksUi.wrongPassword"));
            } else if (mode === "recovery") {
              run(ipc.recoverFolder(id, recovery, next), t("locksUi.wrongRecovery"));
            } else if (selected) {
              run(
                ipc.unlockWithYubiKey(
                  id,
                  selected.serial,
                  selected.policy === "pin" ? { kind: "pin", pin } : { kind: "touch" },
                ),
                selected.policy === "pin" ? t("locksUi.wrongYubiPin") : t("locksUi.yubiFailed"),
              );
            }
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("locksUi.unlockTitle", { name })}</DialogTitle>
            <DialogDescription>
              {mode === "password" && t("locksUi.unlockHint")}
              {mode === "recovery" && t("locksUi.recoveryHint")}
              {mode === "yubikey" &&
                selected?.policy === "pin" &&
                t("locksUi.yubiPinHint")}
              {mode === "yubikey" &&
                selected?.policy !== "pin" &&
                t("locksUi.yubiTouchHint")}
            </DialogDescription>
          </DialogHeader>
          {mode === "password" && (
            <PasswordField
              label={t("locksUi.password")}
              value={password}
              onChange={setPassword}
              autoFocus
            />
          )}
          {mode === "recovery" && (
            <>
              <div className="grid gap-1">
                <label htmlFor={recoveryId} className="text-xs text-muted-foreground">
                  {t("locksUi.recoveryKeyOrWords")}
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
              <PasswordField label={t("locksUi.newPassword")} value={next} onChange={setNext} />
              <PasswordField
                label={t("locksUi.repeatNewPassword")}
                value={confirm}
                onChange={setConfirm}
                invalid={mismatch}
              />
              {mismatch && <ErrorText>{t("locksUi.mismatch")}</ErrorText>}
            </>
          )}
          {mode === "yubikey" && selected && (
            <>
              {wraps.length > 1 && (
                <label className="grid gap-1 text-xs text-muted-foreground">
                  {t("locksUi.yubiKey")}
                  <select
                    aria-label={t("locksUi.yubiKey")}
                    className="rounded-lg border bg-transparent px-2.5 py-1 text-sm text-foreground"
                    value={selected.serial}
                    onChange={(event) => setSerial(event.target.value)}
                  >
                    {wraps.map((wrap) => (
                      <option key={wrap.serial} value={wrap.serial}>
                        {wrap.serial}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              {selected.policy === "pin" ? (
                <PasswordField
                  label={t("locksUi.yubiPin")}
                  value={pin}
                  onChange={setPin}
                  autoFocus
                />
              ) : (
                <p className="text-sm">{t("locksUi.touchWhenAsked")}</p>
              )}
            </>
          )}
          {error && <ErrorText>{error}</ErrorText>}
          {working && <p className="text-xs text-muted-foreground">{t("locksUi.checking")}</p>}
          <DialogFooter className="sm:justify-between">
            <div className="flex flex-wrap gap-2">
              {mode !== "password" && (
                <Button
                  type="button"
                  variant="link"
                  className="px-0"
                  onClick={() => {
                    setMode("password");
                    setError(undefined);
                  }}
                >
                  {t("locksUi.usePassword")}
                </Button>
              )}
              {mode !== "recovery" && (
                <Button
                  type="button"
                  variant="link"
                  className="px-0"
                  onClick={() => {
                    setMode("recovery");
                    setError(undefined);
                  }}
                >
                  {t("locksUi.forgotPassword")}
                </Button>
              )}
              {mode !== "yubikey" && wraps.length > 0 && (
                <Button
                  type="button"
                  variant="link"
                  className="px-0"
                  onClick={() => {
                    setMode("yubikey");
                    setError(undefined);
                  }}
                >
                  {t("locksUi.useYubiKey")}
                </Button>
              )}
            </div>
            <div className="flex gap-2">
              <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
                {t("common.cancel")}
              </Button>
              <Button type="submit" disabled={!ready || working}>
                {t("commonExtra.unlock")}
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
  const { t } = useTranslation();
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
                    ? t("locksUi.wrongCurrentPassword")
                    : describeError(reason),
                ),
              )
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("locksUi.changePasswordTitle", { name })}</DialogTitle>
            <DialogDescription>{t("locksUi.changePasswordBody")}</DialogDescription>
          </DialogHeader>
          <PasswordField
            label={t("locksUi.currentPassword")}
            value={current}
            onChange={setCurrent}
            autoFocus
          />
          <PasswordField label={t("locksUi.newPassword")} value={next} onChange={setNext} />
          <PasswordField
            label={t("locksUi.repeatNewPassword")}
            value={confirm}
            onChange={setConfirm}
            invalid={mismatch}
          />
          {mismatch && <ErrorText>{t("locksUi.mismatch")}</ErrorText>}
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!ready || working}>
              {t("locksUi.changePasswordAction")}
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
  const { t } = useTranslation();
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
                  isWrongPassword(reason) ? t("locksUi.wrongPassword") : describeError(reason),
                ),
              )
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("locksUi.newRecoveryTitle", { name })}</DialogTitle>
            <DialogDescription>{t("locksUi.renewBody")}</DialogDescription>
          </DialogHeader>
          <PasswordField
            label={t("locksUi.password")}
            value={current}
            onChange={setCurrent}
            autoFocus
          />
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!current || working}>
              {t("locksUi.makeNewKey")}
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
  const { t } = useTranslation();
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
                  isWrongPassword(reason) ? t("locksUi.wrongPassword") : describeError(reason),
                ),
              )
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("locksUi.removeLockTitle", { name: folder.name })}</DialogTitle>
            <DialogDescription>{t("locksUi.removeLockBody")}</DialogDescription>
          </DialogHeader>
          {working ? (
            <Working progress={progress} verb={t("locksUi.decrypting")} />
          ) : (
            <PasswordField
              label={t("locksUi.password")}
              value={password}
              onChange={setPassword}
              autoFocus
            />
          )}
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" variant="destructive" disabled={!password || working}>
              {t("locksUi.removeLockAction")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function AddYubiKey({
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
  const { t } = useTranslation();
  const [policy, setPolicy] = useState<PivPolicy>("touch");
  const [password, setPassword] = useState("");
  const [pin, setPin] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  const ready = password !== "" && (policy === "touch" || pin !== "");
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
              .bindYubiKey(id, password, policy, policy === "pin" ? pin : null)
              .then(onDone)
              .catch((reason: unknown) => setError(describeError(reason)))
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("locksUi.addYubiTitle", { name })}</DialogTitle>
            <DialogDescription>{t("locksUi.addYubiBody")}</DialogDescription>
          </DialogHeader>
          <fieldset className="grid gap-1">
            <legend className="text-xs text-muted-foreground">{t("locksUi.openWith")}</legend>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="piv-policy"
                checked={policy === "touch"}
                onChange={() => {
                  setPolicy("touch");
                  setPin("");
                }}
              />
              {t("locksUi.touch")}
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="piv-policy"
                checked={policy === "pin"}
                onChange={() => setPolicy("pin")}
              />
              {t("commonExtra.pin")}
            </label>
          </fieldset>
          {policy === "pin" && (
            <PasswordField label={t("locksUi.yubiPin")} value={pin} onChange={setPin} />
          )}
          <PasswordField
            label={t("locksUi.folderPassword")}
            value={password}
            onChange={setPassword}
            autoFocus
          />
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!ready || working}>
              {t("locksUi.addYubiAction")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function RemoveYubiKey({
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
  const { t } = useTranslation();
  const [wraps, setWraps] = useState<PivWrap[]>([]);
  const [serial, setSerial] = useState("");
  const [password, setPassword] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string>();
  useEffect(() => {
    let cancelled = false;
    ipc
      .pivWraps(id)
      .then((rows) => {
        if (cancelled) return;
        setWraps(rows);
        const first = rows[0];
        if (first) setSerial(first.serial);
      })
      .catch((reason: unknown) => setError(describeError(reason)));
    return () => {
      cancelled = true;
    };
  }, [id]);
  const chosen = wraps.find((wrap) => wrap.serial === serial) ?? wraps[0];
  return (
    <Dialog open onOpenChange={(open) => !open && !working && onCancel()}>
      <DialogContent>
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (!chosen || password === "" || working) return;
            setWorking(true);
            setError(undefined);
            ipc
              .unbindYubiKey(id, password, chosen.serial)
              .then(onDone)
              .catch((reason: unknown) => setError(describeError(reason)))
              .finally(() => setWorking(false));
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("locksUi.removeYubiTitle", { name })}</DialogTitle>
            <DialogDescription>{t("locksUi.removeYubiBody")}</DialogDescription>
          </DialogHeader>
          {wraps.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("locksUi.noYubiKey")}</p>
          ) : (
            <label className="grid gap-1 text-xs text-muted-foreground">
              {t("locksUi.yubiKey")}
              <select
                aria-label={t("locksUi.yubiKey")}
                className="rounded-lg border bg-transparent px-2.5 py-1 text-sm text-foreground"
                value={chosen?.serial ?? ""}
                onChange={(event) => setSerial(event.target.value)}
              >
                {wraps.map((wrap) => (
                  <option key={wrap.serial} value={wrap.serial}>
                    {wrap.serial}
                  </option>
                ))}
              </select>
            </label>
          )}
          <PasswordField
            label={t("locksUi.folderPassword")}
            value={password}
            onChange={setPassword}
            autoFocus
          />
          {error && <ErrorText>{error}</ErrorText>}
          <DialogFooter>
            <Button type="button" variant="outline" disabled={working} onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!chosen || password === "" || working}>
              {t("locksUi.removeYubiAction")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
