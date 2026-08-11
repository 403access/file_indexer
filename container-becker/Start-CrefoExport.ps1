# =============================================================================
# Start-CrefoExport.ps1
# Orchestrates the Crefo Factoring limit export:
#   1. authenticate (OAuth2 password flow, cached token)
#   2. fetch the debitor account list (paginated)
#   3. per debtor: fetch risk data and append one CSV row
#   4. persist progress after every account so runs are resumable
# Run with:  pwsh -File Start-CrefoExport.ps1 [-Reset] [-ForceToken] [-ConfigPath <path>]
# Exit code: 0 = success, 1 = at least one account failed (re-run to retry).
# =============================================================================

[CmdletBinding()]
param(
    [string]$ConfigPath = (Join-Path $PSScriptRoot 'config.psd1'),  # path to config.psd1
    [switch]$Reset,                                                # reprocess all accounts, truncate CSV
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

# Builds one CSV data row from the risk payload:
#   Kto-Nr. | Name1 | Limit | LimitKennz | Gekauft | freie Linie
# freie Linie is not returned by the API and is derived from limit minus the
# purchased receivables (or minus balance when FreeLineFromBalance is enabled).
function New-CsvRow {
    param(
        [hashtable]$Cfg,
        [int]$Id,
        [string]$Name,
        [object]$Risk
    )
    $limit = 0.0
    $purchased = 0.0
    $balance = 0.0
    $limitCode = ''
    if ($null -ne $Risk) {
        $limit = [double]$Risk.limit
        $purchased = [double]$Risk.purchasedReceivables
        $balance = [double]$Risk.balance
        $limitCode = [string]$Risk.limitCode
    }
    $freeBase = if ($Cfg['FreeLineFromBalance']) { $balance } else { $purchased }
    $freeLine = $limit - $freeBase

    $fields = @(
        (ConvertTo-CsvField ([string]$Id)),
        (ConvertTo-CsvField ([string]$Name)),
        (ConvertTo-GermanyNumber $limit),
        (ConvertTo-CsvField $limitCode),
        (ConvertTo-GermanyNumber $purchased),
        (ConvertTo-GermanyNumber $freeLine)
    )
    return ($fields -join ';')
}

# Appends rows to the CSV. Writes the header (with a BOM so Excel detects UTF-8)
# only when the file is created for the first time.
function Add-CsvLine {
    param(
        [string]$Path,
        [string[]]$Lines
    )
    $utf8WithBom = New-Object System.Text.UTF8Encoding($true)
    if (-not (Test-Path -LiteralPath $Path)) {
        $header = 'Kto-Nr.;Name1;Limit;LimitKennz;Gekauft;freie Linie'
        [System.IO.File]::WriteAllLines($Path, @($header) + @($Lines), $utf8WithBom)
    }
    else {
        [System.IO.File]::AppendAllLines($Path, $Lines, $utf8WithBom)
    }
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
$script:cfg['ArchiveRequests'] = [bool](Get-Default $script:cfg['ArchiveRequests'] $true)
$script:cfg['OutputFileName'] = [string](Get-Default $script:cfg['OutputFileName'] 'crefo_limits.csv')

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
    Write-CrefoWarn 'Reset requested: resetting all accounts and truncating the output CSV.'
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
# Main processing loop
# ---------------------------------------------------------------------------

$toProcess = @(Get-CrefoPendingAccounts -State $state)
if ($toProcess.Count -eq 0) {
    Write-CrefoInfo 'Nothing to process - all accounts already done.'
    exit 0
}

Write-CrefoInfo ("Processing {0} account(s)..." -f $toProcess.Count)
$csvPath = Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']
$succeeded = 0
$failed = 0
$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

foreach ($account in $toProcess) {
    $id = [int]$account.id
    try {
        Write-CrefoInfo ("Fetching risk data for debitor {0} ({1})..." -f $id, $account.name)
        $risk = Get-CrefoDebtorRisk -Config $script:cfg -DebtorId $id -AccessToken $script:token -AuthRefresher $script:authRefresher
        $row = New-CsvRow -Cfg $script:cfg -Id $id -Name $account.name -Risk $risk
        Add-CsvLine -Path $csvPath -Lines @($row)
        Write-CrefoInfo ("Debitor {0} processed." -f $id)
        $account.status = 'done'
        $account.error = $null
        $succeeded++
    }
    catch {
        # Keep the failed account in state as 'failed' so it is retried on the
        # next run instead of being lost or duplicated.
        $failed++
        $account.status = 'failed'
        $account.error = $_.Exception.Message
        Write-CrefoError ("Debitor {0} failed: {1}" -f $id, $_.Exception.Message)
    }
    $account.updatedAt = (Get-Date).ToUniversalTime().ToString('o')
    # Persist after every single account - this is what makes a Ctrl+C or
    # crashed run resumable with no duplicate CSV rows.
    Save-CrefoState -Path $statePath -State $state

    # Be polite to the API between requests (configurable).
    if ($script:cfg['RequestDelayMs'] -gt 0) {
        Start-Sleep -Milliseconds $script:cfg['RequestDelayMs']
    }
}
$stopwatch.Stop()

Write-CrefoInfo ("Run finished: total={0} succeeded={1} failed={2} elapsed={3:N1}s" -f $toProcess.Count, $succeeded, $failed, $stopwatch.Elapsed.TotalSeconds)
if ($failed -gt 0) {
    Write-CrefoWarn 'Some accounts failed and are persisted in state. Re-run this script later to retry them.'
    exit 1
}
exit 0