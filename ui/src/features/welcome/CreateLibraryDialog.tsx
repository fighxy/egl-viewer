import { type FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
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
import type { LibraryInfo } from "@/ipc";
import { describeError } from "@/lib/errors";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The folder the location starts in. */
  loadDefaultLocation: () => Promise<string>;
  browse: (current: string) => Promise<string | null>;
  create: (parent: string, name: string) => Promise<LibraryInfo>;
  onCreated: (library: LibraryInfo) => void;
};

/** Create library: a name and a location; makes `<location>\<name>.eglib` (docs/spec.md §7.2). */
export function CreateLibraryDialog({
  open,
  onOpenChange,
  loadDefaultLocation,
  browse,
  create,
  onCreated,
}: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [location, setLocation] = useState("");
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName("");
    setError(undefined);
    loadDefaultLocation()
      .then(setLocation)
      .catch(() => setLocation(""));
  }, [open, loadDefaultLocation]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(undefined);
    try {
      onCreated(await create(location.trim(), name));
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  const pick = async () => {
    const picked = await browse(location).catch(() => null);
    if (picked) setLocation(picked);
  };

  const ready = name.trim() !== "" && location.trim() !== "" && !busy;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <form className="grid gap-4" onSubmit={submit} noValidate>
          <DialogHeader>
            <DialogTitle>{t("createLibrary.title")}</DialogTitle>
            <DialogDescription>
              A library is a folder that keeps your files and their tags, notes and ratings.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-1.5">
            <Label htmlFor="library-name">{t("createLibrary.name")}</Label>
            <Input
              id="library-name"
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="My Library"
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="library-location">{t("createLibrary.location")}</Label>
            <div className="flex gap-2">
              <Input
                id="library-location"
                value={location}
                onChange={(event) => setLocation(event.target.value)}
              />
              <Button type="button" variant="outline" onClick={pick}>
                {t("common.browse")}
              </Button>
            </div>
            {name.trim() !== "" && location.trim() !== "" && (
              <p className="truncate text-xs text-muted-foreground">
                Creates {joinPath(location.trim(), `${name.trim()}.eglib`)}
              </p>
            )}
          </div>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!ready}>
              {t("createLibrary.create")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function joinPath(folder: string, name: string) {
  const separator = folder.includes("\\") ? "\\" : "/";
  return folder.endsWith(separator) ? `${folder}${name}` : `${folder}${separator}${name}`;
}
