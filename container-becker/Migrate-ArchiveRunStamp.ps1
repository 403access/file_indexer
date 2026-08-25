# =============================================================================
# Migrate-ArchiveRunStamp.ps1 - restructure an existing archive so that each
# script execution gets its own run-stamp subfolder.
#
# Before (current layout):
#   <ArchiveDir>/<endpoint>/<stamp>_<seq>_request.json
#   <ArchiveDir>/risks/<outer>/<inner>/debtor-<id>-risk/<stamp>_<seq>_*.json
#
# After:
#   <ArchiveDir>/<endpoint>/<run-stamp>/<stamp>_<seq>_request.json
#   <ArchiveDir>/risks/<outer>/<inner>/debtor-<id>-risk/<run-stamp>/<stamp>_<seq>_*.json
#
# The run-stamp is the first 15 characters of the filename (yyyyMMdd_HHmmss).
# Files that are already inside a run-stamp folder are skipped.
#
# Usage:
#   pwsh -File Migrate-ArchiveRunStamp.ps1 -ArchiveDir container-becker/archive
#   pwsh -File Migrate-ArchiveRunStamp.ps1 -ArchiveDir container-becker/archive -DryRun
# =============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ArchiveDir,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $ArchiveDir)) {
    throw "Archive directory not found: $ArchiveDir"
}

$endpointDirs = Get-ChildItem -LiteralPath $ArchiveDir -Directory -ErrorAction SilentlyContinue
if ($endpointDirs.Count -eq 0) {
    Write-Host "No subdirectories found in $ArchiveDir - nothing to migrate."
    exit 0
}

$moved = 0
$skipped = 0
$errors = 0

foreach ($endpointDir in $endpointDirs) {
    $files = Get-ChildItem -LiteralPath $endpointDir.FullName -File -Filter '*_request.json' -Recurse -ErrorAction SilentlyContinue
    if ($files.Count -eq 0) { continue }

    foreach ($reqFile in $files) {
        $name = $reqFile.Name
        if ($name.Length -lt 20 -or $name -notmatch '^\d{8}_\d{6}_') {
            $skipped++
            continue
        }
        $runStamp = $name.Substring(0, 15)  # yyyyMMdd_HHmmss

        $targetDir = Join-Path $reqFile.Directory.FullName $runStamp
        $alreadyNested = $false
        $parent = $reqFile.Directory.Name
        if ($parent -eq $runStamp) {
            $alreadyNested = $true
        }

        if ($alreadyNested) {
            $skipped++
            continue
        }

        if (-not (Test-Path -LiteralPath $targetDir)) {
            if ($DryRun) {
                Write-Host "[DRY RUN] Would create: $targetDir"
            }
            else {
                New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
            }
        }

        $baseName = $name.Substring(0, $name.Length - '_request.json'.Length)
        $extensions = @('_request.json', '_response.json', '_data.json')
        foreach ($ext in $extensions) {
            $src = Join-Path $reqFile.Directory.FullName ($baseName + $ext)
            if (Test-Path -LiteralPath $src) {
                $dst = Join-Path $targetDir ($baseName + $ext)
                if ($DryRun) {
                    Write-Host "[DRY RUN] Would move: $src -> $dst"
                }
                else {
                    Move-Item -LiteralPath $src -Destination $dst -Force
                }
                $moved++
            }
        }
    }
}

Write-Host ""
Write-Host "Migration complete."
Write-Host "  Moved file groups : $moved"
Write-Host "  Skipped           : $skipped"
Write-Host "  Errors            : $errors"
if ($DryRun) {
    Write-Host "  (dry run - no files were actually moved)"
}
