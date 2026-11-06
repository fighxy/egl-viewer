import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { emit } from "@tauri-apps/api/event";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setAppLocale } from "@/i18n";
import { DEFAULT_PREFERENCES, type ExportProgress, type Preferences } from "@/ipc";
import { called, LIBRARY, listening, mockBackend } from "@/test/backend";
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

  it("refuses a password shorter than eight characters before starting (D-110)", async () => {
    const calls = mockBackend({
      export_preview: () => ({ items: 2, bytes: 2048, hidden: 0 }),
      exports: () => [],
    });
    const user = userEvent.setup();
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
          preferences={{ ...DEFAULT_PREFERENCES, exportDestination: "D:\\Backups" }}
          onPreferences={vi.fn()}
        />
      </QueryClientProvider>,
    );
    await user.click(screen.getByLabelText("Encrypt with a password"));
    await user.type(screen.getByLabelText("Password"), "short");
    await user.type(screen.getByLabelText("Repeat the password"), "short");
    // The button says why it waits, like the other reasons it cannot start.
    const backUp = screen.getByRole("button", { name: /Back up/ });
    await waitFor(() =>
      expect(backUp).toHaveAttribute("title", "The password needs at least 8 characters."),
    );
    expect(backUp).toBeDisabled();
    await user.type(screen.getByLabelText("Password"), "-but-long");
    await user.type(screen.getByLabelText("Repeat the password"), "-but-long");
    await waitFor(() => expect(backUp).toBeEnabled());
    expect(called(calls, "start_export")).toEqual([]);
  });

  it("follows an export from its first event to the result, and Close closes", async () => {
    const step = (change: Partial<ExportProgress>): ExportProgress => ({
      id: 7,
      state: "preparing",
      total: 2,
      done: 0,
      bytesTotal: 2048,
      bytesDone: 0,
      current: null,
      summary: null,
      error: null,
      ...change,
    });
    const calls = mockBackend({
      export_preview: () => ({ items: 2, bytes: 2048, hidden: 0 }),
      exports: () => [],
      // The first event arrives before `start_export` answers: the dialog must not miss it.
      start_export: async () => {
        await emit("export-progress", step({ state: "copying", done: 1, bytesDone: 1024 }));
        return step({});
      },
    });
    const onOpenChange = vi.fn();
    const onPreferences = vi.fn();
    const preferences: Preferences = { ...DEFAULT_PREFERENCES, exportDestination: "D:\\Backups" };
    const user = userEvent.setup();
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <ExportDialog
          open
          onOpenChange={onOpenChange}
          library={LIBRARY}
          target={null}
          folders={[]}
          vocabulary={[]}
          preferences={preferences}
          onPreferences={onPreferences}
        />
      </QueryClientProvider>,
    );
    await listening("export-progress");
    const backUp = screen.getByRole("button", { name: /Back up/ });
    await waitFor(() => expect(backUp).toBeEnabled());
    await user.click(backUp);
    expect(await screen.findByText(/1 \/ 2|1 of 2/)).toBeInTheDocument();
    expect(called(calls, "start_export")[0]?.args.request).toMatchObject({
      destination: "D:\\Backups",
      selection: { whole: true },
    });
    await emit(
      "export-progress",
      step({
        state: "done",
        done: 2,
        bytesDone: 2048,
        summary: { items: 2, files: 3, bytes: 2048, missing: [], path: "D:\\Backups\\Refs.zip" },
      }),
    );
    await user.click(await screen.findByRole("button", { name: "Close" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
  it("sends a ZIP password or a 7z archive as the format says (D-112, D-113)", async () => {
    const first: ExportProgress = {
      id: 1,
      state: "preparing",
      total: 1,
      done: 0,
      bytesTotal: 1,
      bytesDone: 0,
      current: null,
      summary: null,
      error: null,
    };
    for (const [choose, protection, format] of [
      ["ZIP archive", "ZIP password", { kind: "zip", compression: "balanced", encryption: "aes" }],
      ["7z archive", null, { kind: "sevenZip", compression: "balanced" }],
    ] as const) {
      const calls = mockBackend({
        export_preview: () => ({ items: 1, bytes: 1, hidden: 0 }),
        exports: () => [],
        start_export: () => first,
      });
      const user = userEvent.setup();
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      const { unmount } = render(
        <QueryClientProvider client={client}>
          <ExportDialog
            open
            onOpenChange={vi.fn()}
            library={LIBRARY}
            target={null}
            folders={[]}
            vocabulary={[]}
            preferences={{ ...DEFAULT_PREFERENCES, exportDestination: "D:\\Backups" }}
            onPreferences={vi.fn()}
          />
        </QueryClientProvider>,
      );
      await user.click(screen.getByText(choose));
      await user.click(screen.getByLabelText("Encrypt with a password"));
      if (protection) {
        expect(screen.getByText(/Names and contents hidden/)).toBeInTheDocument();
        await user.click(screen.getByText(protection));
        expect(screen.getByText(/File names stay visible/)).toBeInTheDocument();
      } else {
        expect(screen.getByText(/file names hidden too/)).toBeInTheDocument();
      }
      await user.type(screen.getByLabelText("Password"), "correct horse");
      await user.type(screen.getByLabelText("Repeat the password"), "correct horse");
      const backUp = screen.getByRole("button", { name: /Back up/ });
      await waitFor(() => expect(backUp).toBeEnabled());
      await user.click(backUp);
      await waitFor(() => expect(called(calls, "start_export")).toHaveLength(1));
      expect(called(calls, "start_export")[0]?.args.request).toMatchObject({
        format,
        encrypt: "correct horse",
      });
      unmount();
    }
  });
});
