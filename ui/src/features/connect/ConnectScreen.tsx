import { ArrowLeft } from "lucide-react";
import { type FormEvent, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { ConnectionSettings } from "@/ipc";
import { type ConnectForm, isValid, validateConnectForm } from "./validation";

type Props = {
  defaults: ConnectionSettings;
  onConnect: (form: ConnectForm) => void;
  /** Back to the Welcome screen; the button is hidden without it. */
  onBack?: () => void;
  /** A message under the form, for example the result of the last attempt. */
  status?: string;
};

/** Connecting to a running Eagle, a secondary feature (docs/spec.md §8.3). */
export function ConnectScreen({ defaults, onConnect, onBack, status }: Props) {
  const [form, setForm] = useState<ConnectForm>({
    host: defaults.host,
    port: String(defaults.port),
    token: "",
  });
  const errors = validateConnectForm(form);
  const valid = isValid(errors);

  const update = (field: keyof ConnectForm) => (value: string) =>
    setForm((current) => ({ ...current, [field]: value }));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (valid) {
      onConnect({ ...form, host: form.host.trim(), port: form.port.trim() });
    }
  };

  return (
    <Card className="w-full max-w-md">
      <form onSubmit={submit} noValidate>
        <CardHeader>
          <CardTitle>Connect to Eagle</CardTitle>
          <CardDescription>
            Eagle must be running. Use 127.0.0.1 when it runs on this PC.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 py-4">
          <Field id="host" label="Host" error={errors.host}>
            <Input
              id="host"
              value={form.host}
              onChange={(e) => update("host")(e.target.value)}
              aria-invalid={errors.host !== undefined}
              aria-describedby={errors.host ? "host-error" : undefined}
              autoComplete="off"
              spellCheck={false}
            />
          </Field>
          <Field id="port" label="Port" error={errors.port}>
            <Input
              id="port"
              inputMode="numeric"
              value={form.port}
              onChange={(e) => update("port")(e.target.value)}
              aria-invalid={errors.port !== undefined}
              aria-describedby={errors.port ? "port-error" : undefined}
            />
          </Field>
          <Field
            id="token"
            label="API token"
            hint="Eagle → Preferences → Developer. Not needed when Eagle runs on this PC."
          >
            <Input
              id="token"
              type="password"
              value={form.token}
              onChange={(e) => update("token")(e.target.value)}
              autoComplete="off"
            />
          </Field>
        </CardContent>
        <CardFooter className="flex flex-col items-stretch gap-2">
          <Button type="submit" disabled={!valid}>
            Connect
          </Button>
          {onBack && (
            <Button type="button" variant="ghost" onClick={onBack}>
              <ArrowLeft aria-hidden />
              Back
            </Button>
          )}
          {status && (
            <p role="status" className="text-sm text-muted-foreground">
              {status}
            </p>
          )}
        </CardFooter>
      </form>
    </Card>
  );
}

type FieldProps = {
  id: string;
  label: string;
  error?: string;
  hint?: string;
  children: React.ReactNode;
};

function Field({ id, label, error, hint, children }: FieldProps) {
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {error ? (
        <p id={`${id}-error`} className="text-sm text-destructive">
          {error}
        </p>
      ) : (
        hint && <p className="text-xs text-muted-foreground">{hint}</p>
      )}
    </div>
  );
}
