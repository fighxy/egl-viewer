# Privacy (egl-viewer for Android)

Local-first asset manager. **No tracking. No analytics. No account.**

## What stays on the phone

- Native libraries (`.eglib`) the user creates or opens: originals, thumbnails, tags, folders,
  ratings, notes and backups live in app storage, or in a folder elsewhere on the phone that
  the user opened after granting **All files access**. That access is asked only when the user
  opens a library outside app storage; the app reads and writes there only that library's
  folder, and declining keeps everything else working.
- App Lock (PIN / password hash and optional biometrics): Keystore / app-private preferences
  on this device only. The recovery key for a locked folder is shown once and never uploaded.
- Export and Decrypt Archive passphrases: typed for that export or archive only; never stored.
  A decrypted archive goes to a temporary folder that is removed when the screen closes,
  unless the user saves or shares it.

## What the app does not do

- No advertising identifiers, crash reporters or analytics SDKs.
- No network calls to egl-viewer servers. The Android client does not talk to Eagle.
- Photos and files are read through the system Gallery / Files pickers; the app does not get
  broad media access beyond what the user picks for an import.

## Backups and transfers

Android Auto Backup and device-to-device transfer exclude the app login, recent-library list,
saved views and library files (`data_extraction_rules.xml` / `backup_rules.xml`). Copy a
library with the in-app export or by copying its `.eglib` folder.

## Store listing

Play Console Data safety: no data collected, no data shared. Approximate location, personal
info and photos are **not** collected by the app; photos the user adds become part of their
on-device library only.
