// A fake Rust side for UI tests: every command the UI may call, overridable per test.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { waitFor } from "@testing-library/react";
import { expect } from "vitest";
import {
  DEFAULT_PREFERENCES,
  type Folder,
  type ImportProgress,
  type Item,
  type ItemCell,
  type LibraryInfo,
} from "@/ipc";

export const LIBRARY: LibraryInfo = {
  kind: "native",
  id: "01JLIB",
  name: "Refs",
  path: "C:\\Users\\me\\egl-library\\Refs.eglib",
  modificationTime: null,
  readOnly: false,
  readOnlyReason: null,
  holder: null,
};

/** The 12 words the mocked locks hand out. */
export const RECOVERY_PHRASE =
  "abandon ability able about above absent absorb abstract absurd abuse access accident";

type Handler = (args: Record<string, unknown>) => unknown;
export type Call = { cmd: string; args: Record<string, unknown> };
type Invoke = (cmd: string, args?: unknown, options?: unknown) => Promise<unknown>;

/** Events the UI subscribed to since the last `mockBackend`. */
const heard = new Set<string>();

export function mockBackend(overrides: Record<string, Handler> = {}): Call[] {
  const calls: Call[] = [];
  const handlers: Record<string, Handler> = {
    app_info: () => ({ name: "egl-viewer", version: "9.9.9", commit: "abc1234", webview: "141.0" }),
    connection_defaults: () => ({ host: "127.0.0.1", port: 41595 }),
    get_preferences: () => DEFAULT_PREFERENCES,
    set_preferences: (args) => args.preferences,
    startup: () => ({ library: null, notice: null }),
    recent_libraries: () => [],
    forget_recent_library: () => null,
    default_library_location: () => "C:\\Users\\me\\egl-library",
    create_library: (args) => ({ ...LIBRARY, name: String(args.name) }),
    open_library: () => LIBRARY,
    close_library: () => null,
    library_counts: () => ({ all: 0, uncategorized: 0, untagged: 0, trash: 0 }),
    add_files: () => ({ id: 1 }),
    paste_files: () => ({ id: 1 }),
    answer_import: () => null,
    cancel_import: () => null,
    imports: () => [],
    quit: () => null,
    open_view: () => ({ viewId: 1, total: 0 }),
    view_range: () => [],
    view_ids: () => [],
    close_view: () => null,
    item_detail: () => null,
    folder_tree: () => [],
    save_folder: () => [],
    delete_folder: () => [],
    apply_item_edit: () => null,
    apply_batch_edit: () => 0,
    reorder_items: () => null,
    selection_tags: () => ({ items: 0, tags: [] }),
    tag_vocabulary: () => [],
    tag_groups: () => [],
    save_tag_group: () => [],
    delete_tag_group: () => [],
    rename_tag: () => null,
    merge_tags: () => null,
    delete_tag: () => null,
    lock_folder: () => ({ key: "ABCD-EFGH-JKMN-PQRS-TVWX-YZ01-2345", phrase: RECOVERY_PHRASE }),
    unlock_folder: () => null,
    recover_folder: () => null,
    relock_folder: () => null,
    change_folder_password: () => null,
    renew_recovery_key: () => ({
      key: "NEW0-KEY0-NEW0-KEY0-NEW0-KEY0-NEW0",
      phrase: RECOVERY_PHRASE,
    }),
    remove_folder_lock: () => null,
    set_lock_label: () => null,
    save_recovery_file: () => null,
    "plugin:dialog|save": () => "C:\\Users\\me\\Documents\\Private recovery key.txt",
    "plugin:dialog|open": () => "C:\\Users\\me\\Libraries\\Refs.eglib",
    ...overrides,
  };
  mockWindows("main");
  mockIPC(
    (cmd, args) => {
      const payload = (args ?? {}) as Record<string, unknown>;
      calls.push({ cmd, args: payload });
      if (cmd.startsWith("plugin:window|")) return null;
      const handler = handlers[cmd];
      if (!handler) throw new Error(`unexpected command ${cmd}`);
      return handler(payload);
    },
    { shouldMockEvents: true },
  );
  // The mock adds a listener synchronously inside `invoke`; note which events have one.
  heard.clear();
  const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
    .__TAURI_INTERNALS__;
  const invoke = internals.invoke;
  internals.invoke = (cmd, args, options) => {
    const result = invoke(cmd, args, options);
    if (cmd === "plugin:event|listen") heard.add(String((args as { event: unknown }).event));
    return result;
  };
  return calls;
}

