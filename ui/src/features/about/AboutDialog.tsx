import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { AppInfo } from "@/ipc";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  info: AppInfo | null;
};

/** About egl-viewer (docs/spec.md §7.4). Third-party licenses arrive with the release build. */
export function AboutDialog({ open, onOpenChange, info }: Props) {
  const rows: [string, string][] = [
    ["Version", info?.version ?? "unknown"],
    ...(info?.commit ? ([["Build", info.commit]] as [string, string][]) : []),
    ...(info?.webview ? ([["WebView", info.webview]] as [string, string][]) : []),
  ];
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>About egl-viewer</DialogTitle>
          <DialogDescription>
            A desktop app for collections of images and design references.
          </DialogDescription>
        </DialogHeader>
        <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-[13px]">
          {rows.map(([label, value]) => (
            <div key={label} className="contents">
              <dt className="text-muted-foreground">{label}</dt>
              <dd className="select-text">{value}</dd>
            </div>
          ))}
        </dl>
      </DialogContent>
    </Dialog>
  );
}
