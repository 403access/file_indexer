# =============================================================================
# CrefoLib/Inspect/Queries.ps1 - data retrieval functions for inspection.
# Dot-sourced by CrefoLib/Inspect/index.psm1.
# =============================================================================

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
