# Stages a portable Windows layout for the GPUI desktop host (D-133).
# Ships gpui-desktop.exe + heif.dll / libde265.dll (+ LGPL notices) in one folder,
# then optionally zips it. Full NSIS installer, Authenticode, and .eglib file
# association stay deferred until Ivan confirms cutover.
#
# Usage (repo root, after libheif is installed via vcpkg):
#   pwsh scripts/package-gpui-desktop.ps1
#   pwsh scripts/package-gpui-desktop.ps1 -SkipBuild
#   pwsh scripts/package-gpui-desktop.ps1 -OutDir dist/gpui-desktop -NoZip

param(
    [string]$OutDir = "dist/gpui-desktop",
    [string]$VcpkgRoot = $(if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } elseif ($env:VCPKG_INSTALLATION_ROOT) { $env:VCPKG_INSTALLATION_ROOT } else { "C:\vcpkg" }),
    [switch]$SkipBuild,
    [switch]$NoZip
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repo

if (-not $SkipBuild) {
    Write-Host "Building gpui-desktop (release)…"
    cargo build -p gpui-desktop --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build -p gpui-desktop --release failed" }
}

$exeCandidates = @(
    (Join-Path $repo "target\release\gpui-desktop.exe"),
    (Join-Path $repo "crates\gpui-desktop\target\release\gpui-desktop.exe")
)
$exe = $exeCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $exe) {
    throw "gpui-desktop.exe not found under target/release. Build first or omit -SkipBuild."
}

$stage = Join-Path $repo $OutDir
if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $stage | Out-Null

Copy-Item -LiteralPath $exe -Destination (Join-Path $stage "gpui-desktop.exe") -Force

# Stage libheif runtime beside the exe (D-070). Prefer the shared bundle script when
# the host can parse its UTF-8 source (pwsh / CI); otherwise copy the two DLLs here.
$heifBin = Join-Path $VcpkgRoot "installed\x64-windows\bin"
$bundleScript = Join-Path $repo "scripts\bundle-heif-dlls.ps1"
$heifStage = Join-Path $stage "_heif-stage"
$bundled = $false
$pwshCmd = Get-Command pwsh -ErrorAction SilentlyContinue
if ($pwshCmd) {
    & $pwshCmd.Source -NoProfile -File $bundleScript -Dest $heifStage -VcpkgRoot $VcpkgRoot
    if ($LASTEXITCODE -eq 0) { $bundled = $true }
} else {
    try {
        & $bundleScript -Dest $heifStage -VcpkgRoot $VcpkgRoot
        $bundled = $true
    } catch {
        Write-Warning "bundle-heif-dlls.ps1 failed ($($_.Exception.Message)); copying DLLs directly."
    }
}
if ($bundled) {
    foreach ($name in @("heif.dll", "libde265.dll", "NOTICE-libheif.txt", "NOTICE-libde265.txt", "bundled-dlls.txt")) {
        $src = Join-Path $heifStage $name
        if (Test-Path -LiteralPath $src) {
            Copy-Item -LiteralPath $src -Destination (Join-Path $stage $name) -Force
        }
    }
    Remove-Item -LiteralPath $heifStage -Recurse -Force -ErrorAction SilentlyContinue
} else {
    foreach ($name in @("heif.dll", "libde265.dll")) {
        $src = Join-Path $heifBin $name
        if (-not (Test-Path -LiteralPath $src)) {
            throw "$name not found under $heifBin. Install with: vcpkg install libheif[core,libde265]:x64-windows"
        }
        Copy-Item -LiteralPath $src -Destination (Join-Path $stage $name) -Force
    }
    @("heif.dll", "libde265.dll") -join "`n" | Set-Content -LiteralPath (Join-Path $stage "bundled-dlls.txt") -Encoding utf8
    Set-Content -LiteralPath (Join-Path $stage "NOTICE-libheif.txt") -Value "libheif - LGPL-3.0-only. See https://github.com/strukturag/libheif" -Encoding utf8
    Set-Content -LiteralPath (Join-Path $stage "NOTICE-libde265.txt") -Value "libde265 - LGPL. See https://github.com/strukturag/libde265" -Encoding utf8
}

$icon = Join-Path $repo "src-tauri\icons\icon.ico"
if (Test-Path -LiteralPath $icon) {
    Copy-Item -LiteralPath $icon -Destination (Join-Path $stage "icon.ico") -Force
}

$licenses = Join-Path $repo "crates\gpui-desktop\assets\THIRD_PARTY_LICENSES.txt"
if (Test-Path -LiteralPath $licenses) {
    Copy-Item -LiteralPath $licenses -Destination (Join-Path $stage "THIRD_PARTY_LICENSES.txt") -Force
}

$readme = @"
egl-viewer GPUI desktop (portable)

Run gpui-desktop.exe. Keep heif.dll and libde265.dll beside the exe (D-070 / D-133).

This layout is the dual-path packaging slice while Tauri/NSIS remains the shipping
Windows installer. A dedicated GPUI NSIS installer and Authenticode signing are deferred.
"@
Set-Content -LiteralPath (Join-Path $stage "README.txt") -Value $readme.Trim() -Encoding utf8

Write-Host "Staged portable layout at $stage"
Get-ChildItem -LiteralPath $stage | ForEach-Object { Write-Host ("  {0}" -f $_.Name) }

if (-not $NoZip) {
    $version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages |
        Where-Object { $_.name -eq "gpui-desktop" } |
        Select-Object -ExpandProperty version -First 1
    if (-not $version) { $version = "0.0.0" }
    $zipName = "egl-viewer-gpui_${version}_x64.zip"
    $zipDir = Split-Path -Parent $stage
    if (-not (Test-Path -LiteralPath $zipDir)) {
        New-Item -ItemType Directory -Force -Path $zipDir | Out-Null
    }
    $zipPath = Join-Path $zipDir $zipName
    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zipPath -CompressionLevel Optimal
    Write-Host "Wrote $zipPath"
}
