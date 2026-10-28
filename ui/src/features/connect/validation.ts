// Mirrors ConnectionSettings::validate in crates/app-store, so the form reports problems before
// anything is sent to Rust.

export type ConnectForm = {
  host: string;
  port: string;
  token: string;
};

export type ConnectErrors = Partial<Record<"host" | "port", string>>;

export function validateConnectForm(form: ConnectForm): ConnectErrors {
  const errors: ConnectErrors = {};

  const host = form.host.trim();
  if (host === "") {
    errors.host = "Enter the address of the PC that runs Eagle.";
  } else if (/\s|\//.test(host)) {
    errors.host = "Enter a computer name or an IP address, without spaces or slashes.";
  }

  const port = form.port.trim();
  const portNumber = Number(port);
  if (!/^\d+$/.test(port) || portNumber < 1 || portNumber > 65535) {
    errors.port = "The port is a number from 1 to 65535.";
  }

  return errors;
}

export function isValid(errors: ConnectErrors): boolean {
  return Object.keys(errors).length === 0;
}
