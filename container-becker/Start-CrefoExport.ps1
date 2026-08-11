# =============================================================================
# Start-CrefoExport.ps1
# Orchestrates the Crefo Factoring limit export (daily sync):
#   1. authenticate (OAuth2 password flow, cached token)
#   2. fetch the debitor account list (paginated) and merge into state
#   3. fetch the limit-workflow bulk endpoints (last-limit-decisions +
#      open-limit-desires): accounts with no limit context short-circuit to a
#      0,00 / N row without a per-debtor /risk request
#   4. decide per account whether its stored risk snapshot is stale (new,
#      failed, decision changed, account in the open pipeline, or older than
#      MaxAgeDays) and only then call /risk
#   5. rebuild the full CSV from the account snapshots (stable order by id)
#   6. persist progress after every account so runs are resumable
#
# SyncMode:
#   Incremental (default) - few requests most days; /risk only where the
#     decision changed or the snapshot is older than MaxAgeDays.
#   RefreshAll           - like the original one-time sync: /risk for every
#     account with a limit context each run.
# Run with:  pwsh -File Start-CrefoExport.ps1 [-Reset] [-ForceToken] [-ConfigPath <path>]
# Exit code: 0 = success, 1 = at least one account failed (re-run to retry).
# =============================================================================

[CmdletBinding()]
param(
    [string]$ConfigPath = (Join-Path $PSScriptRoot 'config.psd1'),  # path to config.psd1
    [switch]$Reset,                                                # reprocess all accounts from scratch
    [switch]$ForceToken                                            # ignore cached token, re-authenticate
)

# Fail fast: any unhandled error stops the script instead of continuing blind.
$ErrorActionPreference = 'Stop'

# Modules are imported into the global scope so that functions inside one
# module (e.g. the API module calling the logger or the archive) can see each
# other.
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Logger.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Config.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\StateStore.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Snapshot.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\CsvFormat.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Database.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\ApiArchive.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\CrefoApi.psm1') -Global -Force

# ---------------------------------------------------------------------------
# Configuration loading & merging (defaults, env vars, dirs, validation)
# ---------------------------------------------------------------------------

$script:cfg = Import-CrefoConfig -ConfigPath $ConfigPath

# ---------------------------------------------------------------------------
# Logging setup
# ---------------------------------------------------------------------------

# One log file per run (timestamped) so a run's history is never overwritten.
$script:logFile = Join-Path $script:cfg['LogDir'] ("crefo_export_{0}.log" -f (Get-Date -Format 'yyyyMMdd_HHmmss'))
Initialize-Logger -LogFilePath $script:logFile -Level $script:cfg['LogLevel'] -Console $true

