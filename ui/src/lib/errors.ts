import type { LocationProblem, SourceError } from "@/ipc";

const LOCATION: Record<LocationProblem, string> = {
  syncFolder:
    "Libraries cannot live in a cloud-synced folder such as OneDrive or Dropbox: syncing a library while it is open damages it. Choose a folder on this computer that is not synced.",
  networkPath: "Libraries cannot live on a network drive. Choose a folder on this computer.",
  insideLibrary: "This folder is inside another library. Choose a folder outside it.",
  insideEagleLibrary:
    "This folder is inside an Eagle library, which only Eagle may change. Choose another folder.",
};

function isSourceError(error: unknown): error is SourceError {
  return typeof error === "object" && error !== null && "kind" in error;
}

/** English text for an error from Rust (D-007). */
export function describeError(error: unknown): string {
  if (!isSourceError(error)) {
    return error instanceof Error ? error.message : String(error);
  }
  switch (error.kind) {
    case "notALibrary":
      return error.detail.eagle
        ? "This is an Eagle library. Importing Eagle libraries comes in a later version."
        : "This folder is not an egl-viewer library.";
    case "newerFormat":
      return `This library was made by a newer version of egl-viewer (format ${error.detail.version}). Update the app to open it.`;
    case "unsupportedLocation":
      return LOCATION[error.detail];
    case "alreadyExists":
      return "A library with this name already exists in this folder.";
    case "notFound":
      return "The folder does not exist.";
    case "readOnly":
      return "This library is read-only.";
    case "invalid":
    case "notSupported":
      return error.detail;
    case "storage":
      return `Something went wrong on disk: ${error.detail}`;
    case "wrongPassword":
      return "The password or recovery key is not right.";
    case "locked":
      return `Unlock “${error.detail.folder}” first.`;
    default:
      return "Something went wrong.";
  }
}
