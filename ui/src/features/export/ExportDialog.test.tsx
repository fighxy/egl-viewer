import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLocale } from "@/i18n";
import { DEFAULT_PREFERENCES } from "@/ipc";
import { LIBRARY } from "@/test/backend";
import { ExportDialog } from "./ExportDialog";

function renderBackup(locale: "en" | "ru" | "zh-CN" = "en") {
  setAppLocale(locale);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <ExportDialog
        open
        onOpenChange={vi.fn()}
        library={LIBRARY}
        target={null}
        folders={[]}
        vocabulary={[]}
        preferences={DEFAULT_PREFERENCES}
        onPreferences={vi.fn()}
      />
    </QueryClientProvider>,
  );
}

describe("ExportDialog", () => {
  beforeEach(() => {
    setAppLocale("en");
  });

  it("shows English Backup labels by default", () => {
    renderBackup("en");
    expect(screen.getByRole("heading", { name: "Backup" })).toBeInTheDocument();
    expect(screen.getByText("What")).toBeInTheDocument();
    expect(screen.getByText("Entire library")).toBeInTheDocument();
    expect(screen.getByText("Folders")).toBeInTheDocument();
    expect(screen.getByText("Tags")).toBeInTheDocument();
    // Entire library is selected; outside Tauri there is no preview → empty hint.
    expect(screen.getByText("Nothing to export.")).toBeInTheDocument();
    expect(screen.getByText("Format")).toBeInTheDocument();
    expect(screen.getByText("Files in folders")).toBeInTheDocument();
    expect(screen.getByText("ZIP archive")).toBeInTheDocument();
    expect(screen.getByText("Where")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("A folder on this PC")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Browse/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Back up/ })).toBeInTheDocument();
  });

  it("localizes the Backup dialog under Russian", () => {
    renderBackup("ru");
    expect(screen.getByRole("heading", { name: "Резервная копия" })).toBeInTheDocument();
    expect(screen.getByText("Что")).toBeInTheDocument();
    expect(screen.getByText("Вся библиотека")).toBeInTheDocument();
    expect(screen.getByText("Папки")).toBeInTheDocument();
    expect(screen.getByText("Теги")).toBeInTheDocument();
    expect(screen.getByText("Нечего экспортировать.")).toBeInTheDocument();
    expect(screen.getByText("Формат")).toBeInTheDocument();
    expect(screen.getByText("Файлы в папках")).toBeInTheDocument();
    expect(screen.getByText("ZIP-архив")).toBeInTheDocument();
    expect(screen.getByText("Куда")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Папка на этом ПК")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Обзор/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Отмена" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Создать копию/ })).toBeInTheDocument();
  });
});
