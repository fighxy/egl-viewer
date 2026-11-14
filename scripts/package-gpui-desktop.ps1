# Stages a portable Windows layout for the GPUI desktop host (D-133 / D-153).
# Ships gpui-desktop.exe + heif.dll / libde265.dll + the FFmpeg DLLs (+ LGPL notices) in one folder,
# then optionally zips it. The per-user NSIS setup is built separately by
# scripts/package-gpui-nsis.ps1 from the same stage. Authenticode and .eglib
# file association stay deferred.
#
# Usage (repo root, after libheif is installed via vcpkg):
#   pwsh scripts/package-gpui-desktop.ps1
#   pwsh scripts/package-gpui-desktop.ps1 -SkipBuild
#   pwsh scripts/package-gpui-desktop.ps1 -OutDir dist/gpui-desktop -NoZip
#   pwsh scripts/package-gpui-desktop.ps1 -CleanHugeTarget   # cargo clean if target/ > 20 GiB (D-135)

param(
    [string]$OutDir = "dist/gpui-desktop",
    [string]$VcpkgRoot = $(if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } elseif ($env:VCPKG_INSTALLATION_ROOT) { $env:VCPKG_INSTALLATION_ROOT } else { "C:\vcpkg" }),
    [switch]$SkipBuild,
    [switch]$NoZip,
    [switch]$CleanHugeTarget
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repo

if ($CleanHugeTarget) {
    $cleanScript = Join-Path $repo "scripts\clean-target-if-huge.ps1"
    Write-Host "Checking workspace target/ size (D-135)…"
    & $cleanScript
    if ($LASTEXITCODE -ne 0) { throw "clean-target-if-huge.ps1 failed" }
}

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

# Stage libheif runtime beside the exe (D-070 / D-134). Prefer decode-only bundle
# (heif + libde265, refuses GPL x265). If the local vcpkg tree still links x265
# (or other siblings), fall back to copying every vcpkg-bin DLL heif imports so
# the portable layout launches; NSIS/CI stay on the strict bundle script.
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
        Write-Warning "bundle-heif-dlls.ps1 failed ($($_.Exception.Message)); copying transitive heif DLLs."
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
    function Get-DumpbinPath {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
        if (-not (Test-Path -LiteralPath $vswhere)) { return $null }
        $vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        if (-not $vsPath) { return $null }
        $candidate = Get-ChildItem -Path (Join-Path $vsPath "VC\Tools\MSVC") -Recurse -Filter dumpbin.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\Hostx64\\x64\\dumpbin\.exe$' } |
            Select-Object -First 1
        if ($candidate) { return $candidate.FullName }
        return $null
    }
    function Get-PeDependentDlls([string]$Dumpbin, [string]$DllPath) {
        if (-not $Dumpbin) { return @() }
        $output = & $Dumpbin /dependents $DllPath 2>$null | Out-String
        $names = @()
        $inSection = $false
        foreach ($line in $output -split "`r?`n") {
            if ($line -match 'Image has the following dependencies') { $inSection = $true; continue }
            if ($inSection) {
                if ($line.Trim() -eq "" -or $line -match '^\s*Summary') { break }
                $name = $line.Trim()
                if ($name -match '\.dll$') { $names += $name }
            }
        }
        return $names
    }

    $heifDll = Join-Path $heifBin "heif.dll"
    if (-not (Test-Path -LiteralPath $heifDll)) {
        throw "heif.dll not found under $heifBin. Install with: vcpkg install libheif[core,libde265]:x64-windows"
    }
    $copied = New-Object System.Collections.Generic.List[string]
    $queue = New-Object System.Collections.Generic.Queue[string]
    $queue.Enqueue("heif.dll")
    $dumpbin = Get-DumpbinPath
    while ($queue.Count -gt 0) {
        $name = $queue.Dequeue()
        if ($copied.Contains($name)) { continue }
        $src = Join-Path $heifBin $name
        if (-not (Test-Path -LiteralPath $src)) { continue }
        Copy-Item -LiteralPath $src -Destination (Join-Path $stage $name) -Force
        $copied.Add($name) | Out-Null
        foreach ($dep in Get-PeDependentDlls $dumpbin $src) {
            $depPath = Join-Path $heifBin $dep
            if ((Test-Path -LiteralPath $depPath) -and -not $copied.Contains($dep)) {
                $queue.Enqueue($dep)
            }
        }
    }
    if (-not $copied.Contains("libde265.dll")) {
        $de265 = Join-Path $heifBin "libde265.dll"
        if (Test-Path -LiteralPath $de265) {
            Copy-Item -LiteralPath $de265 -Destination (Join-Path $stage "libde265.dll") -Force
            $copied.Add("libde265.dll") | Out-Null
        }
    }
    if ($copied | Where-Object { $_ -match '(?i)^libx265|^x265' }) {
        Write-Warning "Portable layout includes GPL x265 DLL(s) because this vcpkg heif links them. Reinstall libheif[core,libde265] for decode-only (D-070)."
    }
    if (-not $dumpbin) {
        Write-Warning "dumpbin not found; copied only DLLs discovered without a full PE walk."
    }
    ($copied | Sort-Object) -join "`n" | Set-Content -LiteralPath (Join-Path $stage "bundled-dlls.txt") -Encoding utf8
    Set-Content -LiteralPath (Join-Path $stage "NOTICE-libheif.txt") -Value "libheif - LGPL-3.0-only. See https://github.com/strukturag/libheif" -Encoding utf8
    Set-Content -LiteralPath (Join-Path $stage "NOTICE-libde265.txt") -Value "libde265 - LGPL. See https://github.com/strukturag/libde265" -Encoding utf8
}

