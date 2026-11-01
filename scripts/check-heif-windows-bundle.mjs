/**
 * Guardrail for D-070: Windows NSIS must map heif.dll + libde265.dll (and LGPL
 * notices) to the install root beside egl-viewer.exe. Runs on every OS in CI
 * via `node scripts/check-heif-windows-bundle.mjs`.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const confPath = join(root, "src-tauri", "tauri.windows.conf.json");
const conf = JSON.parse(readFileSync(confPath, "utf8"));
const resources = conf?.bundle?.resources;
if (!resources || typeof resources !== "object" || Array.isArray(resources)) {
  console.error("tauri.windows.conf.json: bundle.resources must be a path map");
  process.exit(1);
}

const required = {
  "heif-runtime/heif.dll": "heif.dll",
  "heif-runtime/libde265.dll": "libde265.dll",
  "heif-runtime/NOTICE-libheif.txt": "NOTICE-libheif.txt",
  "heif-runtime/NOTICE-libde265.txt": "NOTICE-libde265.txt",
};

let failed = false;
for (const [src, dest] of Object.entries(required)) {
  if (resources[src] !== dest) {
    console.error(`missing or wrong resource mapping: ${src} → ${dest} (got ${resources[src]})`);
    failed = true;
  }
  if (dest.includes("/") || dest.includes("\\")) {
    console.error(`${src} must map to the install root (beside the exe), not ${dest}`);
    failed = true;
  }
}

if (failed) process.exit(1);
console.log("heif Windows bundle mappings ok:", Object.keys(required).join(", "));
