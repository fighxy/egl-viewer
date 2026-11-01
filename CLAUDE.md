# CLAUDE.md

Guidance for agents working in this repository.

## Start here

- egl-viewer is a **standalone** asset manager with its own library format (`Name.eglib`).
  Eagle compatibility (import, live connection, export) is secondary (D-013).
- [`docs/spec.md`](docs/spec.md): what the app does, the plan by phases with done checks
  (§9), the tests to write (§10), pitfalls (§11). Everything about Eagle is in §8.
- [`docs/architecture.md`](docs/architecture.md): how it is built. The native library format is
  §15.3, the media pipeline §15.4. Section 1 lists 41 numbered corrections about Eagle's real
  behaviour; the docs refer to them as "correction N".
- [`docs/library-format.md`](docs/library-format.md): the on-disk format of a native library: the
  schema, the lock, opening and closing, adding a file. It is the reference for the format.
- [`docs/decisions.md`](docs/decisions.md): decisions taken while building. Add an entry for
  every decision that the spec or the architecture does not already cover.
- If spec and architecture disagree, the architecture wins and the spec gets fixed.

## Workflow

- Work phase by phase (spec §9). Finish a phase's done check before starting the next one.
- Every change comes with tests. Run `pnpm check` before calling something done.
- Keep the docs in this repository current in the same change: README status table, the
  relevant spec or architecture section, and `docs/decisions.md`.
- New dependencies: the newest version older than pnpm's minimum release age (D-003), pinned
  exactly (D-002). Record the addition in `docs/decisions.md`.
- `ui/src/ipc/generated/` is written by `cargo test`. Never edit it by hand; commit it.
- Eagle's live API on this PC is a real library. Write tests against `fixtures/eagle-api/` and
  `wiremock`; touch a live Eagle only with a separate test library and after asking.

## Hard rules

Native library:

- `library.db` is the single source of truth; no per-item metadata files.
- One writer per library (an OS file lock on `library.lock`); a second instance opens read-only
  and writes nothing.
- Libraries live on local disks: never create one in a cloud-synced folder or on a network path.
- Adding a file: copy into `files/.tmp/` while hashing, flush, record the target in
  `pending_file`, rename into place, then insert the row and clear the journal entry. A row never
  points at a missing file, and no file stays without a row.
- Migrations only forward, always after a backup. Refuse libraries from a newer format version.
- Tags are case-sensitive strings. Never lowercase them.
- The `thumb` scheme and file commands only touch files inside the open library folder.
- File bytes never pass through the WebView; Rust reads files by path.

Eagle:

- Only Rust talks to Eagle. The UI never calls the Eagle API and never sees the token.
- Never write files inside an Eagle `.library` folder. Eagle is the only writer of its library.
- POST bodies to Eagle: the token in the JSON body, never in the query (correction 6); every
  non-ASCII character escaped as `\uXXXX` (correction 7).
- At most 2 concurrent requests to Eagle (correction 10).
- `folder/update` always gets the full desired state; omit empty values, never send `null`
  (corrections 3, 31).
- `item/update` replaces `tags` and `folders`: re-read the item, then send complete arrays.
- `item/add`: a `data:` URL from another computer; the batch form with `path` on the same PC
  (corrections 1, 35).
- Folder passwords never reach models, logs or the UI: drop them while parsing.
- Never log the token or raw bodies of `library/info` and `folder/get`.
- Never send whole item lists over IPC: serve windows of a view.
- Never call `/api/script/inject` or `/api/check`.
- Reading an Eagle `.library` folder (import): retry on JSON parse errors (correction 37); never
  touch Eagle's caches in `%APPDATA%\Eagle` (correction 36).

Forge / releases:

- Never mention Cursor or put Cursor links in commits, PR titles/bodies, comments, release
  notes, or branch names. Forbidden: `cursor.com` URLs, Agent Store / Try Live badges,
  `cursor/` branch prefixes, `CURSOR_AGENT` HTML markers, and “Open in Cursor” / “Open in Web”
  footers. Prefer `feat/…` (and other conventional) branch names. Commits and PRs stay
  authored as Ivan K (`fighxy@users.noreply.github.com`).

## Commands

```bash
pnpm install          # once
pnpm dev              # app with hot reload
pnpm check            # rustfmt, clippy, Rust tests, Biome, TypeScript, UI tests
cargo test --workspace
pnpm test
```

## Conventions

- Rust: edition 2024, `cargo fmt`, clippy with `-D warnings`, `unsafe` forbidden. Pure logic in
  `library-core`; `src-tauri` stays a thin layer of commands.
- UI: React function components, TypeScript strict, Biome for lint and format, shadcn/ui
  components in `ui/src/components/ui/`, imports through `@/`. UI strings in English (D-007).
- IPC: typed wrappers in `ui/src/ipc/index.ts`; types from `ui/src/ipc/generated/`.
