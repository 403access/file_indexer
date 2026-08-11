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
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\StateStore.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\ApiArchive.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\CrefoApi.psm1') -Global -Force

# Returns $Default when $Value is null/empty, otherwise $Value.
function Get-Default {
    param($Value, $Default)
    if ($null -eq $Value -or [string]::IsNullOrWhiteSpace([string]$Value)) { return $Default }
    return $Value
}

# Escapes a value for a ';'-separated CSV: quotes fields containing ';', a
# quote, or a newline, and doubles embedded quotes.
function ConvertTo-CsvField {
    param([string]$Value)
    if ($Value -match '[;"\r\n]') {
        return ('"{0}"' -f ($Value -replace '"', '""'))
    }
    return $Value
}

# Formats a decimal German-style (comma as decimal separator, 2 places), e.g. 0,00.
function ConvertTo-GermanyNumber {
    param([object]$Value)
    if ($null -eq $Value) { return '0,00' }
    try {
        $double = [double]$Value
        return $double.ToString('F2', [System.Globalization.CultureInfo]::InvariantCulture) -replace '\.', ','
    }
    catch {
        return '0,00'
    }
}

# Adds/updates a field on an account object. State accounts are deserialized
# JSON (or literal PSCustomObject) and only accept assignment to existing
# properties; new snapshot fields must be added as note properties.
function Set-AccountField {
    [CmdletBinding()]
    param(
        [object]$Account,
        [string]$Name,
        [object]$Value
    )
    $Account | Add-Member -NotePropertyName $Name -NotePropertyValue $Value -Force
}

# Stores the last known risk values onto the account object so a CSV row can be
# re-emitted later without a network call. With -Risk $null (the short-circuit
# path) the account is stamped as the explicit N / zero snapshot.
function Set-AccountSnapshot {
    [CmdletBinding()]
    param(
        [object]$Account,
        [object]$Risk
    )
    if ($null -ne $Risk) {
        Set-AccountField -Account $Account -Name 'limit' -Value ([double]$Risk.limit)
        Set-AccountField -Account $Account -Name 'purchased' -Value ([double]$Risk.purchasedReceivables)
        Set-AccountField -Account $Account -Name 'balance' -Value ([double]$Risk.balance)
        $code = [string]$Risk.limitCode
        if ([string]::IsNullOrWhiteSpace($code)) { $code = 'N' }
        Set-AccountField -Account $Account -Name 'limitCode' -Value $code
        Set-AccountField -Account $Account -Name 'riskFetched' -Value $true
        Set-AccountField -Account $Account -Name 'fetchedAt' -Value ((Get-Date).ToUniversalTime().ToString('o'))
    }
    else {
        # No risk request made (short-circuit): explicit N / zero snapshot.
        Set-AccountField -Account $Account -Name 'limit' -Value 0.0
        Set-AccountField -Account $Account -Name 'purchased' -Value 0.0
        Set-AccountField -Account $Account -Name 'balance' -Value 0.0
        Set-AccountField -Account $Account -Name 'limitCode' -Value 'N'
        Set-AccountField -Account $Account -Name 'riskFetched' -Value $false
    }
    return $Account
}

# Builds one CSV data row from an account's stored snapshot:
#   Kto-Nr. | Name1 | Limit | LimitKennz | Gekauft | freie Linie
# freie Linie is derived from limit minus purchased (or minus balance when
# FreeLineFromBalance is enabled), never fetched.
function New-CsvRowFromAccount {
    param(
        [hashtable]$Cfg,
        [object]$Account
    )
    $limit = [double]$Account.limit
    $purchased = [double]$Account.purchased
    $balance = [double]$Account.balance
    $freeBase = if ($Cfg['FreeLineFromBalance']) { $balance } else { $purchased }
    $freeLine = $limit - $freeBase

    $fields = @(
        (ConvertTo-CsvField ([string]$Account.id)),
        (ConvertTo-CsvField ([string]$Account.name)),
        (ConvertTo-GermanyNumber $limit),
        (ConvertTo-CsvField ([string]$Account.limitCode)),
        (ConvertTo-GermanyNumber $purchased),
        (ConvertTo-GermanyNumber $freeLine)
    )
    return ($fields -join ';')
}

# Writes a complete CSV (header + all rows) atomically via a temp file.
function Write-CrefoCsv {
    param(
        [string]$Path,
        [string[]]$Rows
    )
    $utf8WithBom = New-Object System.Text.UTF8Encoding($true)
    $header = 'Kto-Nr.;Name1;Limit;LimitKennz;Gekauft;freie Linie'
    $tmp = $Path + '.tmp'
    [System.IO.File]::WriteAllLines($tmp, @($header) + @($Rows), $utf8WithBom)
    Move-Item -LiteralPath $tmp -Destination $Path -Force
}

