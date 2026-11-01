# Stages the Windows libheif runtime DLLs for the NSIS package (D-070).
# Tauri maps files under src-tauri/heif-runtime/ to the install root (beside
# egl-viewer.exe) via tauri.windows.conf.json. LoadLibrary only searches the
# exe directory.
#
# Ships heif.dll + libde265.dll only (decode-only vcpkg features). Refuses GPL
# x265. dumpbin verifies those two pull no other DLLs from the vcpkg bin dir.
#
# Usage (repo root, after `vcpkg install libheif[core,libde265]:x64-windows`):
#   pwsh scripts/bundle-heif-dlls.ps1
#   pwsh scripts/bundle-heif-dlls.ps1 -Dest src-tauri/heif-runtime -VcpkgRoot C:\vcpkg

param(
    [string]$VcpkgRoot = $(if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } elseif ($env:VCPKG_INSTALLATION_ROOT) { $env:VCPKG_INSTALLATION_ROOT } else { "C:\vcpkg" }),
    [string]$Dest = "src-tauri/heif-runtime",
    [string]$Triplet = "x64-windows"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$bin = Join-Path $VcpkgRoot "installed\$Triplet\bin"
$required = @("heif.dll", "libde265.dll")
foreach ($name in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $bin $name))) {
        throw "$name not found under $bin. Install with: vcpkg install libheif[core,libde265]:$Triplet"
    }
}

New-Item -ItemType Directory -Force -Path $Dest | Out-Null

# Clear previous staged artefacts so a feature change cannot leave GPL siblings behind.
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

$copied = New-Object System.Collections.Generic.List[string]
foreach ($name in $required) {
    Copy-Item -LiteralPath (Join-Path $bin $name) -Destination (Join-Path $Dest $name) -Force
    $copied.Add($name) | Out-Null
}

# If dumpbin is available, every import that exists in the vcpkg bin must already
# be in $required — otherwise tauri.windows.conf.json would miss a sibling DLL.
$dumpbin = Get-Dumpbin
if ($dumpbin) {
    $allowed = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($name in $required) { [void]$allowed.Add($name) }
    $extra = New-Object System.Collections.Generic.List[string]
    foreach ($name in $required) {
        foreach ($dep in Get-PeDependentDlls $dumpbin (Join-Path $Dest $name)) {
            $depPath = Join-Path $bin $dep
            if ((Test-Path -LiteralPath $depPath) -and -not $allowed.Contains($dep)) {
                $extra.Add($dep) | Out-Null
            }
        }
    }
    if ($extra.Count -gt 0) {
        $unique = $extra | Sort-Object -Unique
        throw "heif runtime imports extra vcpkg DLL(s) not in the bundle list: $($unique -join ', '). Update scripts/bundle-heif-dlls.ps1 and tauri.windows.conf.json."
    }
} else {
    Write-Warning "dumpbin not found; skipped PE dependency check for vcpkg siblings."
}

# LGPL notices from the vcpkg port copyright files (D-063 / D-070).
$share = Join-Path $VcpkgRoot "installed\$Triplet\share"
foreach ($pair in @(
        @{ Port = "libheif"; Out = "NOTICE-libheif.txt"; Fallback = "libheif — LGPL-3.0-only. See https://github.com/strukturag/libheif" },
        @{ Port = "libde265"; Out = "NOTICE-libde265.txt"; Fallback = "libde265 — LGPL. See https://github.com/strukturag/libde265" }
    )) {
    $copyright = Join-Path $share "$($pair.Port)\copyright"
    $outPath = Join-Path $Dest $pair.Out
    if (Test-Path -LiteralPath $copyright) {
        Copy-Item -LiteralPath $copyright -Destination $outPath -Force
    } else {
        Set-Content -LiteralPath $outPath -Value $pair.Fallback -Encoding utf8NoBOM
        Write-Warning "No vcpkg copyright for $($pair.Port) at $copyright; wrote a short notice."
    }
}

foreach ($pattern in @("libx265.dll", "x265.dll", "libx265-*.dll")) {
    $hits = @(Get-ChildItem -LiteralPath $Dest -Filter $pattern -ErrorAction SilentlyContinue)
    if ($hits.Count -gt 0) {
        throw "Refusing to bundle GPL x265 DLL(s): $($hits.Name -join ', '). Reinstall with libheif[core,libde265]."
    }
}

$manifest = Join-Path $Dest "bundled-dlls.txt"
($copied | Sort-Object) -join "`n" | Set-Content -LiteralPath $manifest -Encoding utf8NoBOM
Write-Host "Staged $($copied.Count) DLL(s) into $Dest :"
$copied | Sort-Object | ForEach-Object { Write-Host "  $_" }
