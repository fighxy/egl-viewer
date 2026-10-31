// Thin wrappers over the Tauri updater and process plugins so tests can mock one module.
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { inTauri } from "@/ipc";

export type AvailableUpdate = {
  currentVersion: string;
  version: string;
  body?: string;
  date?: string;
};

export type UpdateProgress = {
  downloaded: number;
  contentLength: number | null;
};

/** The Update resource from the last successful check; cleared on install, dismiss, or a new check. */
let pending: Update | null = null;

async function clearPending() {
  if (!pending) return;
  const previous = pending;
  pending = null;
  try {
    await previous.close();
  } catch {
    // The resource may already be gone after install.
  }
}

/** Asks the configured endpoints whether a newer signed build exists. */
export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  if (!inTauri()) return null;
  await clearPending();
  const update = await check();
  if (!update) return null;
  pending = update;
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    body: update.body,
    date: update.date,
  };
}

/** Drops a pending update the user declined without installing. */
export async function dismissPendingUpdate(): Promise<void> {
  await clearPending();
}

/** Downloads and installs the pending update, then relaunches (macOS/Linux; Windows exits on install). */
export async function installAndRelaunch(
  onProgress?: (progress: UpdateProgress) => void,
): Promise<void> {
  if (!inTauri()) {
    throw new Error("Updates are only available in the desktop app.");
  }
  const update = pending;
  if (!update) {
    throw new Error("No update is available.");
  }
  let downloaded = 0;
  let contentLength: number | null = null;
  try {
    await update.downloadAndInstall((event: DownloadEvent) => {
      switch (event.event) {
        case "Started":
          contentLength = event.data.contentLength ?? null;
          downloaded = 0;
          onProgress?.({ downloaded, contentLength });
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          onProgress?.({ downloaded, contentLength });
          break;
        case "Finished":
          onProgress?.({ downloaded, contentLength });
          break;
      }
    });
  } finally {
    pending = null;
  }
  await relaunch();
}

/** Formats a short progress line for the status / dialog. */
export function progressLabel(progress: UpdateProgress): string {
  if (progress.contentLength && progress.contentLength > 0) {
    const pct = Math.min(100, Math.round((100 * progress.downloaded) / progress.contentLength));
    return `Downloading update… ${pct}%`;
  }
  if (progress.downloaded > 0) {
    return `Downloading update… ${formatBytes(progress.downloaded)}`;
  }
  return "Downloading update…";
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
