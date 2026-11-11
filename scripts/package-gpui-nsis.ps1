# Builds the per-user NSIS setup for the GPUI desktop host from the portable stage.
# Requires makensis on PATH (CI installs NSIS via Chocolatey).
#
# Usage (repo root):
#   pwsh scripts/package-gpui-desktop.ps1 -NoZip   # or a full package first
#   pwsh scripts/package-gpui-nsis.ps1
#   pwsh scripts/package-gpui-nsis.ps1 -StageDir dist/gpui-desktop -Version 0.2.77
#
# Authenticode signing is intentionally not performed here (deferred).

param(
    [string]$StageDir = "dist/gpui-desktop",
    [string]$Version = "",
    [string]$OutFile = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repo

$stage = Join-Path $repo $StageDir
if (-not (Test-Path -LiteralPath (Join-Path $stage "gpui-desktop.exe"))) {
    throw "gpui-desktop.exe missing under $stage. Run package-gpui-desktop.ps1 first."
}
foreach ($name in @("heif.dll", "libde265.dll")) {
    if (-not (Test-Path -LiteralPath (Join-Path $stage $name))) {
        throw "$name missing under $stage (DLLs must sit beside the exe)."
    }
}
foreach ($name in @("libx265.dll", "x265.dll")) {
    if (Test-Path -LiteralPath (Join-Path $stage $name)) {
        throw "GPL x265 DLL must not ship in the NSIS payload: $name"
    }
}

if (-not $Version) {
    $Version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages |
        Where-Object { $_.name -eq "gpui-desktop" } |
        Select-Object -ExpandProperty version -First 1
}
if (-not $Version) { throw "could not resolve gpui-desktop version" }

if (-not $OutFile) {
    $OutFile = Join-Path $repo "dist\egl-viewer-gpui_${Version}_windows-x64-setup.exe"
}
$outDir = Split-Path -Parent $OutFile
if (-not (Test-Path -LiteralPath $outDir)) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}
if (Test-Path -LiteralPath $OutFile) {
    Remove-Item -LiteralPath $OutFile -Force
}

$makensis = Get-Command makensis -ErrorAction SilentlyContinue
if (-not $makensis) {
    $candidates = @(
        "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
        "${env:ProgramFiles}\NSIS\makensis.exe"
    )
    foreach ($cand in $candidates) {
        if (Test-Path -LiteralPath $cand) {
            $makensis = Get-Command $cand
            break
        }
    }
}
if (-not $makensis) {
    throw "makensis not found. Install NSIS and put makensis on PATH."
}

$nsi = Join-Path $repo "scripts\installer\egl-viewer-gpui.nsi"
# NSIS wants backslashes in /D paths.
$stageNsis = (Resolve-Path -LiteralPath $stage).Path
$outNsis = $OutFile

Write-Host "Building NSIS setup with $($makensis.Source)…"
& $makensis.Source `
    "/DPRODUCT_VERSION=$Version" `
    "/DSTAGE_DIR=$stageNsis" `
    "/DOUT_FILE=$outNsis" `
    $nsi
if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit $LASTEXITCODE" }
if (-not (Test-Path -LiteralPath $OutFile)) {
    throw "NSIS did not write $OutFile"
}

Write-Host "Wrote $OutFile"
Get-Item -LiteralPath $OutFile | Format-List Name, Length, FullName
