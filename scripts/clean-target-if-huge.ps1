# Deletes the Cargo `target` directory when it exceeds a size threshold (D-135).
# Only the workspace `target/` folder is considered. Never touches `.eglib` libraries,
# `vcpkg`, `dist/`, `node_modules/`, or any other path.
#
# Usage (repo root):
#   pwsh scripts/clean-target-if-huge.ps1
#   pwsh scripts/clean-target-if-huge.ps1 -ThresholdGiB 20
#   pwsh scripts/clean-target-if-huge.ps1 -WhatIf
#   pwsh scripts/clean-target-if-huge.ps1 -Force   # clean even below the threshold
#
# Prefer `cargo clean` when cargo is on PATH; fall back to Remove-Item on target/.

param(
    [double]$ThresholdGiB = 20,
    [switch]$Force,
    [switch]$WhatIf
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
$target = Join-Path $repo "target"
$thresholdBytes = [int64]([math]::Round($ThresholdGiB * 1GB))

function Format-GiB([int64]$bytes) {
    return "{0:N2} GiB" -f ($bytes / 1GB)
}

function Get-DirectoryByteSize([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return [int64]0 }
    $sum = [int64]0
    Get-ChildItem -LiteralPath $path -Recurse -Force -File -ErrorAction SilentlyContinue |
        ForEach-Object { $sum += $_.Length }
    return $sum
}

Write-Host "Repo: $repo"
Write-Host "Target: $target"
Write-Host ("Threshold: {0} ({1:N0} bytes)" -f (Format-GiB $thresholdBytes), $thresholdBytes)

if (-not (Test-Path -LiteralPath $target)) {
    Write-Host "No target/ directory — nothing to do."
    exit 0
}

# Refuse if target somehow resolved outside the repo (symlink / junction safety).
$targetFull = (Resolve-Path -LiteralPath $target).Path
$repoFull = $repo.Path
if (-not $targetFull.StartsWith($repoFull, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean '$targetFull': it is not under the repo root '$repoFull'."
}

$size = Get-DirectoryByteSize $target
Write-Host ("Current size: {0} ({1:N0} bytes)" -f (Format-GiB $size), $size)

$shouldClean = $Force -or ($size -gt $thresholdBytes)
if (-not $shouldClean) {
    Write-Host ("Under threshold — leaving target/ in place ({0} <= {1})." -f (Format-GiB $size), (Format-GiB $thresholdBytes))
    exit 0
}

if ($WhatIf) {
    Write-Host ("WhatIf: would clean target/ ({0} > {1}; Force={2})." -f (Format-GiB $size), (Format-GiB $thresholdBytes), [bool]$Force)
    exit 0
}

$reason = if ($Force -and $size -le $thresholdBytes) {
    "Force"
} else {
    "size {0} > {1}" -f (Format-GiB $size), (Format-GiB $thresholdBytes)
}
Write-Host "Cleaning target/ ($reason)…"

$cargo = Get-Command cargo -ErrorAction SilentlyContinue
$cleaned = $false
if ($cargo) {
    Push-Location $repo
    try {
        & $cargo.Source clean
        if ($LASTEXITCODE -eq 0) {
            $cleaned = $true
            Write-Host "Ran: cargo clean"
        } else {
            Write-Warning "cargo clean exited $LASTEXITCODE; falling back to Remove-Item."
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Warning "cargo not on PATH; removing target/ directly."
}

if (-not $cleaned) {
    Remove-Item -LiteralPath $target -Recurse -Force
    Write-Host "Removed: $target"
}

$after = Get-DirectoryByteSize $target
if (Test-Path -LiteralPath $target) {
    Write-Host ("Done. Remaining size: {0}." -f (Format-GiB $after))
} else {
    Write-Host "Done. target/ is gone."
}