# Decides whether the account's /risk snapshot must be re-fetched for this run.
# Returns [pscustomobject]@{ ShouldRefresh, Reason } so the caller can log why.
function Get-RefreshDecision {
    [CmdletBinding()]
    param(
        [hashtable]$Cfg,
        [object]$Account,
        [bool]$HasDecision,
        [object]$Decision,
        [bool]$InOpenDesires
    )
    $D = {
        param([bool]$Refresh, [string]$Reason)
        return [pscustomobject]@{ ShouldRefresh = $Refresh; Reason = $Reason }
    }

    # New or previously failed accounts are always fetched.
    if ($Account.status -in @('pending', 'failed')) {
        return & $D $true ("status '{0}'" -f $Account.status)
    }

    # Accounts from a run before the snapshot feature have no stored data yet:
    # fetch only when they have a live limit context, otherwise they fall into
    # the short-circuit path below.
    if ($null -eq $Account.limitCode) {
        if ($HasDecision -or $InOpenDesires) {
            return & $D $true 'fresh account, live limit context (decision or open desire)'
        }
        return & $D $false 'fresh account, no limit context (short-circuit row)'
    }

    # RefreshAll: refetch every account with a limit context each run.
    if ($Cfg['SyncMode'] -eq 'RefreshAll') {
        if ($HasDecision -or $InOpenDesires) {
            return & $D $true 'RefreshAll mode, account has limit context'
        }
        return & $D $false 'RefreshAll mode, no limit context (short-circuit row)'
    }

    # --- Incremental mode below -------------------------------------------------
    $storedCode = [string]$Account.limitCode
    $storedLimit = [double]$Account.limit
    if ([string]::IsNullOrWhiteSpace($storedCode)) { $storedCode = 'N' }
    $storedActive = ($storedCode -ne 'N') -or ($storedLimit -gt 0.0)

    # Decision removed while the account previously had one: refetch, because
    # purchases may still exist and only /risk knows.
    if (-not $HasDecision -and -not $InOpenDesires -and $storedActive) {
        return & $D $true 'completed decision removed, account previously had a limit'
    }

    if ($HasDecision) {
        # Limit/decision changed since our last snapshot?
        $decisionCode = [string]$Decision.limitCode
        if ([string]::IsNullOrWhiteSpace($decisionCode)) { $decisionCode = 'N' }
        $decisionLimit = [double]$Decision.currentLimit
        if ($storedCode -ne $decisionCode) {
            return & $D $true ("limit code changed ({0} -> {1})" -f $storedCode, $decisionCode)
        }
        if ([math]::Abs($storedLimit - $decisionLimit) -gt 0.001) {
            return & $D $true ("current limit changed ({0:N2} -> {1:N2})" -f $storedLimit, $decisionLimit)
        }
    }
    else {
        # No decision, but the account sits in the open-limit pipeline: refresh.
        if ($InOpenDesires) {
            return & $D $true 'no completed decision, but account is in the open limit pipeline'
        }
    }

    # Staleness cap: refetch everything past MaxAgeDays regardless of changes.
    $maxAgeDays = [int]$Cfg['MaxAgeDays']
    if ($maxAgeDays -gt 0 -and $Account.riskFetched) {
        try {
            $fetchedUtc = [datetime]$Account.fetchedAt
            if ($fetchedUtc.Kind -ne 'Utc') { $fetchedUtc = $fetchedUtc.ToUniversalTime() }
            $age = (Get-Date).ToUniversalTime() - $fetchedUtc
            if ($age.TotalDays -ge $maxAgeDays) {
                return & $D $true ("snapshot older than MaxAgeDays ({0:N1} days)" -f $age.TotalDays)
            }
        }
        catch { }
    }
    return & $D $false 'snapshot fresh, decision unchanged'
}

# Boolean convenience wrapper (returns just the fetch/no-fetch decision).
function Test-ShouldRefreshRisk {
    [CmdletBinding()]
    param(
        [hashtable]$Cfg,
        [object]$Account,
        [bool]$HasDecision,
        [object]$Decision,
        [bool]$InOpenDesires
    )
    return (Get-RefreshDecision -Cfg $Cfg -Account $Account -HasDecision $HasDecision -Decision $Decision -InOpenDesires $InOpenDesires).ShouldRefresh
}

# ---------------------------------------------------------------------------
# Configuration loading & merging
# ---------------------------------------------------------------------------

if (-not (Test-Path -LiteralPath $ConfigPath)) {
    throw ("Configuration not found: '{0}'. Copy 'config.example.psd1' to 'config.psd1' and fill in your credentials." -f $ConfigPath)
}
$script:cfg = Import-PowerShellDataFile -LiteralPath $ConfigPath
$cfgRoot = Split-Path -Parent (Resolve-Path $ConfigPath)

