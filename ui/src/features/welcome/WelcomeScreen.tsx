import { Cable, FolderOpen, FolderPlus, Import, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { RecentLibrary } from "@/ipc";

type Props = {
  onCreateLibrary: () => void;
  onOpenLibrary: () => void;
  onImportEagleLibrary: () => void;
  onConnectEagle: () => void;
  recent?: RecentLibrary[];
  onOpenRecent?: (path: string) => void;
  onForgetRecent?: (path: string) => void;
  /** A message under the actions, for example the result of the last attempt. */
  status?: string;
};

const dateFormat = new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" });

/** The first screen while no library is open (docs/spec.md §7.2, D-017). */
export function WelcomeScreen({
  onCreateLibrary,
  onOpenLibrary,
  onImportEagleLibrary,
  onConnectEagle,
  recent = [],
  onOpenRecent,
  onForgetRecent,
  status,
}: Props) {
  return (
    <Card className="w-full max-w-lg">
      <CardHeader>
        <CardTitle>
          <h1>Welcome to egl-viewer</h1>
        </CardTitle>
        <CardDescription>Keep your images and references in a library on this PC.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6 py-4">
        <div className="grid gap-2 sm:grid-cols-2">
          <Button size="lg" onClick={onCreateLibrary}>
            <FolderPlus aria-hidden />
            Create library
          </Button>
          <Button size="lg" variant="outline" onClick={onOpenLibrary}>
            <FolderOpen aria-hidden />
            Open library
          </Button>
        </div>
        <section aria-labelledby="recent-heading" className="grid gap-2">
          <h2 id="recent-heading" className="text-sm font-medium">
            Recent libraries
          </h2>
          {recent.length === 0 ? (
            <p className="text-sm text-muted-foreground">No recent libraries yet.</p>
          ) : (
            <ul className="grid gap-0.5">
              {recent.map((library) => (
                <li key={library.path} className="group flex items-center gap-1">
                  <button
                    type="button"
                    className="grid min-w-0 flex-1 gap-0.5 rounded-md px-2 py-1.5 text-left outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
                    onClick={() => onOpenRecent?.(library.path)}
                    title={library.path}
                  >
                    <span className="flex items-center gap-2 text-sm font-medium">
                      {library.name}
                      {library.missing && (
                        <span className="rounded bg-muted px-1.5 text-xs font-normal text-muted-foreground">
                          Missing
                        </span>
                      )}
                    </span>
                    <span className="truncate text-xs text-muted-foreground">
                      {library.path} · {dateFormat.format(library.lastOpenedAt)}
                    </span>
                  </button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={`Remove ${library.name} from the list`}
                    title="Remove from the list"
                    onClick={() => onForgetRecent?.(library.path)}
                  >
                    <X aria-hidden />
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section aria-labelledby="eagle-heading" className="grid gap-1 border-t pt-4">
          <h2 id="eagle-heading" className="text-sm font-medium">
            Eagle
          </h2>
          <Button variant="ghost" className="justify-start" onClick={onImportEagleLibrary}>
            <Import aria-hidden />
            Import an Eagle library
          </Button>
          <Button variant="ghost" className="justify-start" onClick={onConnectEagle}>
            <Cable aria-hidden />
            Connect to a running Eagle
          </Button>
        </section>
      </CardContent>
      {status && (
        <CardFooter>
          <p role="status" className="text-sm text-muted-foreground">
            {status}
          </p>
        </CardFooter>
      )}
    </Card>
  );
}
