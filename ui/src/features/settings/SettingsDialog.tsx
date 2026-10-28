import { cn } from "cn";
import { type FormEvent, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import type { Preferences, Theme } from "@/ipc";
import { describeError } from "@/lib/errors";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  preferences: Preferences;
  /** `%USERPROFILE%\egl-library`, shown when no other default folder is set. */
  builtInLocation: string;
  browse: (current: string) => Promise<string | null>;
  /** Rejects with the reason when the default folder cannot hold libraries. */
  save: (preferences: Preferences) => Promise<void>;
};

const THEMES: readonly { value: Theme; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

/** Settings → General (docs/spec.md §7.4, D-027). */
export function SettingsDialog({
  open,
  onOpenChange,
  preferences,
  builtInLocation,
  browse,
  save,
}: Props) {
  const [draft, setDraft] = useState(preferences);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setDraft(preferences);
      setError(undefined);
    }
  }, [open, preferences]);

  const folder = draft.defaultLibraryParent ?? "";

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      await save({ ...draft, defaultLibraryParent: folder.trim() || null });
      onOpenChange(false);
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  const pick = async () => {
    const picked = await browse(folder || builtInLocation).catch(() => null);
    if (picked) setDraft({ ...draft, defaultLibraryParent: picked });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <form className="grid gap-5" onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>General</DialogDescription>
          </DialogHeader>
          <fieldset className="grid gap-1.5">
            <legend className="mb-1.5 text-sm font-medium">Theme</legend>
            <div className="flex gap-1">
              {THEMES.map(({ value, label }) => (
                <label
                  key={value}
                  className={cn(
                    "rounded-md border px-3 py-1 text-[13px] has-focus-visible:ring-3 has-focus-visible:ring-ring/50",
                    draft.theme === value
                      ? "border-primary bg-primary text-primary-foreground"
                      : "hover:bg-muted",
                  )}
                >
                  <input
                    type="radio"
                    name="theme"
                    value={value}
                    className="sr-only"
                    checked={draft.theme === value}
                    onChange={() => setDraft({ ...draft, theme: value })}
                  />
                  {label}
                </label>
              ))}
            </div>
          </fieldset>
          <div className="flex items-center justify-between gap-4">
            <Label htmlFor="reopen-last-library">Reopen the last library on start</Label>
            <Switch
              id="reopen-last-library"
              checked={draft.reopenLastLibrary}
              onCheckedChange={(checked) => setDraft({ ...draft, reopenLastLibrary: checked })}
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="default-folder">Default folder for new libraries</Label>
            <div className="flex gap-2">
              <Input
                id="default-folder"
                value={folder}
                placeholder={builtInLocation}
                onChange={(event) =>
                  setDraft({ ...draft, defaultLibraryParent: event.target.value })
                }
              />
              <Button type="button" variant="outline" onClick={pick}>
                Browse…
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Leave empty to use {builtInLocation}. Changing it does not move existing libraries.
            </p>
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
