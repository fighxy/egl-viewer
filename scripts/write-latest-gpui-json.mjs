#!/usr/bin/env node
/**
 * Builds the static GPUI updater manifest (`latest-gpui.json`) for the
 * rolling `gpui-latest` prerelease (D-153). Not attached to GitHub Latest.
 *
 * Usage:
 *   node scripts/write-latest-gpui-json.mjs \
 *     --version 0.2.77 \
 *     --notes "GPUI desktop." \
 *     --out latest-gpui.json \
 *     --windows-x86_64 url=https://…/egl-viewer-gpui_0.2.77_windows-x64-setup.exe \
 *     [--linux-x86_64 url=…] [--macos-aarch64 url=…]
 *
 * Signature is optional until minisign is wired (deferred); leave empty.
 */
import { readFileSync, writeFileSync } from "node:fs";

const PLATFORM_FLAGS = new Set([
  "windows-x86_64",
  "linux-x86_64",
  "macos-aarch64",
]);

function usage() {
  console.error(`Usage: node scripts/write-latest-gpui-json.mjs --version <semver> --out <file> [--notes <text>] [--pub-date <rfc3339>]
  --windows-x86_64 url=<url>[,sig=<sig-file>]
  [--linux-x86_64 url=<url>[,sig=<sig-file>]]
  [--macos-aarch64 url=<url>[,sig=<sig-file>]]`);
  process.exit(1);
}

function parsePlatform(raw) {
  const parts = Object.fromEntries(
    raw.split(",").map((piece) => {
      const i = piece.indexOf("=");
      if (i < 0) throw new Error(`expected key=value in ${raw}`);
      return [piece.slice(0, i), piece.slice(i + 1)];
    }),
  );
  if (!parts.url) throw new Error(`url= required in ${raw}`);
  const entry = { url: parts.url, signature: "" };
  if (parts.sig) {
    entry.signature = readFileSync(parts.sig, "utf8").trim();
  }
  return entry;
}

const args = process.argv.slice(2);
const opts = {};
const platforms = {};
for (let i = 0; i < args.length; i += 1) {
  const key = args[i];
  const value = args[i + 1];
  if (!key?.startsWith("--") || value === undefined) usage();
  i += 1;
  const name = key.slice(2);
  if (PLATFORM_FLAGS.has(name)) {
    platforms[name] = parsePlatform(value);
  } else {
    opts[name] = value;
  }
}

if (!opts.version || !opts.out || !platforms["windows-x86_64"]) usage();

const manifest = {
  version: opts.version.replace(/^v/, ""),
  notes: opts.notes ?? "",
  pub_date: opts["pub-date"] ?? new Date().toISOString(),
  platforms,
};

writeFileSync(opts.out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Wrote ${opts.out} with platforms: ${Object.keys(platforms).join(", ")}`);
