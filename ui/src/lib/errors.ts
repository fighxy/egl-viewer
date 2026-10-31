import type { LocationProblem, SourceError } from "@/ipc";
import i18n from "@/i18n";

const LOCATION_KEYS: Record<LocationProblem, string> = {
  syncFolder: "errors.syncFolder",
  networkPath: "errors.networkPath",
  insideLibrary: "errors.insideLibrary",
  insideEagleLibrary: "errors.insideEagleLibrary",
};

function isSourceError(error: unknown): error is SourceError {
  return typeof error === "object" && error !== null && "kind" in error;
}

/** Localized text for an error from Rust (D-007, D-069). */
export function describeError(error: unknown): string {
  if (!isSourceError(error)) {
    return error instanceof Error ? error.message : String(error);
  }
  switch (error.kind) {
    case "notALibrary":
      return error.detail.eagle ? i18n.t("errors.notALibraryEagle") : i18n.t("errors.notALibrary");
    case "newerFormat":
      return i18n.t("errors.newerFormat", { version: error.detail.version });
    case "unsupportedLocation":
      return i18n.t(LOCATION_KEYS[error.detail]);
    case "alreadyExists":
      return i18n.t("errors.alreadyExists");
    case "notFound":
      return i18n.t("errors.notFound");
    case "readOnly":
      return i18n.t("errors.readOnly");
    case "invalid":
    case "notSupported":
      return error.detail;
    case "storage":
      return i18n.t("errors.storage", { detail: error.detail });
    case "wrongPassword":
      return i18n.t("errors.wrongPassword");
    case "locked":
      return i18n.t("errors.locked", { folder: error.detail.folder });
    default:
      return i18n.t("errors.generic");
  }
}
