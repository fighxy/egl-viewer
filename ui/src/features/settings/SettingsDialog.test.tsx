import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES } from "@/ipc";
import { SettingsDialog } from "./SettingsDialog";

function renderSettings(save = vi.fn().mockResolvedValue(undefined)) {
  const props = {
    onOpenChange: vi.fn(),
    preferences: DEFAULT_PREFERENCES,
    builtInLocation: "C:\\Users\\me\\egl-library",
    browse: vi.fn().mockResolvedValue("D:\\Libraries"),
    save,
    login: "none" as const,
    onSetPassword: vi.fn().mockResolvedValue(undefined),
    onSetPin: vi.fn().mockResolvedValue(undefined),
    onSetWebAuthn: vi.fn().mockResolvedValue(undefined),
    onClearLogin: vi.fn().mockResolvedValue(undefined),
  };
  render(<SettingsDialog open {...props} />);
  return props;
}

describe("SettingsDialog", () => {
  it("saves the theme, the reopen switch and the default folder", async () => {
    const user = userEvent.setup();
    const props = renderSettings();
    expect(screen.getByRole("radio", { name: "System" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Dark" }));
    await user.click(screen.getByRole("switch", { name: "Reopen the last library on start" }));
    await user.click(screen.getByRole("button", { name: "Browse…" }));
    expect(await screen.findByDisplayValue("D:\\Libraries")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(props.save).toHaveBeenCalledWith({
      ...DEFAULT_PREFERENCES,
      theme: "dark",
      reopenLastLibrary: false,
      defaultLibraryParent: "D:\\Libraries",
    });
    expect(props.onOpenChange).toHaveBeenCalledWith(false);
  });

  it("saves the chosen UI language", async () => {
    const user = userEvent.setup();
    const props = renderSettings();
    expect(screen.getByRole("radio", { name: "English" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Русский" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(props.save).toHaveBeenCalledWith({
      ...DEFAULT_PREFERENCES,
      locale: "ru",
    });
  });

  it("an empty default folder means the built-in one", async () => {
    const user = userEvent.setup();
    const props = renderSettings();
    expect(screen.getByLabelText("Default folder for new libraries")).toHaveAttribute(
      "placeholder",
      "C:\\Users\\me\\egl-library",
    );
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(props.save).toHaveBeenCalledWith(DEFAULT_PREFERENCES);
  });

  it("shows why a folder cannot be the default", async () => {
    const user = userEvent.setup();
    const props = renderSettings(
      vi.fn().mockRejectedValue({ kind: "unsupportedLocation", detail: "networkPath" }),
    );
    await user.type(screen.getByLabelText("Default folder for new libraries"), "\\\\nas\\libs");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("network drive");
    expect(props.onOpenChange).not.toHaveBeenCalledWith(false);
  });

  it("keeps app login to one method and does not ask a YubiKey for a PIN", async () => {
    const user = userEvent.setup();
    const props = renderSettings();
    expect(screen.getByRole("radio", { name: "Off" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "YubiKey" }));
    expect(screen.getByRole("radio", { name: "YubiKey" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Password" })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: "PIN" })).not.toBeChecked();
    expect(screen.queryByLabelText("Login PIN")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Login password")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Set login" }));
    expect(props.onSetWebAuthn).toHaveBeenCalledOnce();
    expect(props.onSetPin).not.toHaveBeenCalled();
    expect(props.onSetPassword).not.toHaveBeenCalled();
  });
});
