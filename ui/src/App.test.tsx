import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";

async function openConnect() {
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Connect to a running Eagle" }));
  return user;
}

describe("App", () => {
  it("starts on the Welcome screen", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Welcome to egl-viewer" })).toBeInTheDocument();
    expect(screen.queryByText("Connect to Eagle")).not.toBeInTheDocument();
    expect(screen.getByText("egl-viewer")).toBeInTheDocument();
  });

  it("says that creating a library is not available yet", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: "Create library" }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "Creating a library is not available yet.",
    );
  });

  it("opens the Connect screen with fallback defaults outside Tauri", async () => {
    render(<App />);
    await openConnect();
    expect(screen.getByText("Connect to Eagle")).toBeInTheDocument();
    expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
  });

  it("goes back to Welcome and clears the last message", async () => {
    render(<App />);
    const user = await openConnect();
    await user.click(screen.getByRole("button", { name: "Connect" }));
    expect(screen.getByRole("status")).toHaveTextContent("Connecting is not available yet.");
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("heading", { name: "Welcome to egl-viewer" })).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("takes the version and the defaults from Rust inside Tauri", async () => {
    mockIPC((cmd) => {
      if (cmd === "app_info") return { name: "egl-viewer", version: "9.9.9" };
      if (cmd === "connection_defaults") return { host: "10.0.0.5", port: 41000 };
      return undefined;
    });
    render(<App />);
    expect(await screen.findByText("egl-viewer 9.9.9")).toBeInTheDocument();
    await openConnect();
    expect(await screen.findByDisplayValue("10.0.0.5")).toBeInTheDocument();
    expect(screen.getByLabelText("Port")).toHaveValue("41000");
  });
});
