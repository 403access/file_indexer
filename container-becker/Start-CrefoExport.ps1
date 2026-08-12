# =============================================================================
# Start-CrefoExport.ps1
# Orchestrates the Crefo Factoring limit export (daily sync):
#   1. authenticate (OAuth2 password flow, cached token)
#   2. fetch the debitor account list (paginated) and merge into state; when
#      the database already holds entries, probe the size with pageSize=1 and
#      fetch only the delta instead of a full sync
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
    [switch]$ForceToken,                                           # ignore cached token, re-authenticate
    [string]$RefetchRanges = ''                                    # e.g. "1014,1100-1200"; forces /risk for those debtor ids/ranges
)

# Fail fast: any unhandled error stops the script instead of continuing blind.
$ErrorActionPreference = 'Stop'

# Modules are imported into the global scope so that functions inside one
# module (e.g. the API module calling the logger or the archive) can see each
# other.
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Logger.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Config.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\StateStore.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Snapshot.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\CsvFormat.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Database\index.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\ApiArchive.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\CrefoApi.psm1') -Global -Force

# ---------------------------------------------------------------------------
# Configuration loading & merging (defaults, env vars, dirs, validation)
# ---------------------------------------------------------------------------

$script:cfg = Import-CrefoConfig -ConfigPath $ConfigPath

# Command-line override: -RefetchRanges wins over the config file.
if (-not [string]::IsNullOrWhiteSpace($RefetchRanges)) {
    $script:cfg['RefetchRanges'] = $RefetchRanges
}

# Parse + validate the forced refetch ranges once; they are matched per account
# in the processing loop (empty = no forced refetches this run).
$script:forceRanges = @(ConvertTo-CrefoRefetchRanges -Value $script:cfg['RefetchRanges'])
$script:forceRangesText = @($script:forceRanges | ForEach-Object {
    if ($_.Min -eq $_.Max) { '{0}' -f $_.Min } else { '{0}-{1}' -f $_.Min, $_.Max }
}) -join ', '

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
if ($script:forceRanges.Count -gt 0) {
    Write-CrefoInfo ("Forcing /risk refetch for debtor id range(s): {0}" -f $script:forceRangesText)
}

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

# One-time seed: copy accounts/snapshots already known in JSON state into the
# database so the CSV can be rebuilt from it even before any /risk runs. This
# runs before discovery so the delta-sync decision below can read the database.
Import-CrefoDatabaseFromState -State $state

# Refresh the account list (merges new debtors in, keeps old progress) unless
# the config disables it. Always fetched on the very first run. When the
# database already holds entries we note the highest known id, probe the
# production list with the smallest possible request (pageSize=1) to read the
# total size, and - only when there is a gap - fetch the difference instead of
# a full sync. A broken probe falls back to the full list fetch.
if ($script:cfg['RefreshAccountList'] -or -not $state.accountListFetchedAt) {
    $dbSummary = Get-CrefoDatabaseAccountSummary
    $knownCount = if ($dbSummary -and $null -ne $dbSummary.count) { [int]$dbSummary.count } else { 0 }
    $knownMaxId = if ($dbSummary -and $null -ne $dbSummary.highest_id) { [int]$dbSummary.highest_id } else { $null }
    Write-CrefoInfo ("Debtor list check: database holds {0} known account(s), highest id {1}." -f $knownCount, $knownMaxId)

    if ($knownCount -eq 0) {
        # No database entries yet - retrieve the whole list as always.
        Write-CrefoInfo 'Database has no accounts yet - fetching the full debitor list.'
        $accounts = Get-CrefoAccounts -Config $script:cfg -AccessToken $script:token -PageSize $script:cfg['PageSize'] -AuthRefresher $script:authRefresher
        Write-CrefoInfo ("Found {0} debitor account(s)." -f @($accounts).Count)
    }
    else {
        try {
            # Probe the production size with a pageSize=1 request, compare it to
            # how many accounts we already have, and fetch only the surplus.
            $probe = Get-CrefoDebtorListStats -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher
            $gap = [int]$probe.TotalItems - $knownCount
            if ($gap -le 0) {
                Write-CrefoInfo ("Debtor list unchanged ({0} accounts); keeping cached list, skipping full sync." -f $knownCount)
                $accounts = @()
            }
            else {
                Write-CrefoInfo ("Debtor list grew by {0} (production {1} vs. database {2}); fetching only the difference." -f $gap, $probe.TotalItems, $knownCount)
                $accounts = Get-CrefoAccounts -Config $script:cfg -AccessToken $script:token -PageSize $script:cfg['PageSize'] -AuthRefresher $script:authRefresher -StartIndex $knownCount -MaxCount $gap
                Write-CrefoInfo ("Delta from account list: {0} new account(s)." -f @($accounts).Count)
            }
        }
        catch {
            # Probe or delta failed (server hiccup / weird response): degrade to
            # the safe full-list behaviour rather than missing new debtors.
            Write-CrefoWarn ("Account list probe failed ({0}); falling back to full list sync." -f $_.Exception.Message)
            $accounts = Get-CrefoAccounts -Config $script:cfg -AccessToken $script:token -PageSize $script:cfg['PageSize'] -AuthRefresher $script:authRefresher
            Write-CrefoInfo ("Found {0} debitor account(s)." -f @($accounts).Count)
        }
    }
    Merge-CrefoAccounts -State $state -Accounts $accounts
    $state.accountListFetchedAt = (Get-Date).ToUniversalTime().ToString('o')
    Save-CrefoState -Path $statePath -State $state
}

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

