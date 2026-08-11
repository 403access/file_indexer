# =============================================================================
# CrefoLib/Inspect.psm1 - read-only diagnostic queries against crefo.db.
# Dot-sourced by Inspect-CrefoDatabase.ps1.
# =============================================================================

function Invoke-InspectSqlite {
    [CmdletBinding()]
    param(
        [string]$DbPath,
        [string]$Sql,
        [switch]$AsJson
    )
    if (-not (Test-Path -LiteralPath $DbPath)) {
        throw "Database not found: $DbPath"
    }
    $exe = Get-Command 'sqlite3' -ErrorAction Stop
    $args = @('-bail')
    if ($AsJson) { $args += '-json' }
    $args += $DbPath
    $args += $Sql
    $output = & $exe.Source @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        $detail = ($output -join ' ').Trim()
        throw "sqlite3 failed: $detail"
    }
    if ($AsJson -and $output) {
        return ConvertFrom-Json ($output -join '')
    }
    return $output
}

function Get-DatabaseStats {
    [CmdletBinding()]
    param([string]$DbPath)
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
    $row = Invoke-InspectSqlite -DbPath $DbPath -Sql $sql -AsJson
    return [PSCustomObject]@{
        Accounts              = [long]$row.accounts
        Done                  = [long]$row.done
        Pending               = [long]$row.pending
        Failed                = [long]$row.failed
        Snapshots             = [long]$row.snapshots
        Exchanges             = [long]$row.exchanges
        AccountsWithSnapshots = [long]$row.accounts_with_snapshots
    }
}

function Get-InspectAccount {
    [CmdletBinding()]
    param(
        [string]$DbPath,
        [int]$AccountId,
        [string]$Status,
        [int]$Limit
    )
    $where = @()
    if ($AccountId) { $where += "a.id = $AccountId" }
    if ($Status)    { $where += "a.status = '$Status'" }

    $whereSql = if ($where) { 'WHERE ' + ($where -join ' AND ') } else { '' }
    $limitSql = if ($Limit -and $Limit -gt 0) { "LIMIT $Limit" } else { '' }

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
    return @(Invoke-InspectSqlite -DbPath $DbPath -Sql $sql -AsJson)
}

function Get-InspectAccountHistory {
    [CmdletBinding()]
    param(
        [string]$DbPath,
        [int]$AccountId
    )
    $sql = "SELECT id, limit_value, purchased, balance, limit_code, fetched_at, source, created_at FROM risk_snapshots WHERE account_id = $AccountId ORDER BY id DESC"
    return @(Invoke-InspectSqlite -DbPath $DbPath -Sql $sql -AsJson)
}

function Show-DatabaseStats {
    [CmdletBinding()]
    param([object]$StatsObject)
    foreach ($prop in $StatsObject.PSObject.Properties) {
        Write-Host ("{0,-20} {1}" -f $prop.Name, $prop.Value)
    }
}

function Show-InspectAccountDetail {
    [CmdletBinding()]
    param([object]$Account, [switch]$History, [string]$DbPath)
    Write-Host ("Account : {0} ({1})" -f $Account.id, $Account.name)
    Write-Host ("Status  : {0}" -f $Account.status)
    if ($Account.error) { Write-Host ("Error   : {0}" -f $Account.error) }
    Write-Host ("Created : {0}" -f $Account.created_at)
    Write-Host ("Updated : {0}" -f $Account.updated_at)
    Write-Host ("Limit   : {0}" -f $Account.limit_value)
    Write-Host ("Purchased: {0}" -f $Account.purchased)
    Write-Host ("Balance : {0}" -f $Account.balance)
    Write-Host ("Code    : {0}" -f $Account.limit_code)
    Write-Host ("Fetched : {0}" -f $Account.fetched_at)
    Write-Host ("Source  : {0}" -f $Account.source)

    if ($History) {
        Write-Host ""
        Write-Host "Snapshot history:"
        $hist = Get-InspectAccountHistory -DbPath $DbPath -AccountId $Account.id
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
}

function Show-InspectAccountList {
    [CmdletBinding()]
    param([object[]]$Rows)
    if ($Rows.Count -eq 0) {
        Write-Host "No accounts found."
        return
    }
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f 'ID', 'Name', 'Status', 'Limit', 'Code', 'Fetched')
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f '--------', '------------------------------', '----------', '------------', '----------', '--------')

    foreach ($r in $Rows) {
        if ($r.name.Length -gt 28) {
            $name = $r.name.Substring(0, 28) + '..'
        }
        else {
            $name = $r.name
        }
        $limitDisplay = if ($r.limit_value) { [math]::Round([double]$r.limit_value, 2).ToString('N2') } else { '0,00' }
        $fetchedDisplay = if ($r.fetched_at) { $r.fetched_at.Substring(0, [math]::Min(8, $r.fetched_at.Length)) } else { '' }
        Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f `
            $r.id, $name, $r.status, $limitDisplay, $r.limit_code, $fetchedDisplay)
    }
    Write-Host ""
    Write-Host ("{0} row(s)" -f $Rows.Count)
}
