import { cn } from "cn";
import { type FormEvent, useEffect, useState } from "react";
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
import { Switch } from "@/components/ui/switch";
import { APP_LOCALES } from "@/i18n";
import type { Locale, LoginMethod, Preferences, Theme } from "@/ipc";
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

const THEMES: readonly Theme[] = ["system", "light", "dark"];

const LOGIN_METHODS: readonly LoginMethod[] = ["none", "password", "pin", "webAuthn"];

/** Settings → General (docs/spec.md §7.4, D-027, D-069). */
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
  const { t } = useTranslation();
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

  const loginLabel = (value: LoginMethod) => {
    switch (value) {
      case "none":
        return t("settings.loginOff");
      case "password":
        return t("settings.loginPasswordMethod");
      case "pin":
        return t("settings.loginPinMethod");
      case "webAuthn":
        return t("settings.loginWebAuthn");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <form className="grid gap-5" onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>{t("settings.title")}</DialogTitle>
            <DialogDescription>{t("settings.general")}</DialogDescription>
          </DialogHeader>
          <fieldset className="grid gap-1.5">
            <legend className="mb-1.5 text-sm font-medium">{t("settings.theme")}</legend>
            <div className="flex gap-1">
              {THEMES.map((value) => (
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
                  {t(`themes.${value}`)}
                </label>
              ))}
            </div>
          </fieldset>
          <fieldset className="grid gap-1.5">
            <legend className="mb-1.5 text-sm font-medium">{t("settings.language")}</legend>
            <div className="flex flex-wrap gap-1">
              {APP_LOCALES.map(({ value, nativeLabel }) => (
                <label
                  key={value}
                  className={cn(
                    "rounded-md border px-3 py-1 text-[13px] has-focus-visible:ring-3 has-focus-visible:ring-ring/50",
                    draft.locale === value
                      ? "border-primary bg-primary text-primary-foreground"
                      : "hover:bg-muted",
                  )}
                >
                  <input
                    type="radio"
                    name="locale"
                    value={value}
                    className="sr-only"
                    checked={draft.locale === value}
                    onChange={() => setDraft({ ...draft, locale: value as Locale })}
                  />
                  {nativeLabel}
                </label>
              ))}
            </div>
          </fieldset>
          <div className="flex items-center justify-between gap-4">
            <Label htmlFor="reopen-last-library">{t("settings.reopenLast")}</Label>
            <Switch
              id="reopen-last-library"
              checked={draft.reopenLastLibrary}
              onCheckedChange={(checked) => setDraft({ ...draft, reopenLastLibrary: checked })}
            />
          </div>
          <div className="grid gap-1.5">
            <div className="flex items-center justify-between gap-4">
              <Label htmlFor="confirm-before-trash">{t("settings.confirmTrash")}</Label>
              <Switch
                id="confirm-before-trash"
                checked={draft.confirmBeforeTrash}
                onCheckedChange={(checked) => setDraft({ ...draft, confirmBeforeTrash: checked })}
              />
            </div>
            <p className="text-xs text-muted-foreground">{t("settings.confirmTrashHint")}</p>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="default-folder">{t("settings.defaultFolder")}</Label>
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
                {t("common.browse")}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              {t("settings.defaultFolderHint", { path: builtInLocation })}
            </p>
          </div>
          <fieldset className="grid gap-2">
            <legend className="text-sm font-medium">{t("settings.appLogin")}</legend>
            <p className="text-xs text-muted-foreground">{t("settings.appLoginHint")}</p>
            <div className="flex flex-wrap gap-1">
              {LOGIN_METHODS.map((value) => (
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
                  {loginLabel(value)}
                </label>
              ))}
            </div>
            {method === "password" && (
              <Input
                aria-label={t("settings.loginPassword")}
                type="password"
                autoComplete="new-password"
                value={secret}
                onChange={(event) => setSecret(event.target.value)}
              />
            )}
            {method === "pin" && (
              <Input
                aria-label={t("settings.loginPin")}
                type="password"
                inputMode="numeric"
                autoComplete="off"
                value={secret}
                onChange={(event) => setSecret(event.target.value)}
              />
            )}
            {method === "webAuthn" && (
              <p className="text-xs text-muted-foreground">{t("settings.webAuthnHint")}</p>
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
                {t("settings.setLogin")}
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
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={busy}>
              {t("common.save")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
