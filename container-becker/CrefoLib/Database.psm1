# =============================================================================
# Database.psm1 - local SQLite store for the Crefo export (source of truth).
#
# Everything the CSV needs is persisted here so the CSV can be rebuilt purely
# from the database while the JSON state file is kept alongside for rollback:
#   - accounts          one row per debitor (id, name, status, error, times)
#   - risk_snapshots    append-only history of every /risk result (and the
#                       short-circuit zero rows), latest per account wins
#   - api_exchanges     audit log of every API call (endpoint, http, url,
#                       elapsed, archived path / timestamp)
#
# Access: this module shells out to the sqlite3 CLI (macOS ships it). Writes
# are batched into transactions (BEGIN ... COMMIT) and executed with -bail so
# a failed statement rolls back the whole batch atomically. Resolution of
# archive file paths is left to the caller: stored timestamps + debtor id are
# enough to reconstruct archive/risks/<outer>/<inner>/<debtor-<id>-risk/...
# =============================================================================

$script:SqliteExe = $null      # resolved path to the sqlite3 binary
$script:DbPath    = $null      # full path of the open database file
$script:DbBuffer  = $null      # pending SQL statements (flushed in a transaction)
$script:DbBatch   = 0          # number of statements accumulated since last flush

# ---------------------------------------------------------------------------
# Low-level sqlite3 wrapper
# ---------------------------------------------------------------------------

# Locates the sqlite3 binary once (configurable, default = PATH lookup).
function Resolve-CrefoSqlite {
    [CmdletBinding()]
    param([string]$SqlitePath = '')
    if ($script:SqliteExe) { return $script:SqliteExe }
    $exe = if ([string]::IsNullOrWhiteSpace($SqlitePath)) {
        (Get-Command 'sqlite3' -ErrorAction Stop).Source
    }
    else { $SqlitePath }
    $script:SqliteExe = $exe
    return $exe
}

# Executes SQL text against the database. Supports a JSON result set with the
# -AsJson switch. The whole call runs in autocommit (callers wrap batches in
# BEGIN/COMMIT themselves for atomicity).
function Invoke-CrefoSqlite {
    [CmdletBinding()]
    param(
        [string]$DbPath,
        [Parameter(Mandatory = $true)][string]$Sql,
        [string]$SqlitePath = '',
        [switch]$AsJson
    )
    $exe = Resolve-CrefoSqlite -SqlitePath $SqlitePath
    $args = New-Object System.Collections.Generic.List[string]
    $args.Add('-bail')
    if ($AsJson) { $args.Add('-json') }
    $args.Add($DbPath)
    $args.Add($Sql)
    $output = & $exe @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        $detail = ($output -join ' ').Trim()
        throw ("sqlite3 failed (exit {0}): {1}" -f $LASTEXITCODE, $detail)
    }
    if ($AsJson) {
        if (-not [string]::IsNullOrWhiteSpace([string]$output)) {
            return @(ConvertFrom-Json ([string]$output -join ''))
        }
        return @()
    }
    return $output
}

# ---------------------------------------------------------------------------
# Schema & lifecycle
# ---------------------------------------------------------------------------

