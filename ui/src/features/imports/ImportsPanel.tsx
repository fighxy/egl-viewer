import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import type { ImportProgress } from "@/ipc";
import { importSummary, isRunning, percent, plural } from "./importText";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  imports: ImportProgress[];
  onCancel: (id: number) => void;
};

const timeFormat = new Intl.DateTimeFormat("en", { timeStyle: "short" });

/** Running and finished imports with their problems and skipped duplicates (docs/spec.md §7.4). */
export function ImportsPanel({ open, onOpenChange, imports, onCancel }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Imports</DialogTitle>
          <DialogDescription>Files added in this session.</DialogDescription>
        </DialogHeader>
        {imports.length === 0 ? (
          <p className="text-sm text-muted-foreground">Nothing added yet.</p>
        ) : (
          <ul className="grid max-h-[60vh] gap-3 overflow-y-auto">
            {imports.map((progress) => (
              <li
                key={progress.id}
                aria-label={`Import ${progress.id}`}
                className="grid gap-2 rounded-lg border p-3"
              >
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-medium">
                    {progress.kind === "previews" ? "Previews" : "Adding files"}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {timeFormat.format(progress.startedAt)}
                  </span>
                  {isRunning(progress) && (
                    <Button
                      size="xs"
                      variant="outline"
                      className="ml-auto"
                      onClick={() => onCancel(progress.id)}
                    >
                      Cancel
                    </Button>
                  )}
                </div>
                <Progress value={percent(progress)} aria-label="Progress" />
                <p className="text-[13px]">{importSummary(progress)}</p>
                {progress.failures.length > 0 && (
                  <details className="text-[13px]">
                    <summary className="cursor-default text-destructive">
                      {plural(progress.failed, "problem")}
                    </summary>
                    <ul className="mt-1 grid gap-1">
                      {progress.failures.map((failure) => (
                        <li key={`${failure.path}:${failure.reason}`} className="select-text">
                          <span className="font-medium break-all">{failure.path}</span>
                          <span className="text-muted-foreground"> — {failure.reason}</span>
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
                {progress.skipped.length > 0 && (
                  <details className="text-[13px]">
                    <summary className="cursor-default text-muted-foreground">
                      {plural(progress.duplicates, "duplicate")} skipped
                    </summary>
                    <ul className="mt-1 grid gap-1">
                      {progress.skipped.map((duplicate) => (
                        <li key={duplicate.path} className="break-all select-text">
                          {duplicate.path}
                          {duplicate.inTrash && (
                            <span className="text-muted-foreground"> (in the trash)</span>
                          )}
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
              </li>
            ))}
          </ul>
        )}
      </DialogContent>
    </Dialog>
  );
}
