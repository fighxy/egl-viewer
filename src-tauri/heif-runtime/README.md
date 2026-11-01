# Windows libheif runtime (D-070)

Staged here by `scripts/bundle-heif-dlls.ps1` before a Windows NSIS build.
`tauri.windows.conf.json` maps `heif.dll`, `libde265.dll`, and the LGPL notices
to the install root so they sit beside `egl-viewer.exe`. LoadLibrary only
searches the exe directory.

Do not commit the `.dll` / `NOTICE-*.txt` files. CI (and local Windows release
builds) populate this folder after:

```text
vcpkg install libheif[core,libde265]:x64-windows
pwsh scripts/bundle-heif-dlls.ps1
```

Decode-only features (`core,libde265`) on purpose: the default `hevc` feature
pulls in GPL x265, which we do not ship.
