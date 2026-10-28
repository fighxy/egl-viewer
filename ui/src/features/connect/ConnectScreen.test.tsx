import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ConnectScreen } from "./ConnectScreen";

const defaults = { host: "127.0.0.1", port: 41595 };

describe("ConnectScreen", () => {
  it("starts with the defaults and an empty token", () => {
    render(<ConnectScreen defaults={defaults} onConnect={() => {}} />);
    expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
    expect(screen.getByLabelText("Port")).toHaveValue("41595");
    expect(screen.getByLabelText("API token")).toHaveValue("");
  });

  it("hides the token", () => {
    render(<ConnectScreen defaults={defaults} onConnect={() => {}} />);
    expect(screen.getByLabelText("API token")).toHaveAttribute("type", "password");
  });

  it("disables Connect and explains a bad port", async () => {
    const user = userEvent.setup();
    render(<ConnectScreen defaults={defaults} onConnect={() => {}} />);
    const port = screen.getByLabelText("Port");
    await user.clear(port);
    await user.type(port, "70000");
    expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
    expect(port).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByText("The port is a number from 1 to 65535.")).toBeInTheDocument();
  });

  it("sends the trimmed values", async () => {
    const user = userEvent.setup();
    const onConnect = vi.fn();
    render(<ConnectScreen defaults={defaults} onConnect={onConnect} />);
    const host = screen.getByLabelText("Host");
    await user.clear(host);
    await user.type(host, "  desktop-pc  ");
    await user.type(screen.getByLabelText("API token"), "secret");
    await user.click(screen.getByRole("button", { name: "Connect" }));
    expect(onConnect).toHaveBeenCalledWith({ host: "desktop-pc", port: "41595", token: "secret" });
  });

  it("has no Back button without a handler", () => {
    render(<ConnectScreen defaults={defaults} onConnect={() => {}} />);
    expect(screen.queryByRole("button", { name: "Back" })).not.toBeInTheDocument();
  });

  it("goes back without connecting", async () => {
    const user = userEvent.setup();
    const onConnect = vi.fn();
    const onBack = vi.fn();
    render(<ConnectScreen defaults={defaults} onConnect={onConnect} onBack={onBack} />);
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(onBack).toHaveBeenCalledOnce();
    expect(onConnect).not.toHaveBeenCalled();
  });

  it("shows a status message", () => {
    render(<ConnectScreen defaults={defaults} onConnect={() => {}} status="Not connected" />);
    expect(screen.getByRole("status")).toHaveTextContent("Not connected");
  });
});
