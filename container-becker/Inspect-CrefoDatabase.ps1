# =============================================================================
# Inspect-CrefoDatabase.ps1 - read-only diagnostic queries against crefo.db.
#
# Usage examples:
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -AccountId 10169
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -Status done
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -Stats
#   pwsh -File Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -AccountId 10169 -History
# =============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DbPath,
    [int]$AccountId,
    [string]$Status,
    [switch]$Stats,
    [switch]$History,
    [int]$Limit
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $DbPath)) {
    Write-Error "Database not found: $DbPath"
    exit 1
}

# Resolve sqlite3
function Resolve-Sqlite {
    $exe = Get-Command 'sqlite3' -ErrorAction SilentlyContinue
    if (-not $exe) { throw "sqlite3 binary not found in PATH." }
    return $exe.Source
}

$sqlite = Resolve-Sqlite

function Invoke-Sqlite {
    [CmdletBinding()]
    param([string]$Sql, [switch]$AsJson)
    $args = @('-bail')
    if ($AsJson) { $args += '-json' }
    $args += $DbPath
    $args += $Sql
    $output = & $sqlite @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        $detail = ($output -join ' ').Trim()
        throw "sqlite3 failed: $detail"
    }
    if ($AsJson -and $output) {
        return ConvertFrom-Json ($output -join '')
    }
    return $output
}

# ---------------------------------------------------------------------------
# Stats
# ---------------------------------------------------------------------------
if ($Stats) {
    $sql = @"
SELECT
    (SELECT COUNT(*) FROM accounts) AS accounts,
    (SELECT COUNT(*) FROM accounts WHERE status='done') AS done,
    (SELECT COUNT(*) FROM accounts WHERE status='pending') AS pending,
    (SELECT COUNT(*) FROM accounts WHERE status='failed') AS failed,
    (SELECT COUNT(*) FROM risk_snapshots) AS snapshots,
    (SELECT COUNT(*) FROM api_exchanges) AS exchanges,
    (SELECT COUNT(DISTINCT account_id) FROM risk_snapshots) AS accounts_with_snapshots
"@
    $row = Invoke-Sqlite -Sql $sql -AsJson
    [PSCustomObject]@{
        Accounts             = [long]$row.accounts
        Done                 = [long]$row.done
        Pending              = [long]$row.pending
        Failed               = [long]$row.failed
        Snapshots            = [long]$row.snapshots
        Exchanges            = [long]$row.exchanges
        AccountsWithSnapshots = [long]$row.accounts_with_snapshots
    } | Format-List
    exit 0
}

# ---------------------------------------------------------------------------
# Account list (optionally filtered)
# ---------------------------------------------------------------------------
$where = @()
if ($AccountId) { $where += "a.id = $AccountId" }
if ($Status)    { $where += "a.status = '$Status'" }

$whereSql = if ($where) { 'WHERE ' + ($where -join ' AND ') } else { '' }

$limitSql = ''
if ($Limit -and $Limit -gt 0) {
    $limitSql = "LIMIT $Limit"
}

$sql = @"
SELECT a.id, a.name, a.status, a.error, a.created_at, a.updated_at,
       s.limit_value, s.purchased, s.balance, s.limit_code, s.fetched_at, s.source
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
ORDER BY a.id
$limitSql
"@

$rows = @(Invoke-Sqlite -Sql $sql -AsJson)

if ($rows.Count -eq 0) {
    Write-Host "No accounts found."
    exit 0
}

if ($AccountId) {
    $acct = $rows[0]
    Write-Host ("Account : {0} ({1})" -f $acct.id, $acct.name)
    Write-Host ("Status  : {0}" -f $acct.status)
    if ($acct.error) { Write-Host ("Error   : {0}" -f $acct.error) }
    Write-Host ("Created : {0}" -f $acct.created_at)
    Write-Host ("Updated : {0}" -f $acct.updated_at)
    Write-Host ("Limit   : {0}" -f $acct.limit_value)
    Write-Host ("Purchased: {0}" -f $acct.purchased)
    Write-Host ("Balance : {0}" -f $acct.balance)
    Write-Host ("Code    : {0}" -f $acct.limit_code)
    Write-Host ("Fetched : {0}" -f $acct.fetched_at)
    Write-Host ("Source  : {0}" -f $acct.source)

    if ($History) {
        Write-Host ""
        Write-Host "Snapshot history:"
        $histSql = "SELECT id, limit_value, purchased, balance, limit_code, fetched_at, source, created_at FROM risk_snapshots WHERE account_id = $AccountId ORDER BY id DESC"
        $hist = @(Invoke-Sqlite -Sql $histSql -AsJson)
        if ($hist.Count -eq 0) {
            Write-Host "  (none)"
        }
        else {
            foreach ($h in $hist) {
                Write-Host ("  [{0}] limit={1} purchased={2} balance={3} code={4} fetched={5} source={6}" -f `
                    $h.id, $h.limit_value, $h.purchased, $h.balance, $h.limit_code, $h.fetched_at, $h.source)
            }
        }
    }
    exit 0
}

# Multi-row output
Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f 'ID', 'Name', 'Status', 'Limit', 'Code', 'Fetched')
Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f '--------', '------------------------------', '----------', '------------', '----------', '--------')

foreach ($r in $rows) {
    $name = if ($r.name.Length -gt 28) { $r.name.Substring(0, 28) + '..' } else { $r.name }
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f `
        $r.id, $name, $r.status,
        (if ($r.limit_value) { [math]::Round([double]$r.limit_value, 2).ToString('N2') } else { '0,00' }),
        $r.limit_code,
        (if ($r.fetched_at) { $r.fetched_at.Substring(0, [math]::Min(8, $r.fetched_at.Length)) } else { '' }))
}

Write-Host ""
Write-Host ("{0} row(s)" -f $rows.Count)