/** Waits until the UI listens to `events`: listeners subscribe asynchronously after a render. */
export async function listening(...events: string[]) {
  await waitFor(() => expect(events.filter((event) => !heard.has(event))).toEqual([]));
}

/** An import's progress as Rust sends it, with defaults for what a test does not care about. */
export function progress(change: Partial<ImportProgress> = {}): ImportProgress {
  return {
    id: 1,
    kind: "files",
    state: "adding",
    total: 3,
    processed: 1,
    added: 1,
    duplicates: 0,
    failed: 0,
    previewsTotal: 1,
    previewsDone: 0,
    current: "a.jpg",
    duplicate: null,
    failures: [],
    skipped: [],
    startedAt: Date.UTC(2026, 9, 28, 10),
    finishedAt: null,
    ...change,
  };
}

export function called(calls: Call[], cmd: string): Call[] {
  return calls.filter((call) => call.cmd === cmd);
}

/** A folder as `folder_tree` returns it. */
export function folder(id: string, name: string, change: Partial<Folder> = {}): Folder {
  return {
    id,
    name,
    description: "",
    iconColor: null,
    tags: [],
    extendTags: [],
    children: [],
    imageCount: 0,
    descendantImageCount: 0,
    isLocked: false,
    isUnlocked: false,
    lockLabel: null,
    lockColor: null,
    coverItemId: null,
    orderBy: null,
    sortAscending: null,
    ...change,
  };
}

/** The cell at position `n` of a fake view. */
export function cell(n: number, change: Partial<ItemCell> = {}): ItemCell {
  return {
    id: `ID${String(n).padStart(4, "0")}`,
    name: `image-${n}`,
    ext: "png",
    width: 40,
    height: 20,
    size: 1000 + n,
    star: 0,
    palette0: [10, 20, 30],
    hasThumbnail: true,
    thumbVersion: 1,
    addedAt: Date.UTC(2026, 9, 28, 10) + n,
    tags: [],
    ...change,
  };
}

/** Handlers for a view of `total` items: windows and ids come from `cell`. */
export function viewOf(total: number, viewId = 1) {
  const slice = (args: Record<string, unknown>) => {
    const start = Number(args.start);
    const end = Math.min(total, start + Number(args.len));
    return Array.from({ length: Math.max(0, end - start) }, (_, offset) => cell(start + offset));
  };
  return {
    open_view: () => ({ viewId, total }),
    view_range: (args: Record<string, unknown>) => slice(args),
    view_ids: (args: Record<string, unknown>) => slice(args).map((item) => item.id),
  };
}

/** The full item behind `cell(n)`. */
export function item(n: number, change: Partial<Item> = {}): Item {
  const base = cell(n);
  return {
    id: base.id,
    name: base.name,
    ext: base.ext,
    width: base.width,
    height: base.height,
    size: base.size,
    star: base.star,
    tags: [],
    folders: [],
    annotation: "",
    url: "",
    palettes: [{ color: [10, 20, 30], ratio: 60 }],
    addedAt: base.addedAt,
    fileCreatedAt: null,
    fileModifiedAt: null,
    duration: null,
    bpm: null,
    exif: null,
    noThumbnail: false,
    thumbVersion: 1,
    isDeleted: false,
    ...change,
  };
}

/**
 * jsdom has no layout: give every element a size, so the virtualized grid shows rows.
 * Returns a function that removes the fake size.
 */
export function fakeLayout(width = 800, height = 600): () => void {
  const proto = HTMLElement.prototype;
  Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => width });
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => height });
  return () => {
    Reflect.deleteProperty(proto, "clientWidth");
    Reflect.deleteProperty(proto, "clientHeight");
  };
}
