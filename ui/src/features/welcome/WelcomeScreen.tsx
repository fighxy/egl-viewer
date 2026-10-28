import { Cable, FolderOpen, FolderPlus, Import } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

type Props = {
  onCreateLibrary: () => void;
  onOpenLibrary: () => void;
  onImportEagleLibrary: () => void;
  onConnectEagle: () => void;
  /** A message under the actions, for example the result of the last attempt. */
  status?: string;
};

/** The first screen while no library is open (docs/spec.md §7, D-017). */
export function WelcomeScreen({
  onCreateLibrary,
  onOpenLibrary,
  onImportEagleLibrary,
  onConnectEagle,
  status,
}: Props) {
  return (
    <Card className="w-full max-w-md">
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
          <p className="text-sm text-muted-foreground">No recent libraries yet.</p>
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
