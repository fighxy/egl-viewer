import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { LoginMethod, LoginProof } from "@/ipc";
import { describeError } from "@/lib/errors";

type Method = Exclude<LoginMethod, "none">;

type Props = {
  method: Method;
  onSubmit: (proof: LoginProof) => Promise<void>;
};

/**
 * The gate before Welcome (docs/spec.md §13). Exactly one method: a password, a PIN, or a
 * YubiKey touch. WebAuthn does not ask for a PIN and does not unlock folders.
 */
export function LoginScreen({ method, onSubmit }: Props) {
  const { t } = useTranslation();
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const fieldId = useId();
  const submit = (proof: LoginProof) => {
    setBusy(true);
    setError(undefined);
    onSubmit(proof)
      .catch((reason: unknown) => setError(describeError(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <form
      className="grid w-full max-w-sm gap-4"
      aria-label={t("login.form")}
      onSubmit={(event) => {
        event.preventDefault();
        if (busy) return;
        if (method === "webAuthn") submit({ kind: "webAuthn" });
        else if (secret !== "") submit({ kind: method, secret });
      }}
    >
      <div className="grid gap-1">
        <h1 className="text-xl font-semibold">egl-viewer</h1>
        <p className="text-sm text-muted-foreground">
          {method === "password" && t("login.passwordHint")}
          {method === "pin" && t("login.pinHint")}
          {method === "webAuthn" && t("login.webAuthnHint")}
        </p>
      </div>
      {method === "password" && (
        <div className="grid gap-1">
          <label htmlFor={fieldId} className="text-xs text-muted-foreground">
            {t("login.password")}
          </label>
          <Input
            id={fieldId}
            type="password"
            autoComplete="current-password"
            autoFocus
            value={secret}
            onChange={(event) => setSecret(event.target.value)}
          />
        </div>
      )}
      {method === "pin" && (
        <div className="grid gap-1">
          <label htmlFor={fieldId} className="text-xs text-muted-foreground">
            {t("login.pin")}
          </label>
          <Input
            id={fieldId}
            type="password"
            inputMode="numeric"
            autoComplete="off"
            autoFocus
            value={secret}
            onChange={(event) => setSecret(event.target.value)}
          />
        </div>
      )}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <Button type="submit" disabled={busy || (method !== "webAuthn" && secret === "")}>
        {method === "webAuthn" ? t("login.touchYubiKey") : t("login.submit")}
      </Button>
    </form>
  );
}
