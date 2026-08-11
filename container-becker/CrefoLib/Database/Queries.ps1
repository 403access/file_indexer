# =============================================================================
# CrefoLib/Database/Queries.ps1 - read/import operations against the SQLite DB.
# Dot-sourced by CrefoLib/Database/index.psm1.
# =============================================================================

function Import-CrefoDatabaseFromState {
    [CmdletBinding()]
    param([object]$State)
    if (-not $script:DbPath) { throw 'Database not initialized.' }
    try {
        $first = @(Invoke-CrefoSqlite -DbPath $script:DbPath -Sql 'SELECT COUNT(*) AS c FROM accounts;' -AsJson)
        if ($first.Count -gt 0 -and [long]$first[0].c -gt 0) {
            return
        }
        foreach ($account in @($State.accounts)) {
            if ($null -eq $account -or $null -eq $account.id) { continue }
            $id = [int]$account.id
            Save-CrefoAccount -Account $account
            if ($null -ne $account.limitCode) {
                $fetchedAt = if ([string]::IsNullOrWhiteSpace([string]$account.fetchedAt)) { $account.updatedAt } else { $account.fetchedAt }
                Save-CrefoRiskSnapshot -AccountId $id -Risk ([pscustomobject]@{
                    limit      = [double]$account.limit
                    purchased  = [double]$account.purchased
                    balance    = [double]$account.balance
                    limitCode  = [string]$account.limitCode
                    fetchedAt  = [string]$fetchedAt
                }) -Source 'json' -RiskFetched ([bool]$account.riskFetched)
            }
        }
        Complete-CrefoDatabase
        Write-CrefoInfo ("Seed: copied {0} known account(s)/snapshot(s) into the database." -f @($State.accounts).Count)
    }
    catch {
        Write-CrefoWarn ("Database seed from JSON state failed ({0}); the CSV will rebuild from the live run instead." -f $_.Exception.Message)
    }
}

function Get-CrefoDatabaseCsvRows {
    [CmdletBinding()]
    param()
    if (-not $script:DbPath) { throw 'Database not initialized.' }
    Complete-CrefoDatabase
    $sql = @'
SELECT a.id AS id, a.name AS name,
       s.limit_value  AS limit_value,
       s.purchased    AS purchased,
       s.balance      AS balance,
       s.limit_code   AS limit_code
FROM accounts a
JOIN (
    SELECT rank0.account_id,
           rank0.id AS snap_id,
           rank0.limit_value, rank0.purchased, rank0.balance, rank0.limit_code
    FROM risk_snapshots rank0
    JOIN (
        SELECT account_id, MAX(id) AS last_id
        FROM risk_snapshots
        GROUP BY account_id
    ) latest ON latest.account_id = rank0.account_id AND latest.last_id = rank0.id
) s ON s.account_id = a.id
WHERE a.status <> 'failed'
ORDER BY a.id;
'@
    $rows = @(Invoke-CrefoSqlite -DbPath $script:DbPath -Sql $sql -AsJson)
    return @($rows | ForEach-Object {
        [pscustomobject]@{
            id        = [int]$_.id
            name      = [string]$_.name
            limit     = [double]$_.limit_value
            purchased = [double]$_.purchased
            balance   = [double]$_.balance
            limitCode = [string]$_.limit_code
        }
    })
}

function Get-CrefoDatabaseStats {
    [CmdletBinding()]
    param()
    if (-not $script:DbPath) { return $null }
    Complete-CrefoDatabase
    $counts = @(Invoke-CrefoSqlite -DbPath $script:DbPath -Sql "SELECT (SELECT COUNT(*) FROM accounts) AS accounts, (SELECT COUNT(*) FROM risk_snapshots) AS snapshots, (SELECT COUNT(*) FROM api_exchanges) AS exchanges;" -AsJson)
    if ($counts.Count -eq 0) { return $null }
    return $counts[0]
}

function Get-CrefoDatabaseAccountSummary {
    [CmdletBinding()]
    param()
    if (-not $script:DbPath) { return $null }
    Complete-CrefoDatabase
    $rows = @(Invoke-CrefoSqlite -DbPath $script:DbPath -Sql "SELECT COUNT(*) AS count, MAX(id) AS highest_id FROM accounts;" -AsJson)
    if ($rows.Count -eq 0) { return $null }
    return $rows[0]
}
