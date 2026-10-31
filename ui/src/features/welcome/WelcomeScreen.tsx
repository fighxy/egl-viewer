import { Cable, FolderOpen, FolderPlus, Import, X } from "lucide-react";
import { useTranslation } from "react-i18next";
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
  const { t, i18n } = useTranslation();
  const dateFormat = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: "medium",
    timeStyle: "short",
  });
  return (
    <Card className="w-full max-w-lg">
      <CardHeader>
        <CardTitle>
          <h1>{t("welcome.title")}</h1>
        </CardTitle>
        <CardDescription>{t("welcome.description")}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6 py-4">
        <div className="grid gap-2 sm:grid-cols-2">
          <Button size="lg" onClick={onCreateLibrary}>
            <FolderPlus aria-hidden />
            {t("welcome.create")}
          </Button>
          <Button size="lg" variant="outline" onClick={onOpenLibrary}>
            <FolderOpen aria-hidden />
            {t("welcome.open")}
          </Button>
        </div>
        <section aria-labelledby="recent-heading" className="grid gap-2">
          <h2 id="recent-heading" className="text-sm font-medium">
            {t("welcome.recent")}
          </h2>
          {recent.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("welcome.noRecent")}</p>
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
                          {t("common.missing")}
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
                    aria-label={t("welcome.removeRecent", { name: library.name })}
                    title={t("welcome.removeRecentTitle")}
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
            {t("welcome.eagle")}
          </h2>
          <Button variant="ghost" className="justify-start" onClick={onImportEagleLibrary}>
            <Import aria-hidden />
            {t("welcome.importEagle")}
          </Button>
          <Button variant="ghost" className="justify-start" onClick={onConnectEagle}>
            <Cable aria-hidden />
            {t("welcome.connectEagle")}
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
