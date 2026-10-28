// How sizes, dimensions and dates read in the grid, the list and the details panel.

export function formatBytes(bytes: number | null): string {
  if (bytes === null) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

export function formatDimensions(width: number | null, height: number | null): string {
  return width !== null && height !== null ? `${width} × ${height}` : "—";
}

export function formatDate(ms: number | null): string {
  if (ms === null) return "—";
  return new Date(ms).toLocaleString("en", { dateStyle: "medium", timeStyle: "short" });
}
