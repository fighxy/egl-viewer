// URLs of the `thumb` scheme that Rust serves from the open library (docs/architecture.md §11).
// File bytes never pass through IPC: the WebView loads these URLs itself.
import { convertFileSrc } from "@tauri-apps/api/core";

/** `http://thumb.localhost/` on Windows, `thumb://localhost/` on macOS and Linux. */
function base(): string {
  try {
    return convertFileSrc("", "thumb");
  } catch {
    return "thumb://localhost/";
  }
}

/** The thumbnail; the version changes with the thumbnail, so the URL can be cached forever. */
export function thumbUrl(item: { id: string; thumbVersion: number }): string {
  return `${base()}${item.id}?v=${item.thumbVersion}`;
}

export function originalUrl(id: string): string {
  return `${base()}${id}/original`;
}

/** Originals the WebView can show as they are; others get the thumbnail in the preview. */
const SHOWN_AS_IS = new Set(["jpg", "jpeg", "png", "gif", "webp", "avif", "bmp", "svg"]);

export function showsOriginal(ext: string): boolean {
  return SHOWN_AS_IS.has(ext.toLowerCase());
}
