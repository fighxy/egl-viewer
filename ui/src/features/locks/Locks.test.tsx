import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "@/App";
import type { Folder, LibraryQuery } from "@/ipc";
import {
  called,
  fakeLayout,
  folder,
  item,
  LIBRARY,
  mockBackend,
  RECOVERY_PHRASE,
  viewOf,
} from "@/test/backend";

let restoreLayout: () => void;
beforeEach(() => {
  restoreLayout = fakeLayout(800, 600);
});
afterEach(() => {
  restoreLayout();
  localStorage.clear();
});

const PLAIN = folder("F1", "Posters", { imageCount: 2, descendantImageCount: 2 });
const LOCKED = folder("F2", "Taxes", {
  isLocked: true,
  imageCount: null,
  descendantImageCount: null,
  lockLabel: "2026",
  lockColor: "red",
});
const UNLOCKED: Folder = { ...LOCKED, isUnlocked: true, imageCount: 3, descendantImageCount: 3 };

type Handler = (args: Record<string, unknown>) => unknown;

/** An open library whose tree is `current()`, read again on every `folder_tree`. */
function library(current: () => Folder[], overrides: Record<string, Handler> = {}) {
  let nextId = 0;
  return mockBackend({
    startup: () => ({ library: LIBRARY, notice: null }),
    ...viewOf(5),
    open_view: () => {
      nextId += 1;
      return { viewId: nextId, total: 5 };
    },
    item_detail: (args) => item(Number(String(args.id).slice(2))),
    folder_tree: () => current(),
    ...overrides,
  });
}

function lastQuery(calls: ReturnType<typeof mockBackend>): LibraryQuery {
  return called(calls, "open_view").at(-1)?.args.query as LibraryQuery;
}

function lockedSection() {
  return screen.findByRole("region", { name: "Locked folders" });
}

