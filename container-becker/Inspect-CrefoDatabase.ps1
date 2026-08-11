# =============================================================================
# Inspect-CrefoDatabase.ps1 - read-only diagnostic queries against crefo.db.
#
# Usage examples:
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -AccountId 10169
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -Status done
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -ShowStats
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -AccountId 10169 -History
# =============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DbPath,
    [int]$AccountId,
    [string]$Status,
    [switch]$ShowStats,
    [switch]$History,
    [int]$Limit
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $DbPath)) {
    Write-Error "Database not found: $DbPath"
    exit 1
}

Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Inspect\index.psm1') -Global -Force

if ($ShowStats) {
    $stats = Get-DatabaseStats -DbPath $DbPath
    Show-DatabaseStats -StatsObject $stats
    exit 0
}

$rows = Get-InspectAccount -DbPath $DbPath -AccountId $AccountId -Status $Status -Limit $Limit

if ($AccountId -and $rows.Count -gt 0) {
    Show-InspectAccountDetail -Account $rows[0] -History:$History -DbPath $DbPath
    exit 0
}

Show-InspectAccountList -Rows $rows
