#!/usr/bin/env node
/**
 * Builds the static updater manifest (`latest.json`) for GitHub Releases.
 *
 * Usage:
 *   node scripts/write-latest-json.mjs \
 *     --version 0.1.5 \
 *     --notes "Signed desktop updates." \
 *     --out latest.json \
 *     --windows-x86_64 url=https://.../setup.exe,sig=path/to/setup.exe.sig \
 *     --darwin-aarch64 url=https://.../app.tar.gz,sig=path/to/app.tar.gz.sig \
 *     --linux-x86_64 url=https://.../AppImage,sig=path/to/AppImage.sig
 *
 * Attach the written file to the GitHub Release as `latest.json` so the endpoint
 * https://github.com/fighxy/egl-viewer/releases/latest/download/latest.json works (D-062).
 */
import { readFileSync, writeFileSync } from "node:fs";

function usage() {
  console.error(`Usage: node scripts/write-latest-json.mjs --version <semver> --out <file> [--notes <text>] [--pub-date <rfc3339>]
  [--windows-x86_64 url=<url>,sig=<sig-file>]
  [--darwin-aarch64 url=<url>,sig=<sig-file>]
  [--darwin-x86_64 url=<url>,sig=<sig-file>]
  [--linux-x86_64 url=<url>,sig=<sig-file>]`);
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
  if (!parts.url || !parts.sig) throw new Error(`url= and sig= required in ${raw}`);
  return {
    url: parts.url,
    signature: readFileSync(parts.sig, "utf8").trim(),
  };
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
  if (
    name === "windows-x86_64" ||
    name === "darwin-aarch64" ||
    name === "darwin-x86_64" ||
    name === "linux-x86_64"
  ) {
    platforms[name] = parsePlatform(value);
  } else {
    opts[name] = value;
  }
}

if (!opts.version || !opts.out || Object.keys(platforms).length === 0) usage();

const manifest = {
  version: opts.version.replace(/^v/, ""),
  notes: opts.notes ?? "",
  pub_date: opts["pub-date"] ?? new Date().toISOString(),
  platforms,
};

writeFileSync(opts.out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Wrote ${opts.out} with platforms: ${Object.keys(platforms).join(", ")}`);
