import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { App } from "@/App";
import { mockBackend } from "@/test/backend";
import { LoginScreen } from "./LoginScreen";

describe("LoginScreen", () => {
  it("asks for a password and not a PIN or a touch", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<LoginScreen method="password" onSubmit={onSubmit} />);
    expect(screen.queryByLabelText("PIN")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Touch your YubiKey" })).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("Password"), "secret");
    await user.click(screen.getByRole("button", { name: "Log in" }));
    expect(onSubmit).toHaveBeenCalledWith({ kind: "password", secret: "secret" });
  });

  it("asks for a PIN and not a password", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<LoginScreen method="pin" onSubmit={onSubmit} />);
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Touch your YubiKey" })).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("PIN"), "1234");
    await user.click(screen.getByRole("button", { name: "Log in" }));
    expect(onSubmit).toHaveBeenCalledWith({ kind: "pin", secret: "1234" });
  });

  it("asks for a YubiKey touch and not a PIN", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<LoginScreen method="webAuthn" onSubmit={onSubmit} />);
    expect(screen.queryByLabelText("PIN")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    expect(screen.getByText(/does not unlock folders/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Touch your YubiKey" }));
    expect(onSubmit).toHaveBeenCalledWith({ kind: "webAuthn" });
  });
});

describe("the login gate", () => {
  it("opens Welcome when login is off", async () => {
    mockBackend({ login_status: () => ({ method: "none" }) });
    render(<App />);
    expect(
      await screen.findByRole("heading", { name: "Welcome to egl-viewer" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "Log in" })).not.toBeInTheDocument();
  });

  it("stays on the password gate until the password is accepted", async () => {
    let attempts = 0;
    mockBackend({
      login_status: () => ({ method: "password" }),
      unlock_app: () => {
        attempts += 1;
        if (attempts === 1) throw { kind: "wrongPassword" };
        return null;
      },
    });
    const user = userEvent.setup();
    render(<App />);
    const form = await screen.findByRole("form", { name: "Log in" });
    expect(
      screen.queryByRole("heading", { name: "Welcome to egl-viewer" }),
    ).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("Password"), "nope");
    await user.click(screen.getByRole("button", { name: "Log in" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not right");
    await user.clear(screen.getByLabelText("Password"));
    await user.type(screen.getByLabelText("Password"), "secret");
    await user.click(screen.getByRole("button", { name: "Log in" }));
    expect(
      await screen.findByRole("heading", { name: "Welcome to egl-viewer" }),
    ).toBeInTheDocument();
    expect(form).not.toBeInTheDocument();
  });
});