# Detect whether the bulk decisions set changed since the last run. We keep
# the previous set of debtor ids that had completed decisions; an account that
# was previously in that set but is missing now is a "decision removed" case
# and must be re-fetched even when the rest of the set is otherwise stable.
$currentDecisionIds = [System.Collections.Generic.HashSet[int]]::new()
foreach ($id in $limitDecisions.Keys) {
    [void]$currentDecisionIds.Add([int]$id)
}
$previousDecisionIds = [System.Collections.Generic.HashSet[int]]::new()
if ($null -ne $state.decisionsSignature) {
    foreach ($token in ($state.decisionsSignature -split '\|')) {
        if ($token -ne '') { [void]$previousDecisionIds.Add([int]$token) }
    }
}
$newSignature = ($currentDecisionIds | Sort-Object -Unique) -join '|'
$state | Add-Member -NotePropertyName decisionsSignature -NotePropertyValue $newSignature -Force
Write-CrefoDebug ("Decisions: previous={0}, current={1}" -f ($previousDecisionIds -join ','), ($currentDecisionIds -join ','))

$csvPath = Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']
$rows = New-Object System.Collections.Generic.List[string]
$refreshed = 0
$reused = 0
$shortCircuited = 0
$failedCount = 0
$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$stateSaveCounter = 0
$stateSaveInterval = 100

foreach ($account in $allAccounts) {
    $id = [int]$account.id
    $snapshotSource = $null   # 'api' | 'short-circuit' when a new snapshot is written
    try {
        $hasDecision = $limitDecisions.ContainsKey($id)
        $decision = if ($hasDecision) { $limitDecisions[$id] } else { $null }
        $inOpenDesires = $openDesires.ContainsKey($id)

        $accountDecisionRemoved = ($previousDecisionIds.Contains($id) -and -not $currentDecisionIds.Contains($id))
        $refreshDecision = Get-RefreshDecision -Cfg $script:cfg -Account $account -HasDecision $hasDecision -Decision $decision -InOpenDesires $inOpenDesires -DecisionsChanged $accountDecisionRemoved -ForceRanges $script:forceRanges
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
            Write-CrefoDebug ("Reusing snapshot for debitor {0} ({1}) [{2}]: limit {3}, code {4}." -f $id, $account.name, $refreshDecision.Reason, (ConvertTo-GermanyNumber $account.limit), $account.limitCode)
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

    # Persist progress periodically (batched) so a Ctrl+C or crashed run is
    # resumable without re-fetching the whole book, but without the overhead
    # of serializing the full state JSON to disk on every single account.
    $stateSaveCounter++
    if ($stateSaveCounter -ge $stateSaveInterval) {
        Save-CrefoState -Path $statePath -State $state
        $stateSaveCounter = 0
    }

    # Be polite to the API between requests (configurable) — only when we
    # actually made a request. Reused snapshots cost zero network I/O, so
    # sleeping here just burns time for no benefit.
    if ($script:cfg['RequestDelayMs'] -gt 0 -and $null -ne $snapshotSource) {
        Start-Sleep -Milliseconds $script:cfg['RequestDelayMs']
    }
}
$stopwatch.Stop()

# Final state flush (in case the last batch wasn't full).
Save-CrefoState -Path $statePath -State $state

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