# =============================================================================
# Show-CrefoAccountsByLimit.ps1 - list all accounts with credit limits, sorted
# by limit size (descending). Uses the Inspect submodule for DB access.
#
# Usage:
#   pwsh -File Show-CrefoAccountsByLimit.ps1 -DbPath data/state/crefo.db
#   pwsh -File Show-CrefoAccountsByLimit.ps1 -DbPath data/state/crefo.db -Top 20
#   pwsh -File Show-CrefoAccountsByLimit.ps1 -DbPath data/state/crefo.db -Status done
#   pwsh -File Show-CrefoAccountsByLimit.ps1 -DbPath data/state/crefo.db -Top 20 -SortBy Gekauft
#   pwsh -File Show-CrefoAccountsByLimit.ps1 -DbPath data/state/crefo.db -AccountId 10169
# =============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DbPath,
    [int]$Top = 0,
    [string]$Status,
    [int]$AccountId,
    [ValidateSet('Limit', 'Gekauft', 'Balance', 'ID', 'Name')]
    [string]$SortBy = 'Limit'
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $DbPath)) {
    Write-Error "Database not found: $DbPath"
    exit 1
}

Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Inspect\index.psm1') -Global -Force

$where = @()
if ($AccountId) { $where += "a.id = $AccountId" }
if ($Status)    { $where += "a.status = '$Status'" }
$whereSql = if ($where) { 'WHERE ' + ($where -join ' AND ') } else { '' }

$topSql = if ($Top -gt 0 -and -not $AccountId) { "LIMIT $Top" } else { '' }

$sortColumn = switch ($SortBy) {
    'Limit'   { 's.limit_value' }
    'Gekauft' { 's.purchased' }
    'Balance' { 's.balance' }
    'ID'      { 'a.id' }
    'Name'    { 'a.name' }
    default   { 's.limit_value' }
}
$sortDirection = if ($SortBy -eq 'ID') { 'ASC' } else { 'DESC NULLS LAST' }

$sql = @"
SELECT a.id, a.name, a.status,
       s.limit_value, s.purchased, s.balance, s.limit_code, s.fetched_at
FROM accounts a
LEFT JOIN (
    SELECT rs0.* FROM risk_snapshots rs0
    JOIN (
        SELECT account_id, MAX(id) AS last_id
        FROM risk_snapshots
        GROUP BY account_id
    ) latest ON latest.account_id = rs0.account_id AND latest.last_id = rs0.id
) s ON s.account_id = a.id
$whereSql
ORDER BY $sortColumn $sortDirection, a.id ASC
$topSql
"@

$rows = @(Invoke-InspectSqlite -DbPath $DbPath -Sql $sql -AsJson)

if ($rows.Count -eq 0) {
    Write-Host "No accounts found."
    exit 0
}

Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-10} {6,-8}" -f 'ID', 'Name', 'Status', 'Limit', 'Gekauft', 'Balance', 'Code')
Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-10} {6,-8}" -f '--------', '------------------------------', '----------', '------------', '----------', '----------', '--------')

foreach ($r in $rows) {
    $name = if ($r.name.Length -gt 28) { $r.name.Substring(0, 28) + '..' } else { $r.name }
    $limitDisplay = if ($r.limit_value) { [math]::Round([double]$r.limit_value, 2).ToString('N2') } else { '0,00' }
    $purchasedDisplay = if ($r.purchased) { [math]::Round([double]$r.purchased, 2).ToString('N2') } else { '0,00' }
    $balanceDisplay = if ($r.balance) { [math]::Round([double]$r.balance, 2).ToString('N2') } else { '0,00' }
    $fetchedDisplay = if ($r.fetched_at) { ([string]$r.fetched_at).Substring(0, [math]::Min(8, ([string]$r.fetched_at).Length)) } else { '' }
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-10} {6,-8}" -f `
        $r.id, $name, $r.status, $limitDisplay, $purchasedDisplay, $balanceDisplay, $r.limit_code)
}

Write-Host ""
Write-Host ("{0} account(s)" -f $rows.Count)
if ($Top -gt 0 -and -not $AccountId) {
    Write-Host ("(showing top {0} by {1})" -f $Top, $SortBy.ToLower())
}
