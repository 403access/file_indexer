# =============================================================================
# CrefoLib/Database/Writes.ps1 - buffered entity INSERT/UPDATE operations.
# Dot-sourced by CrefoLib/Database/index.psm1.
# =============================================================================

function Save-CrefoAccount {
    [CmdletBinding()]
    param([object]$Account)
    if (-not $script:DbPath) { throw 'Database not initialized.' }
    $id = [int]$Account.id

    $name = if ($null -eq $Account.name) { '' } else { [string]$Account.name }
    $status = if ([string]::IsNullOrWhiteSpace([string]$Account.status)) { 'pending' } else { [string]$Account.status }
    $error = if ($null -eq $Account.error) { '' } else { [string]$Account.error }
    $createdAt = if ([string]::IsNullOrWhiteSpace([string]$Account.createdAt)) { ((Get-Date).ToUniversalTime().ToString('o')) } else { [string]$Account.createdAt }

    $sql = "INSERT INTO accounts (id, name, status, error, created_at, updated_at) VALUES ($id, " +
        "{0}, {1}, {2}, {3}, {4}) ON CONFLICT(id) DO UPDATE SET name=excluded.name, status=excluded.status, error=excluded.error, updated_at=excluded.updated_at;" -f @(
            (ConvertTo-CrefoSqlValue $name),
            (ConvertTo-CrefoSqlValue $status),
            (ConvertTo-CrefoSqlValue $error),
            (ConvertTo-CrefoSqlValue $createdAt),
            (ConvertTo-CrefoSqlValue ((Get-Date).ToUniversalTime().ToString('o')))
        )
    Add-CrefoDbStatement -Sql $sql
}

function Save-CrefoRiskSnapshot {
    [CmdletBinding()]
    param(
        [int]$AccountId,
        [object]$Risk,
        [string]$Source = 'api',
        [bool]$RiskFetched = $true
    )
    if (-not $script:DbPath) { throw 'Database not initialized.' }
    $now = (Get-Date).ToUniversalTime().ToString('o')
    $fetchedAt = ConvertTo-CrefoSqlValue ([string]$Risk.fetchedAt)
    $limit = ConvertTo-CrefoSqlValue ([double]$Risk.limit)
    $purchased = ConvertTo-CrefoSqlValue ([double]$Risk.purchased)
    $balance = ConvertTo-CrefoSqlValue ([double]$Risk.balance)
    $limitCode = ConvertTo-CrefoSqlValue ([string]$Risk.limitCode)
    $riskFetched = $(if ($RiskFetched) { '1' } else { '0' })
    $sql = "INSERT INTO risk_snapshots (account_id, fetched_at, limit_value, purchased, balance, limit_code, risk_fetched, source, created_at) " +
           "VALUES ($AccountId, $fetchedAt, $limit, $purchased, $balance, $limitCode, $riskFetched, '$Source', '$now');"
    Add-CrefoDbStatement -Sql $sql
}

function Save-CrefoApiExchange {
    [CmdletBinding()]
    param(
        [string]$Endpoint,
        [string]$Method,
        [string]$Url,
        [int]$StatusCode,
        [double]$ElapsedMs,
        [bool]$Archived = $false,
        [string]$ArchivePath = ''
    )
    if (-not $script:DbPath) { return }
    $now = (Get-Date).ToUniversalTime().ToString('o')
    $archived = $(if ($Archived) { '1' } else { '0' })
    $sql = "INSERT INTO api_exchanges (endpoint, method, url, status_code, elapsed_ms, archived, archive_path, created_at) " +
           "VALUES ({0}, {1}, {2}, {3}, {4}, {5}, {6}, '{7}');" -f `
        (ConvertTo-CrefoSqlValue $Endpoint), (ConvertTo-CrefoSqlValue $Method), (ConvertTo-CrefoSqlValue $Url), `
        $StatusCode, ([double]$ElapsedMs).ToString([System.Globalization.CultureInfo]::InvariantCulture), `
        $archived, (ConvertTo-CrefoSqlValue $ArchivePath), $now
    Add-CrefoDbStatement -Sql $sql
}

function Save-CrefoApiExchangeLog {
    [CmdletBinding()]
    param(
        [string]$Name,
        [string]$Method,
        [string]$Uri,
        [int]$StatusCode,
        [double]$ElapsedMs,
        [bool]$Archived = $false,
        [string]$ArchivePath = ''
    )
    if (-not $script:DbPath) { return }
    Save-CrefoApiExchange -Endpoint $Name -Method $Method -Url $Uri -StatusCode $StatusCode -ElapsedMs $ElapsedMs -Archived $Archived -ArchivePath $ArchivePath
}