# Stage the FFmpeg runtime (video and audio, D-168): LGPL DLLs beside the exe. The bundle
# script refuses a GPL or non-free FFmpeg; unlike heif there is no fallback.
$ffmpegStage = Join-Path $stage "_ffmpeg-stage"
$ffmpegScript = Join-Path $repo "scripts\bundle-ffmpeg-dlls.ps1"
if ($pwshCmd) {
    & $pwshCmd.Source -NoProfile -File $ffmpegScript -Dest $ffmpegStage -VcpkgRoot $VcpkgRoot
    if ($LASTEXITCODE -ne 0) { throw "bundle-ffmpeg-dlls.ps1 failed" }
} else {
    & $ffmpegScript -Dest $ffmpegStage -VcpkgRoot $VcpkgRoot
}
Get-ChildItem -LiteralPath $ffmpegStage -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $stage $_.Name) -Force
}
Remove-Item -LiteralPath $ffmpegStage -Recurse -Force -ErrorAction SilentlyContinue

$icon = Join-Path $repo "crates\gpui-desktop\assets\icon.ico"
if (Test-Path -LiteralPath $icon) {
    Copy-Item -LiteralPath $icon -Destination (Join-Path $stage "icon.ico") -Force
}

$licenses = Join-Path $repo "crates\gpui-desktop\assets\THIRD_PARTY_LICENSES.txt"
if (Test-Path -LiteralPath $licenses) {
    Copy-Item -LiteralPath $licenses -Destination (Join-Path $stage "THIRD_PARTY_LICENSES.txt") -Force
}

$readme = @"
egl-viewer GPUI desktop (portable)

Run gpui-desktop.exe. Keep the heif and FFmpeg runtime DLLs beside the exe (D-070 / D-133 /
D-134 / D-168).
For cargo run / debug, put VCPKG installed\x64-windows\bin on PATH (includes libx265.dll
when the local heif was not built decode-only).

This layout is the GPUI portable distribution for Windows. A dedicated GPUI
NSIS installer and Authenticode signing are deferred.
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
