# Stages the Windows FFmpeg runtime DLLs beside the executable (video and audio, D-168).
# Like libheif (D-070): LGPL, linked dynamically, the DLLs sit next to gpui-desktop.exe.
#
# Ships avcodec, avformat, avfilter, avutil, swresample, swscale, dav1d (AV1) and zlib.
# Refuses a GPL or non-free FFmpeg: the vcpkg install must not use the `gpl` / `nonfree`
# features (no x264, x265, fdk-aac). dumpbin checks that nothing else is imported from the
# vcpkg bin directory.
#
# Usage (repo root, after
#   vcpkg install "ffmpeg[core,avcodec,avformat,avfilter,swresample,swscale,dav1d,zlib]:x64-windows"):
#   powershell -File scripts/bundle-ffmpeg-dlls.ps1
#   powershell -File scripts/bundle-ffmpeg-dlls.ps1 -Dest dist/ffmpeg-runtime -VcpkgRoot C:\vcpkg

param(
    [string]$VcpkgRoot = $(if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } elseif ($env:VCPKG_INSTALLATION_ROOT) { $env:VCPKG_INSTALLATION_ROOT } else { "C:\vcpkg" }),
    [string]$Dest = "dist/ffmpeg-runtime",
    [string]$Triplet = "x64-windows"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$bin = Join-Path $VcpkgRoot "installed\$Triplet\bin"
$patterns = @("avcodec-*.dll", "avformat-*.dll", "avfilter-*.dll", "avutil-*.dll",
    "swresample-*.dll", "swscale-*.dll", "dav1d.dll", "z.dll")
$required = New-Object System.Collections.Generic.List[string]
foreach ($pattern in $patterns) {
    $hits = @(Get-ChildItem -LiteralPath $bin -Filter $pattern -ErrorAction SilentlyContinue)
    if ($hits.Count -ne 1) {
        throw "Expected one $pattern under $bin, found $($hits.Count). Install with: vcpkg install `"ffmpeg[core,avcodec,avformat,avfilter,swresample,swscale,dav1d,zlib]:$Triplet`""
    }
    $required.Add($hits[0].Name) | Out-Null
}

$share = Join-Path $VcpkgRoot "installed\$Triplet\share"

# Refuse a GPL or non-free FFmpeg: vcpkg records each installed feature as its own paragraph.
$status = Join-Path $VcpkgRoot "installed\vcpkg\status"
if (Test-Path -LiteralPath $status) {
    $paragraphs = (Get-Content -LiteralPath $status -Raw) -split "(?:\r?\n){2,}"
    foreach ($paragraph in $paragraphs) {
        if ($paragraph -match '(?m)^Package: ffmpeg\s*$' -and
            $paragraph -match "(?m)^Architecture: $Triplet\s*$" -and
            $paragraph -match '(?m)^Feature: (gpl|nonfree)\s*$' -and
            $paragraph -match '(?m)^Status: install ok installed\s*$') {
            throw "The vcpkg ffmpeg:$Triplet has the gpl or nonfree feature; reinstall it with the LGPL feature set."
        }
    }
}

New-Item -ItemType Directory -Force -Path $Dest | Out-Null
Get-ChildItem -LiteralPath $Dest -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Extension -in ".dll", ".txt" } |
    Remove-Item -Force

function Get-Dumpbin {
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

foreach ($name in $required) {
    Copy-Item -LiteralPath (Join-Path $bin $name) -Destination (Join-Path $Dest $name) -Force
}

$dumpbin = Get-Dumpbin
if ($dumpbin) {
    $allowed = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($name in $required) { [void]$allowed.Add($name) }
    $extra = New-Object System.Collections.Generic.List[string]
    foreach ($name in $required) {
        foreach ($dep in Get-PeDependentDlls $dumpbin (Join-Path $Dest $name)) {
            if ((Test-Path -LiteralPath (Join-Path $bin $dep)) -and -not $allowed.Contains($dep)) {
                $extra.Add("$name -> $dep") | Out-Null
            }
        }
    }
    if ($extra.Count -gt 0) {
        throw "FFmpeg runtime imports vcpkg DLL(s) outside the bundle: $(($extra | Sort-Object -Unique) -join ', '). Check the vcpkg features or update scripts/bundle-ffmpeg-dlls.ps1."
    }
} else {
    Write-Warning "dumpbin not found; skipped the PE dependency check."
}

foreach ($pattern in @("*x264*.dll", "*x265*.dll", "*fdk-aac*.dll", "postproc-*.dll")) {
    $hits = @(Get-ChildItem -LiteralPath $Dest -Filter $pattern -ErrorAction SilentlyContinue)
    if ($hits.Count -gt 0) {
        throw "Refusing to bundle GPL or non-free DLL(s): $($hits.Name -join ', ')."
    }
}

# License notices from the vcpkg ports.
foreach ($pair in @(
        @{ Port = "ffmpeg"; Out = "NOTICE-ffmpeg.txt"; Fallback = "FFmpeg - LGPL-2.1-or-later. Source: https://ffmpeg.org/download.html" },
        @{ Port = "dav1d"; Out = "NOTICE-dav1d.txt"; Fallback = "dav1d - BSD-2-Clause. See https://code.videolan.org/videolan/dav1d" },
        @{ Port = "zlib"; Out = "NOTICE-zlib.txt"; Fallback = "zlib - zlib License. See https://zlib.net" }
    )) {
    $copyright = Join-Path $share "$($pair.Port)\copyright"
    $outPath = Join-Path $Dest $pair.Out
    if (Test-Path -LiteralPath $copyright) {
        Copy-Item -LiteralPath $copyright -Destination $outPath -Force
    } else {
        Set-Content -LiteralPath $outPath -Value $pair.Fallback -Encoding utf8
        Write-Warning "No vcpkg copyright for $($pair.Port) at $copyright; wrote a short notice."
    }
}

$manifest = Join-Path $Dest "bundled-ffmpeg-dlls.txt"
($required | Sort-Object) -join "`n" | Set-Content -LiteralPath $manifest -Encoding utf8
Write-Host "Staged $($required.Count) FFmpeg DLL(s) into $Dest :"
$required | Sort-Object | ForEach-Object { Write-Host "  $_" }