Write-CrefoInfo ("=== Crefo Factoring limit export started ===")
Write-CrefoInfo ("Config      : " + $ConfigPath)
Write-CrefoInfo ("Base URL    : " + $script:cfg['BaseUrl'])
Write-CrefoInfo ("Sync mode   : " + $script:cfg['SyncMode'] + "  (max age " + $script:cfg['MaxAgeDays'] + "d)")
Write-CrefoInfo ("Output CSV  : " + (Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']))
Write-CrefoInfo ("State file  : " + (Join-Path $script:cfg['StateDir'] 'crefo_state.json'))
Write-CrefoInfo ("Log file    : " + $script:logFile)
Write-CrefoInfo ("Archive dir : " + $script:cfg['ArchiveDir'] + "  (enabled=" + $script:cfg['ArchiveRequests'] + ")")

# Persist every API request/response/data exchange to disk (configurable).
Initialize-ApiArchive -Enabled $script:cfg['ArchiveRequests'] -RootDir $script:cfg['ArchiveDir']

# Local SQLite store: canonical source of truth for the CSV. The JSON state is
# still written alongside (rollback), but the CSV is rebuilt from the database.
$script:dbPath = Join-Path $script:cfg['StateDir'] 'crefo.db'
Initialize-CrefoDatabase -DbPath $script:dbPath
Write-CrefoInfo ("Database    : " + $script:dbPath)

# ---------------------------------------------------------------------------
# Authentication
# ---------------------------------------------------------------------------

$script:tokenCachePath = Join-Path $script:cfg['StateDir'] 'crefo_token_cache.json'

# Small wrapper so the token cache path/config only live in one place.
function Get-AppToken {
    param([bool]$Force = $false)
    return Get-CrefoAccessToken -Config $script:cfg -TokenCachePath $script:tokenCachePath -Force:$Force
}

# Obtain a token for this run (reuses the cache unless -ForceToken is set).
$script:token = Get-AppToken -Force:$ForceToken
$tokenSource = if ($ForceToken) { 're-authenticated (forced)' } elseif (Test-Path -LiteralPath $script:tokenCachePath) { 'cached' } else { 'fresh login' }
Write-CrefoInfo ("Access token : " + $tokenSource)

# Called by the API module on a 401: re-authenticate and hand back a fresh
# token. It also updates $script:token so subsequent requests use it too.
$script:authRefresher = {
    Write-CrefoWarn 'Access token is invalid or expired; requesting a fresh token.'
    $script:token = Get-AppToken -Force $true
    return $script:token
}

# ---------------------------------------------------------------------------
# State loading & account discovery
# ---------------------------------------------------------------------------

$statePath = Join-Path $script:cfg['StateDir'] 'crefo_state.json'
$state = Get-CrefoState -Path $statePath

# -Reset: forget all progress and start from an empty CSV.
if ($Reset) {
    Write-CrefoWarn 'Reset requested: resetting all accounts from scratch.'
    Reset-CrefoAccounts -State $state
    $resetCsv = Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']
    if (Test-Path -LiteralPath $resetCsv) { Remove-Item -LiteralPath $resetCsv -Force }
    # Let the database rebuild from the reset run as well.
    Invoke-CrefoSqlite -DbPath $script:dbPath -Sql 'DELETE FROM risk_snapshots; DELETE FROM accounts;' -ErrorAction SilentlyContinue | Out-Null
}

# Refresh the account list (merges new debtors in, keeps old progress) unless
# the config disables it. Always fetched on the very first run.
if ($script:cfg['RefreshAccountList'] -or -not $state.accountListFetchedAt) {
    Write-CrefoInfo 'Fetching debitor account list...'
    $accounts = Get-CrefoAccounts -Config $script:cfg -AccessToken $script:token -PageSize $script:cfg['PageSize'] -AuthRefresher $script:authRefresher
    Write-CrefoInfo ("Found {0} debitor account(s)." -f @($accounts).Count)
    Merge-CrefoAccounts -State $state -Accounts $accounts
    $state.accountListFetchedAt = (Get-Date).ToUniversalTime().ToString('o')
    Save-CrefoState -Path $statePath -State $state
}

# One-time seed: copy accounts/snapshots already known in JSON state into the
# database so the CSV can be rebuilt from it even before any /risk runs.
Import-CrefoDatabaseFromState -State $state

# ---------------------------------------------------------------------------
# Main processing loop (incremental daily sync)
# ---------------------------------------------------------------------------

$allAccounts = @($state.accounts | Sort-Object -Property id)
if ($allAccounts.Count -eq 0) {
    Write-CrefoInfo 'No accounts known - nothing to do.'
    exit 0
}
$pendingCount = @($allAccounts | Where-Object { $_.status -eq 'pending' }).Count
$doneCount = @($allAccounts | Where-Object { $_.status -eq 'done' }).Count
$failedCount0 = @($allAccounts | Where-Object { $_.status -eq 'failed' }).Count
Write-CrefoInfo ("Accounts in scope: {0} total ({1} pending, {2} done, {3} failed)." -f $allAccounts.Count, $pendingCount, $doneCount, $failedCount0)

# Bulk limit-workflow endpoints. Their union defines the accounts with a live
# limit context: only accounts in NEITHER list are safe to short-circuit to a
# 0,00 / N row without a /risk request. If the bulk calls fail we degrade to
# fetching /risk for every account.
$limitDecisions = @{}
$openDesires = @{}
if ($script:cfg['UseLastLimitDecisions']) {
    try {
        Write-CrefoInfo 'Fetching completed limit decisions (bulk call)...'
        foreach ($decision in Get-CrefoLastLimitDecisions -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher) {
            if ($null -ne $decision -and $null -ne $decision.debtorNumber) {
                $limitDecisions[[int]$decision.debtorNumber] = $decision
            }
        }
        Write-CrefoInfo ("Limit decisions available for {0} account(s)." -f $limitDecisions.Count)
    }
    catch {
        Write-CrefoWarn ("Could not fetch last-limit-decisions ({0}); falling back to fetching /risk for every account this run." -f $_.Exception.Message)
        $limitDecisions = @{}
        $openDesires = @{}
        $script:cfg['UseLastLimitDecisions'] = $false
    }
}
if ($script:cfg['UseLastLimitDecisions']) {
    try {
        Write-CrefoInfo 'Fetching open limit desires (bulk call)...'
        foreach ($desire in Get-CrefoOpenLimitDesires -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher) {
            if ($null -ne $desire -and $null -ne $desire.debtorNumber) {
                $openDesires[[int]$desire.debtorNumber] = $desire
            }
        }
        Write-CrefoInfo ("Open limit desires available for {0} account(s)." -f $openDesires.Count)
    }
    catch {
        Write-CrefoWarn ("Could not fetch open-limit-desires ({0}); proceeding without the open-desire refinements." -f $_.Exception.Message)
        $openDesires = @{}
    }
}

$csvPath = Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']
$rows = New-Object System.Collections.Generic.List[string]
$refreshed = 0
$reused = 0
$shortCircuited = 0
$failedCount = 0
$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

foreach ($account in $allAccounts) {
    $id = [int]$account.id
    $snapshotSource = $null   # 'api' | 'short-circuit' when a new snapshot is written
    try {
        $hasDecision = $limitDecisions.ContainsKey($id)
        $decision = if ($hasDecision) { $limitDecisions[$id] } else { $null }
        $inOpenDesires = $openDesires.ContainsKey($id)

        $refreshDecision = Get-RefreshDecision -Cfg $script:cfg -Account $account -HasDecision $hasDecision -Decision $decision -InOpenDesires $inOpenDesires
        if ($refreshDecision.ShouldRefresh) {
            Write-CrefoInfo ("Fetching risk data for debitor {0} ({1}) [{2}]..." -f $id, $account.name, $refreshDecision.Reason)
            $risk = Get-CrefoDebtorRisk -Config $script:cfg -DebtorId $id -AccessToken $script:token -AuthRefresher $script:authRefresher
            $account = Set-AccountSnapshot -Account $account -Risk $risk
            $snapshotSource = 'api'
            $refreshed++
        }
        elseif ($null -eq $account.limitCode) {
            # No decision, not in the open pipeline and no stored snapshot yet:
            # write the explicit N / zero row without a risk request.
            Write-CrefoInfo ("Debitor {0} ({1}) - no limit context, writing zero row [{2}]." -f $id, $account.name, $refreshDecision.Reason)
            $account = Set-AccountSnapshot -Account $account -Risk $null
            $snapshotSource = 'short-circuit'
            $shortCircuited++
        }
        else {
            $reused++
            Write-CrefoInfo ("Reusing snapshot for debitor {0} ({1}) [{2}]: limit {3}, code {4}." -f $id, $account.name, $refreshDecision.Reason, (ConvertTo-GermanyNumber $account.limit), $account.limitCode)
        }

        $rows.Add((New-CsvRowFromAccount -Cfg $script:cfg -Account $account))
        $account.status = 'done'
        $account.error = $null
    }
    catch {
        # Keep the failed account in state as 'failed' so it is retried on the
        # next run. Failed accounts keep their last snapshot but produce no row
        # in the rebuilt CSV until a later run succeeds for them.
        $failedCount++
        $account.status = 'failed'
        $account.error = $_.Exception.Message
        Write-CrefoError ("Debitor {0} failed: {1}" -f $id, $_.Exception.Message)
    }
    $account.updatedAt = (Get-Date).ToUniversalTime().ToString('o')

    # Persist to the SQLite database (canonical store) alongside the JSON state.
    try {
        Save-CrefoAccount -Account $account
        if ($null -ne $snapshotSource) {
            Save-CrefoRiskSnapshot -AccountId $id -Risk ([pscustomobject]@{
                limit     = [double]$account.limit
                purchased = [double]$account.purchased
                balance   = [double]$account.balance
                limitCode = [string]$account.limitCode
                fetchedAt = ([string]$account.fetchedAt)
            }) -Source $snapshotSource -RiskFetched ($snapshotSource -eq 'api')
        }
    }
    catch {
        Write-CrefoWarn ("Database write failed for debitor {0}: {1}" -f $id, $_.Exception.Message)
    }

    # Persist after every single account - this is what makes a Ctrl+C or
    # crashed run resumable without re-fetching the whole book.
    Save-CrefoState -Path $statePath -State $state

    # Be polite to the API between requests (configurable).
    if ($script:cfg['RequestDelayMs'] -gt 0) {
        Start-Sleep -Milliseconds $script:cfg['RequestDelayMs']
    }
}
$stopwatch.Stop()

# Rebuild the complete CSV from the database (the canonical source). Falls back
# to the in-memory rows if the DB read unexpectedly fails.
try {
    $dbRows = Get-CrefoDatabaseCsvRows
    $dbRowLines = @($dbRows | ForEach-Object { New-CsvRowFromAccount -Cfg $script:cfg -Account $_ })
    if ($dbRowLines.Count -gt 0) {
        Write-CrefoCsv -Path $csvPath -Rows $dbRowLines
        Write-CrefoInfo ("CSV rebuilt from database ({0} rows)." -f $dbRowLines.Count)
    }
    else {
        Write-CrefoWarn 'Database returned no CSV rows (empty store); falling back to in-memory rows.'
        Write-CrefoCsv -Path $csvPath -Rows $rows.ToArray()
    }
}
catch {
    Write-CrefoWarn ("Could not rebuild CSV from database ({0}); falling back to in-memory rows." -f $_.Exception.Message)
    Write-CrefoCsv -Path $csvPath -Rows $rows.ToArray()
}

Write-CrefoInfo ("Run finished: total={0} refreshed={1} reused={2} short-circuited={3} failed={4} elapsed={5:N1}s" -f $allAccounts.Count, $refreshed, $reused, $shortCircuited, $failedCount, $stopwatch.Elapsed.TotalSeconds)
if ($failedCount -gt 0) {
    Write-CrefoWarn 'Some accounts failed and are persisted in state. Re-run this script later to retry them.'
    exit 1
}
exit 0