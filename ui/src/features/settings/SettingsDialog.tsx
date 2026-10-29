import { cn } from "cn";
import { type FormEvent, useEffect, useState } from "react";
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
import { Switch } from "@/components/ui/switch";
import type { LoginMethod, Preferences, Theme } from "@/ipc";
import { describeError } from "@/lib/errors";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  preferences: Preferences;
  /** `%USERPROFILE%\egl-library`, shown when no other default folder is set. */
  builtInLocation: string;
  browse: (current: string) => Promise<string | null>;
  /** Rejects with the reason when the default folder cannot hold libraries. */
  save: (preferences: Preferences) => Promise<void>;
  /** The one login method. Changing it replaces the previous one (spec §13.2). */
  login: LoginMethod;
  onSetPassword: (password: string) => Promise<void>;
  onSetPin: (pin: string) => Promise<void>;
  onSetWebAuthn: () => Promise<void>;
  onClearLogin: () => Promise<void>;
};

const THEMES: readonly { value: Theme; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

const LOGIN_METHODS: readonly { value: LoginMethod; label: string }[] = [
  { value: "none", label: "Off" },
  { value: "password", label: "Password" },
  { value: "pin", label: "PIN" },
  { value: "webAuthn", label: "YubiKey" },
];

/** Settings → General (docs/spec.md §7.4, D-027). */
export function SettingsDialog({
  open,
  onOpenChange,
  preferences,
  builtInLocation,
  browse,
  save,
  login,
  onSetPassword,
  onSetPin,
  onSetWebAuthn,
  onClearLogin,
}: Props) {
  const [draft, setDraft] = useState(preferences);
  const [method, setMethod] = useState<LoginMethod>(login);
  const [secret, setSecret] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setDraft(preferences);
      setMethod(login);
      setSecret("");
      setError(undefined);
    }
  }, [open, preferences, login]);

  const folder = draft.defaultLibraryParent ?? "";

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      await save({ ...draft, defaultLibraryParent: folder.trim() || null });
      onOpenChange(false);
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  const pick = async () => {
    const picked = await browse(folder || builtInLocation).catch(() => null);
    if (picked) setDraft({ ...draft, defaultLibraryParent: picked });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <form className="grid gap-5" onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>General</DialogDescription>
          </DialogHeader>
          <fieldset className="grid gap-1.5">
            <legend className="mb-1.5 text-sm font-medium">Theme</legend>
            <div className="flex gap-1">
              {THEMES.map(({ value, label }) => (
                <label
                  key={value}
                  className={cn(
                    "rounded-md border px-3 py-1 text-[13px] has-focus-visible:ring-3 has-focus-visible:ring-ring/50",
                    draft.theme === value
                      ? "border-primary bg-primary text-primary-foreground"
                      : "hover:bg-muted",
                  )}
                >
                  <input
                    type="radio"
                    name="theme"
                    value={value}
                    className="sr-only"
                    checked={draft.theme === value}
                    onChange={() => setDraft({ ...draft, theme: value })}
                  />
                  {label}
                </label>
              ))}
            </div>
          </fieldset>
          <div className="flex items-center justify-between gap-4">
            <Label htmlFor="reopen-last-library">Reopen the last library on start</Label>
            <Switch
              id="reopen-last-library"
              checked={draft.reopenLastLibrary}
              onCheckedChange={(checked) => setDraft({ ...draft, reopenLastLibrary: checked })}
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="default-folder">Default folder for new libraries</Label>
            <div className="flex gap-2">
              <Input
                id="default-folder"
                value={folder}
                placeholder={builtInLocation}
                onChange={(event) =>
                  setDraft({ ...draft, defaultLibraryParent: event.target.value })
                }
              />
              <Button type="button" variant="outline" onClick={pick}>
                Browse…
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Leave empty to use {builtInLocation}. Changing it does not move existing libraries.
            </p>
          </div>
          <fieldset className="grid gap-2">
            <legend className="text-sm font-medium">App login</legend>
            <p className="text-xs text-muted-foreground">
              One method. A password, a PIN, or a YubiKey touch. The touch does not unlock folders,
              and choosing it means login does not ask for a PIN.
            </p>
            <div className="flex flex-wrap gap-1">
              {LOGIN_METHODS.map(({ value, label }) => (
                <label
                  key={value}
                  className={cn(
                    "rounded-md border px-3 py-1 text-[13px] has-focus-visible:ring-3 has-focus-visible:ring-ring/50",
                    method === value
                      ? "border-primary bg-primary text-primary-foreground"
                      : "hover:bg-muted",
                  )}
                >
                  <input
                    type="radio"
                    name="login"
                    value={value}
                    className="sr-only"
                    checked={method === value}
                    onChange={() => {
                      setMethod(value);
                      setSecret("");
                    }}
                  />
                  {label}
                </label>
              ))}
            </div>
            {method === "password" && (
              <Input
                aria-label="Login password"
                type="password"
                autoComplete="new-password"
                value={secret}
                onChange={(event) => setSecret(event.target.value)}
              />
            )}
            {method === "pin" && (
              <Input
                aria-label="Login PIN"
                type="password"
                inputMode="numeric"
                autoComplete="off"
                value={secret}
                onChange={(event) => setSecret(event.target.value)}
              />
            )}
            {method === "webAuthn" && (
              <p className="text-xs text-muted-foreground">Touch the YubiKey when asked.</p>
            )}
            <div>
              <Button
                type="button"
                variant="outline"
                disabled={
                  busy || ((method === "password" || method === "pin") && secret.trim() === "")
                }
                onClick={() => {
                  setBusy(true);
                  setError(undefined);
                  const apply =
                    method === "none"
                      ? onClearLogin()
                      : method === "password"
                        ? onSetPassword(secret)
                        : method === "pin"
                          ? onSetPin(secret)
                          : onSetWebAuthn();
                  apply
                    .then(() => setSecret(""))
                    .catch((failure: unknown) => setError(describeError(failure)))
                    .finally(() => setBusy(false));
                }}
              >
                Set login
              </Button>
            </div>
          </fieldset>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
