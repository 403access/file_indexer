# =============================================================================
# CrefoLib/Database/Schema.ps1 - schema creation, buffered statement batching,
# and SQL value escaping. Dot-sourced by CrefoLib/Database/index.psm1.
# =============================================================================

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

function Initialize-CrefoDatabase {
    [CmdletBinding()]
    param(
        [string]$DbPath,
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

function Complete-CrefoDatabase {
    [CmdletBinding()]
    param()
    if (-not $script:DbPath -or $script:DbBatch -eq 0) { return }
    $sql = "BEGIN;`n" + ($script:DbBuffer -join "`n") + "`nCOMMIT;"
    Invoke-CrefoSqlite -DbPath $script:DbPath -Sql $sql | Out-Null
    $script:DbBuffer.Clear()
    $script:DbBatch = 0
}

function Add-CrefoDbStatement {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Sql, [int]$BatchSize = 100)
    $script:DbBuffer.Add($Sql)
    $script:DbBatch++
    if ($script:DbBatch -ge $BatchSize) { Complete-CrefoDatabase }
}