# Creates the schema (idempotent) and switches the database to WAL mode.
function Initialize-CrefoDatabase {
    [CmdletBinding()]
    param(
        [string]$DbPath,                        # full path of the .db file
        [string]$SqlitePath = ''
    )
    $dir = Split-Path -Parent $DbPath
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $script:DbPath = $DbPath
    $script:DbBatch = 0
    if ($script:DbBuffer) { $script:DbBuffer.Clear() }
    $script:DbBuffer = New-Object System.Collections.Generic.List[string]

    $schema = @'
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS accounts (
    id           INTEGER PRIMARY KEY,
    name         TEXT    NOT NULL DEFAULT '',
    status       TEXT    NOT NULL DEFAULT 'pending',
    error        TEXT,
    created_at   TEXT    NOT NULL,
    updated_at   TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS risk_snapshots (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id   INTEGER NOT NULL REFERENCES accounts(id),
    fetched_at   TEXT,
    limit_value  REAL    NOT NULL DEFAULT 0,
    purchased    REAL    NOT NULL DEFAULT 0,
    balance      REAL    NOT NULL DEFAULT 0,
    limit_code   TEXT    NOT NULL DEFAULT 'N',
    risk_fetched INTEGER NOT NULL DEFAULT 0,
    source       TEXT    NOT NULL DEFAULT 'api',
    created_at   TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_risk_snapshots_account
    ON risk_snapshots (account_id, id DESC);

CREATE TABLE IF NOT EXISTS api_exchanges (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint      TEXT,
    method        TEXT,
    url           TEXT,
    status_code   INTEGER,
    elapsed_ms    REAL,
    archived      INTEGER NOT NULL DEFAULT 0,
    archive_path  TEXT,
    created_at    TEXT    NOT NULL
);
'@
    Invoke-CrefoSqlite -DbPath $DbPath -Sql $schema -SqlitePath $SqlitePath | Out-Null
}

# Flushes the accumulated statements inside one atomic transaction. Clears the
# buffer afterwards; if a statement fails, -bail aborts before COMMIT and the
# whole batch rolls back (nothing is lost from the buffer).
function Complete-CrefoDatabase {
    [CmdletBinding()]
    param()
    if (-not $script:DbPath -or $script:DbBatch -eq 0) { return }
    $sql = "BEGIN;`n" + ($script:DbBuffer -join "`n") + "`nCOMMIT;"
    Invoke-CrefoSqlite -DbPath $script:DbPath -Sql $sql | Out-Null
    $script:DbBuffer.Clear()
    $script:DbBatch = 0
}

# Appends a statement to the buffer and auto-flushes past a batch size, so a
# long run is written in chunks rather than one giant CLI call.
function Add-CrefoDbStatement {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Sql, [int]$BatchSize = 100)
    $script:DbBuffer.Add($Sql)
    $script:DbBatch++
    if ($script:DbBatch -ge $BatchSize) { Complete-CrefoDatabase }
}

# Escapes a value into a SQL literal (handles NULL, strings and numbers).
function ConvertTo-CrefoSqlValue {
    [CmdletBinding()]
    param([object]$Value)
    if ($null -eq $Value) { return 'NULL' }
    if ($Value -is [string]) {
        if ([string]::IsNullOrWhiteSpace($Value)) { return 'NULL' }
        return ("'{0}'" -f (($Value -replace "'", "''").Trim()))
    }
    if ($Value -is [bool]) { return $(if ($Value) { '1' } else { '0' }) }
    if ($Value -is [double] -or $Value -is [int] -or $Value -is [long]) {
        return ([double]$Value).ToString([System.Globalization.CultureInfo]::InvariantCulture)
    }
    return ("'{0}'" -f (([string]$Value) -replace "'", "''"))
}

# ---------------------------------------------------------------------------
# Entity writes (buffered)
# ---------------------------------------------------------------------------

# Upserts one account row.
function Save-CrefoAccount {
    [CmdletBinding()]
    param([object]$Account)
    if (-not $script:DbPath) { throw 'Database not initialized.' }
    $id = [int]$Account.id

    # The account may lack optional fields depending on where it came from
    # (fresh API list vs. deserialized JSON state), so default defensively.
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

# Appends one risk snapshot. Source: 'api' (live fetch), 'archive' (backfill
# from on-disk responses) or 'short-circuit' (no request, explicit N row).
function Save-CrefoRiskSnapshot {
    [CmdletBinding()]
    param(
        [int]$AccountId,
        [object]$Risk,                       # @{limit;purchased;balance;limitCode;fetchedAt}
        [string]$Source = 'api',
        [bool]$RiskFetched = $true           # false marks the explicit N/zero row
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

# Appends one entry to the API exchange audit log.
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
    if (-not $script:DbPath) { return }   # optional logging - never fatal
    $now = (Get-Date).ToUniversalTime().ToString('o')
    $archived = $(if ($Archived) { '1' } else { '0' })
    $sql = "INSERT INTO api_exchanges (endpoint, method, url, status_code, elapsed_ms, archived, archive_path, created_at) " +
           "VALUES ({0}, {1}, {2}, {3}, {4}, {5}, {6}, '{7}');" -f `
        (ConvertTo-CrefoSqlValue $Endpoint), (ConvertTo-CrefoSqlValue $Method), (ConvertTo-CrefoSqlValue $Url), `
        $StatusCode, ([double]$ElapsedMs).ToString([System.Globalization.CultureInfo]::InvariantCulture), `
        $archived, (ConvertTo-CrefoSqlValue $ArchivePath), $now
    Add-CrefoDbStatement -Sql $sql
}

# Best-effort bridge used by ApiArchive when the Database module may not be
# loaded: only logs when the module is available and initialized. Never throws.
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

# ---------------------------------------------------------------------------
# Queries
# ---------------------------------------------------------------------------

# Seeds a fresh database from the JSON state file (one-time, idempotent).
# Accounts and their existing risk snapshots are copied over so the very first
# run after enabling the DB can already rebuild the CSV from it. Runs in the
# same buffered transaction batch as the writes that follow.
function Import-CrefoDatabaseFromState {
    [CmdletBinding()]
    param([object]$State)
    if (-not $script:DbPath) { throw 'Database not initialized.' }
    try {
        $first = @(Invoke-CrefoSqlite -DbPath $script:DbPath -Sql 'SELECT COUNT(*) AS c FROM accounts;' -AsJson)
        if ($first.Count -gt 0 -and [long]$first[0].c -gt 0) {
            # Database already seeded - do not overwrite history.
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

# Returns the current CSV rows as objects (id, name, limit, purchased, balance,
# limitCode) using the latest risk snapshot per account. Failed accounts and
# accounts without any snapshot are excluded, mirroring the in-memory CSV.
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

# Lightweight diagnostics for logging and tests.
function Get-CrefoDatabaseStats {
    [CmdletBinding()]
    param()
    if (-not $script:DbPath) { return $null }
    Complete-CrefoDatabase
    $counts = @(Invoke-CrefoSqlite -DbPath $script:DbPath -Sql "SELECT (SELECT COUNT(*) FROM accounts) AS accounts, (SELECT COUNT(*) FROM risk_snapshots) AS snapshots, (SELECT COUNT(*) FROM api_exchanges) AS exchanges;" -AsJson)
    if ($counts.Count -eq 0) { return $null }
    return $counts[0]
}

Export-ModuleMember -Function 'Invoke-CrefoSqlite', 'Initialize-CrefoDatabase', 'Complete-CrefoDatabase', 'Save-CrefoAccount', 'Save-CrefoRiskSnapshot', 'Save-CrefoApiExchange', 'Save-CrefoApiExchangeLog', 'Import-CrefoDatabaseFromState', 'Get-CrefoDatabaseCsvRows', 'Get-CrefoDatabaseStats'