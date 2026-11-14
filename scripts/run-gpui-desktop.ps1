# Launch gpui-desktop with the vcpkg heif runtime on PATH (D-134).
# Usage (repo root):
#   pwsh scripts/run-gpui-desktop.ps1
#   pwsh scripts/run-gpui-desktop.ps1 -- D:\RSPECT.eglib
#   pwsh scripts/run-gpui-desktop.ps1 -Release -- --no-reopen

param(
    [switch]$Release,
    [string]$VcpkgRoot = $(if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } elseif ($env:VCPKG_INSTALLATION_ROOT) { $env:VCPKG_INSTALLATION_ROOT } elseif (Test-Path "C:\Users\kirkw\vcpkg") { "C:\Users\kirkw\vcpkg" } else { "C:\vcpkg" }),
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repo

$bin = Join-Path $VcpkgRoot "installed\x64-windows\bin"
if (-not (Test-Path -LiteralPath (Join-Path $bin "heif.dll"))) {
    throw "heif.dll not found under $bin. Set VCPKG_ROOT or install libheif[core,libde265]:x64-windows."
}

if (-not (Get-ChildItem -LiteralPath $bin -Filter "avcodec-*.dll" -ErrorAction SilentlyContinue)) {
    throw "FFmpeg DLLs not found under $bin. Install: vcpkg install `"ffmpeg[core,avcodec,avformat,avfilter,swresample,swscale,dav1d,zlib]:x64-windows`" (D-168)."
}
# ffmpeg-sys-next generates its bindings with bindgen, which needs libclang (LLVM).
if (-not $env:LIBCLANG_PATH -and (Test-Path -LiteralPath "C:\Program Files\LLVM\bin\libclang.dll")) {
    $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
}

$env:VCPKG_ROOT = $VcpkgRoot
$env:VCPKGRS_DYNAMIC = "1"
$env:PATH = "$bin;$env:PATH"
$env:RUST_BACKTRACE = $(if ($env:RUST_BACKTRACE) { $env:RUST_BACKTRACE } else { "full" })

$profile = if ($Release) { "release" } else { "debug" }
Write-Host "VCPKG bin on PATH: $bin"
Write-Host "cargo run -p gpui-desktop ($profile) $($CargoArgs -join ' ')"

$cargo = @("run", "-p", "gpui-desktop")
if ($Release) { $cargo += "--release" }
if ($CargoArgs -and $CargoArgs.Count -gt 0) {
    $cargo += "--"
    $cargo += $CargoArgs
}
& cargo @cargo
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
