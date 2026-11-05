import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLocale } from "@/i18n";
import { called, mockBackend } from "@/test/backend";
import { DecryptArchiveDialog } from "./DecryptArchiveDialog";

describe("DecryptArchiveDialog", () => {
  beforeEach(() => setAppLocale("en"));

  it("picks an archive, saves next to it by default and sends the password once", async () => {
    const calls = mockBackend({
      "plugin:dialog|open": () => "/home/me/Exports/Library.zip.age",
    });
    const user = userEvent.setup();
    render(<DecryptArchiveDialog open onOpenChange={vi.fn()} />);
    expect(screen.getByRole("heading", { name: "Decrypt Archive" })).toBeInTheDocument();

    const [chooseArchive] = screen.getAllByRole("button", { name: "Choose…" });
    if (!chooseArchive) throw new Error("no Choose button");
    await user.click(chooseArchive);
    await waitFor(() =>
      expect(screen.getByLabelText("Archive")).toHaveValue("/home/me/Exports/Library.zip.age"),
    );
    expect(screen.getByLabelText("Save in")).toHaveValue("/home/me/Exports");

    await user.type(screen.getByLabelText("Password"), "correct horse");
    await user.click(screen.getByRole("button", { name: "Decrypt" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Decrypted to Library.zip");
    const sent = called(calls, "decrypt_archive");
    expect(sent).toHaveLength(1);
    expect(sent[0]?.args).toMatchObject({
      source: "/home/me/Exports/Library.zip.age",
      destination: "/home/me/Exports",
      passphrase: "correct horse",
    });
    // The password does not stay in the form.
    expect(screen.getByLabelText("Password")).toHaveValue("");
    await user.click(screen.getByRole("button", { name: "Show in Explorer" }));
    await waitFor(() => expect(called(calls, "reveal_path")).toHaveLength(1));
  });

  it("asks for what is missing and shows what Rust refused", async () => {
    const calls = mockBackend({
      decrypt_archive: () => {
        throw { kind: "invalid", detail: "Wrong passphrase." };
      },
    });
    const user = userEvent.setup();
    render(<DecryptArchiveDialog open onOpenChange={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Decrypt" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Choose the archive.");
    expect(called(calls, "decrypt_archive")).toEqual([]);

    await user.type(screen.getByLabelText("Archive"), "/tmp/a.zip.age");
    await user.type(screen.getByLabelText("Save in"), "/tmp");
    await user.type(screen.getByLabelText("Password"), "nope nope");
    await user.click(screen.getByRole("button", { name: "Decrypt" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Wrong passphrase.");
  });

  it("shows progress, cancels a running decryption and says nothing was left", async () => {
    let finish: (value: null) => void = () => {};
    const calls = mockBackend({
      decrypt_archive: () => new Promise<null>((resolve) => (finish = resolve)),
      cancel_decrypt: () => {
        finish(null);
        return null;
      },
    });
    const user = userEvent.setup();
    render(<DecryptArchiveDialog open onOpenChange={vi.fn()} />);
    await user.type(screen.getByLabelText("Archive"), "/tmp/big.zip.age");
    await user.type(screen.getByLabelText("Save in"), "/tmp");
    await user.type(screen.getByLabelText("Password"), "correct horse");
    await user.click(screen.getByRole("button", { name: "Decrypt" }));
    expect(await screen.findByRole("progressbar")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "The decryption was cancelled; nothing was left behind.",
    );
    expect(called(calls, "cancel_decrypt")).toHaveLength(1);
    // Done: the footer offers Close again instead of Cancel.
    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
  });

  it("speaks Russian", () => {
    mockBackend();
    setAppLocale("ru");
    render(<DecryptArchiveDialog open onOpenChange={vi.fn()} />);
    expect(screen.getByRole("heading", { name: "Расшифровать архив" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Расшифровать" })).toBeInTheDocument();
  });
});