describe("Locked folders", () => {
  it("lists locked folders with their label and asks for that folder's password", async () => {
    let tree = [PLAIN, LOCKED];
    let attempts = 0;
    const calls = library(() => tree, {
      unlock_folder: () => {
        attempts += 1;
        if (attempts === 1) throw { kind: "wrongPassword" };
        tree = [PLAIN, UNLOCKED];
        return null;
      },
    });
    const user = userEvent.setup();
    render(<App />);
    const section = await lockedSection();
    const row = within(section).getByRole("button", { name: /Taxes/ });
    expect(row).toHaveTextContent("2026");
    expect(within(row).getByLabelText("Locked")).toBeInTheDocument();

    await user.click(row);
    const dialog = await screen.findByRole("dialog", { name: "Unlock “Taxes”" });
    await user.type(within(dialog).getByLabelText("Password"), "nope{Enter}");
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "The password is not right.",
    );
    const password = within(dialog).getByLabelText("Password");
    await user.clear(password);
    await user.type(password, "secret{Enter}");
    expect(called(calls, "unlock_folder").at(-1)?.args).toEqual({ id: "F2", password: "secret" });
    // The folder opens once unlocked.
    await waitFor(() => expect(lastQuery(calls).scope).toMatchObject({ kind: "folder", id: "F2" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(within(await lockedSection()).getByLabelText("Unlocked")).toBeInTheDocument();
  });

  it("a locked folder in the tree asks for its password instead of opening", async () => {
    const calls = library(() => [PLAIN, LOCKED]);
    const user = userEvent.setup();
    render(<App />);
    const tree = await screen.findByRole("tree", { name: "Folder tree" });
    await user.click(within(tree).getByRole("treeitem", { name: "Taxes" }));
    expect(await screen.findByRole("dialog", { name: "Unlock “Taxes”" })).toBeInTheDocument();
    expect(
      called(calls, "open_view").every(
        (call) => (call.args.query as LibraryQuery).scope.kind !== "folder",
      ),
    ).toBe(true);
  });

  it("locks a folder and shows the recovery key once", async () => {
    let tree = [PLAIN];
    const calls = library(() => tree, {
      lock_folder: () => {
        tree = [{ ...PLAIN, isLocked: true, imageCount: null, descendantImageCount: null }];
        return { key: "ABCD-EFGH-JKMN-PQRS-TVWX-YZ01-2345", phrase: RECOVERY_PHRASE };
      },
    });
    const user = userEvent.setup();
    render(<App />);
    fireEvent.contextMenu(await screen.findByRole("treeitem", { name: "Posters" }));
    await user.click(await screen.findByRole("menuitem", { name: "Lock…" }));
    const dialog = await screen.findByRole("dialog", { name: "Lock “Posters”" });
    expect(dialog).toHaveTextContent("2 items will be encrypted");
    await user.type(within(dialog).getByLabelText("Password"), "secret");
    await user.type(within(dialog).getByLabelText("Repeat the password"), "secreT");
    expect(within(dialog).getByRole("alert")).toHaveTextContent("The passwords are not the same.");
    expect(within(dialog).getByRole("button", { name: "Lock" })).toBeDisabled();
    const repeat = within(dialog).getByLabelText("Repeat the password");
    await user.clear(repeat);
    await user.type(repeat, "secret");
    await user.type(within(dialog).getByLabelText(/Label/), "Work");
    await user.click(within(dialog).getByRole("button", { name: "blue" }));
    await user.click(within(dialog).getByRole("button", { name: "Lock" }));
    expect(called(calls, "lock_folder")[0]?.args).toMatchObject({
      id: "F1",
      password: "secret",
      label: "Work",
      labelColor: "blue",
    });

    const recovery = await screen.findByRole("dialog", { name: "Save the recovery key" });
    const key = within(recovery).getByRole("figure", { name: "Recovery key" });
    expect(key).toHaveTextContent("ABCD-EFGH-JKMN-PQRS-TVWX-YZ01-2345");
    await user.click(within(recovery).getByRole("button", { name: "Show as 12 words" }));
    expect(key).toHaveTextContent(RECOVERY_PHRASE);
    await user.click(within(recovery).getByRole("button", { name: "Save to a file…" }));
    await waitFor(() =>
      expect(called(calls, "save_recovery_file")[0]?.args).toMatchObject({
        path: "C:\\Users\\me\\Documents\\Private recovery key.txt",
        folder: "Posters",
      }),
    );
    // Escape does not close it; Done needs the confirmation.
    await user.keyboard("{Escape}");
    const done = within(recovery).getByRole("button", { name: "Done" });
    expect(done).toBeDisabled();
    await user.click(within(recovery).getByLabelText("I saved the recovery key"));
    await user.click(done);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(
      within(await lockedSection()).getByRole("button", { name: /Posters/ }),
    ).toBeInTheDocument();
  });

  it("opens with the recovery key and sets a new password", async () => {
    const calls = library(() => [PLAIN, LOCKED]);
    const user = userEvent.setup();
    render(<App />);
    await user.click(within(await lockedSection()).getByRole("button", { name: /Taxes/ }));
    const dialog = await screen.findByRole("dialog", { name: "Unlock “Taxes”" });
    await user.click(within(dialog).getByRole("button", { name: "Forgot the password?" }));
    await user.type(within(dialog).getByLabelText("Recovery key or words"), RECOVERY_PHRASE);
    await user.type(within(dialog).getByLabelText("New password"), "fresh");
    await user.type(within(dialog).getByLabelText("Repeat the new password"), "fresh");
    await user.click(within(dialog).getByRole("button", { name: "Unlock" }));
    expect(called(calls, "recover_folder")[0]?.args).toEqual({
      id: "F2",
      recovery: RECOVERY_PHRASE,
      newPassword: "fresh",
    });
  });

  it("locks again, changes the password and removes the lock from the folder menu", async () => {
    const calls = library(() => [PLAIN, UNLOCKED]);
    const user = userEvent.setup();
    render(<App />);
    const row = await screen.findByRole("treeitem", { name: "Taxes" });

    fireEvent.contextMenu(row);
    await user.click(await screen.findByRole("menuitem", { name: "Lock Now" }));
    expect(called(calls, "relock_folder")[0]?.args).toEqual({ id: "F2" });

    fireEvent.contextMenu(row);
    await user.click(await screen.findByRole("menuitem", { name: "Change Password…" }));
    let dialog = await screen.findByRole("dialog", { name: "Change the password of “Taxes”" });
    await user.type(within(dialog).getByLabelText("Current password"), "old");
    await user.type(within(dialog).getByLabelText("New password"), "new");
    await user.type(within(dialog).getByLabelText("Repeat the new password"), "new{Enter}");
    expect(called(calls, "change_folder_password")[0]?.args).toEqual({
      id: "F2",
      current: "old",
      newPassword: "new",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    fireEvent.contextMenu(row);
    await user.click(await screen.findByRole("menuitem", { name: "Remove Lock…" }));
    dialog = await screen.findByRole("dialog", { name: "Remove the lock of “Taxes”?" });
    await user.type(within(dialog).getByLabelText("Password"), "new");
    await user.click(within(dialog).getByRole("button", { name: "Remove lock" }));
    expect(called(calls, "remove_folder_lock")[0]?.args).toMatchObject({
      id: "F2",
      password: "new",
    });
  });

  it("Ctrl+L locks every open folder again", async () => {
    const calls = library(() => [PLAIN, UNLOCKED, folder("F3", "Old", { isLocked: true })]);
    render(<App />);
    await screen.findByRole("treeitem", { name: "Taxes" });
    fireEvent.keyDown(window, { key: "l", ctrlKey: true });
    await waitFor(() => expect(called(calls, "relock_folder")).toHaveLength(1));
    expect(called(calls, "relock_folder")[0]?.args).toEqual({ id: "F2" });
  });

  it("offers a YubiKey touch or a PIN, and still accepts the password", async () => {
    const calls = library(() => [PLAIN, LOCKED], {
      piv_wraps: () => [{ serial: "111", slot: 130, policy: "touch" }],
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(within(await lockedSection()).getByRole("button", { name: /Taxes/ }));
    const dialog = await screen.findByRole("dialog", { name: "Unlock “Taxes”" });
    expect(within(dialog).getByLabelText("Password")).toBeInTheDocument();
    await user.click(await within(dialog).findByRole("button", { name: "Use a YubiKey" }));
    expect(within(dialog).queryByLabelText("YubiKey PIN")).not.toBeInTheDocument();
    expect(within(dialog).getByText("Touch the key when asked.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Unlock" }));
    expect(called(calls, "unlock_with_yubikey")[0]?.args).toEqual({
      id: "F2",
      serial: "111",
      prompt: { kind: "touch" },
    });

    fireEvent.contextMenu(await screen.findByRole("treeitem", { name: "Taxes" }));
    await user.click(await screen.findByRole("menuitem", { name: "Add YubiKey…" }));
    const add = await screen.findByRole("dialog", { name: "Add a YubiKey to “Taxes”" });
    expect(within(add).getByRole("radio", { name: "Touch" })).toBeChecked();
    expect(within(add).queryByLabelText("YubiKey PIN")).not.toBeInTheDocument();
    await user.click(within(add).getByRole("radio", { name: "PIN" }));
    expect(within(add).getByRole("radio", { name: "Touch" })).not.toBeChecked();
    await user.type(within(add).getByLabelText("YubiKey PIN"), "123456");
    await user.type(within(add).getByLabelText("Folder password"), "taxes");
    await user.click(within(add).getByRole("button", { name: "Add YubiKey" }));
    expect(called(calls, "bind_yubikey")[0]?.args).toEqual({
      id: "F2",
      password: "taxes",
      policy: "pin",
      pin: "123456",
    });
  });

  it("a locked folder takes no items until it is unlocked", async () => {
    library(() => [PLAIN, LOCKED]);
    const user = userEvent.setup();
    render(<App />);
    fireEvent.mouseDown(await screen.findByRole("gridcell", { name: "image-0" }));
    const details = screen.getByRole("complementary", { name: "Details" });
    await user.click(await within(details).findByRole("button", { name: "Add" }));
    expect(await screen.findByRole("option", { name: "Taxes" })).toBeDisabled();
    expect(screen.getByRole("option", { name: "Posters" })).toBeEnabled();
  });
});
