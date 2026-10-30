import { useId, useState } from "react";
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
      aria-label="Log in"
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
          {method === "password" && "Enter the login password. It does not unlock folders."}
          {method === "pin" && "Enter the login PIN. It does not unlock folders."}
          {method === "webAuthn" &&
            "Touch the YubiKey. This only opens the app; it does not unlock folders, and it does not ask for a PIN."}
        </p>
      </div>
      {method === "password" && (
        <div className="grid gap-1">
          <label htmlFor={fieldId} className="text-xs text-muted-foreground">
            Password
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
            PIN
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
        {method === "webAuthn" ? "Touch your YubiKey" : "Log in"}
      </Button>
    </form>
  );
}