# Fill in sane defaults for keys the config file may omit.
$script:cfg['PageSize'] = [int](Get-Default $script:cfg['PageSize'] 50)
$script:cfg['MaxRetries'] = [int](Get-Default $script:cfg['MaxRetries'] 5)
$script:cfg['RequestDelayMs'] = [int](Get-Default $script:cfg['RequestDelayMs'] 200)
$script:cfg['LogLevel'] = [string](Get-Default $script:cfg['LogLevel'] 'INFO')
$script:cfg['RefreshAccountList'] = [bool](Get-Default $script:cfg['RefreshAccountList'] $true)
$script:cfg['FreeLineFromBalance'] = [bool](Get-Default $script:cfg['FreeLineFromBalance'] $false)
$script:cfg['UseLastLimitDecisions'] = [bool](Get-Default $script:cfg['UseLastLimitDecisions'] $true)
$script:cfg['ArchiveRequests'] = [bool](Get-Default $script:cfg['ArchiveRequests'] $true)
$script:cfg['SyncMode'] = [string](Get-Default $script:cfg['SyncMode'] 'Incremental')
$script:cfg['MaxAgeDays'] = [int](Get-Default $script:cfg['MaxAgeDays'] 7)
$script:cfg['OutputFileName'] = [string](Get-Default $script:cfg['OutputFileName'] 'crefo_limits.csv')

if ($script:cfg['SyncMode'] -notin @('Incremental', 'RefreshAll')) {
    throw ("Invalid SyncMode '{0}'. Use 'Incremental' or 'RefreshAll'." -f $script:cfg['SyncMode'])
}

# Environment variables are optional but win over the config file (handy in CI).
$envMap = @{
    Username     = 'CREFO_USERNAME'
    Password     = 'CREFO_PASSWORD'
    ClientId     = 'CREFO_CLIENT_ID'
    ClientSecret = 'CREFO_CLIENT_SECRET'
    BaseUrl      = 'CREFO_BASE_URL'
    ObligoNumber = 'CREFO_OBLIGO'
}
foreach ($key in $envMap.Keys) {
    if ([string]::IsNullOrWhiteSpace([string]$script:cfg[$key])) {
        $script:cfg[$key] = [Environment]::GetEnvironmentVariable($envMap[$key])
    }
}

# Resolve relative directory entries against the config file's location and
# make sure the directories exist before we write anything into them.
foreach ($dirKey in @('OutputDir', 'StateDir', 'LogDir', 'ArchiveDir')) {
    if ([string]::IsNullOrWhiteSpace([string]$script:cfg[$dirKey])) { $script:cfg[$dirKey] = $dirKey }
    if (-not [System.IO.Path]::IsPathRooted([string]$script:cfg[$dirKey])) {
        $script:cfg[$dirKey] = Join-Path $cfgRoot $script:cfg[$dirKey]
    }
    if (-not (Test-Path -LiteralPath $script:cfg[$dirKey])) {
        New-Item -ItemType Directory -Path $script:cfg[$dirKey] -Force | Out-Null
    }
}

# The credential fields are mandatory; fail with a helpful message otherwise.
$required = @('BaseUrl', 'Username', 'Password', 'ClientId', 'ClientSecret')
$missing = @($required | Where-Object { [string]::IsNullOrWhiteSpace([string]$script:cfg[$_]) })
if ($missing.Count -gt 0) {
    throw ("Missing required configuration values: {0}. Provide them in '{1}' or via environment variables." -f ($missing -join ', '), $ConfigPath)
}

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
    try {
        $hasDecision = $limitDecisions.ContainsKey($id)
        $decision = if ($hasDecision) { $limitDecisions[$id] } else { $null }
        $inOpenDesires = $openDesires.ContainsKey($id)

        $refreshDecision = Get-RefreshDecision -Cfg $script:cfg -Account $account -HasDecision $hasDecision -Decision $decision -InOpenDesires $inOpenDesires
        if ($refreshDecision.ShouldRefresh) {
            Write-CrefoInfo ("Fetching risk data for debitor {0} ({1}) [{2}]..." -f $id, $account.name, $refreshDecision.Reason)
            $risk = Get-CrefoDebtorRisk -Config $script:cfg -DebtorId $id -AccessToken $script:token -AuthRefresher $script:authRefresher
            $account = Set-AccountSnapshot -Account $account -Risk $risk
            $refreshed++
        }
        elseif ($null -eq $account.limitCode) {
            # No decision, not in the open pipeline and no stored snapshot yet:
            # write the explicit N / zero row without a risk request.
            Write-CrefoInfo ("Debitor {0} ({1}) - no limit context, writing zero row [{2}]." -f $id, $account.name, $refreshDecision.Reason)
            $account = Set-AccountSnapshot -Account $account -Risk $null
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
    # Persist after every single account - this is what makes a Ctrl+C or
    # crashed run resumable without re-fetching the whole book.
    Save-CrefoState -Path $statePath -State $state

    # Be polite to the API between requests (configurable).
    if ($script:cfg['RequestDelayMs'] -gt 0) {
        Start-Sleep -Milliseconds $script:cfg['RequestDelayMs']
    }
}
$stopwatch.Stop()

# Rebuild the complete CSV from the stored snapshots (stable order by id).
Write-CrefoCsv -Path $csvPath -Rows $rows.ToArray()

Write-CrefoInfo ("Run finished: total={0} refreshed={1} reused={2} short-circuited={3} failed={4} elapsed={5:N1}s" -f $allAccounts.Count, $refreshed, $reused, $shortCircuited, $failedCount, $stopwatch.Elapsed.TotalSeconds)
if ($failedCount -gt 0) {
    Write-CrefoWarn 'Some accounts failed and are persisted in state. Re-run this script later to retry them.'
    exit 1
}
exit 0