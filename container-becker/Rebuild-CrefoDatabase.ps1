# =============================================================================
# Rebuild-CrefoDatabase.ps1 - rebuild crefo.db + crefo_state.json from the
# archive folder so a subsequent run can continue syncing gaps via the API.
#
# What it does:
#   1. Walks archive/list-debitor/*/data.json to reconstruct the account list
#   2. Walks archive/risks/**/data.json to harvest the latest risk snapshot
#      per debtor (newest run-stamp, then highest sequence number)
#   3. Walks archive/last-limit-decisions/*/data.json and
#      archive/open-limit-desires/*/data.json to rebuild the bulk context
#   4. Writes a fresh crefo_state.json with all accounts marked 'done'
#   5. Initializes crefo.db and seeds it from that state
#
# After this, a normal `Start-CrefoExport.ps1` run will:
#   - probe the API, see the same account count, skip full list sync
#   - refetch /risk only for accounts whose decision changed or snapshot aged out
#
# Usage:
#   pwsh -File Rebuild-CrefoDatabase.ps1 -ArchiveDir container-becker/archive `
#        -StateDir container-becker/state -ConfigPath container-becker/config.psd1
# =============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ArchiveDir,
    [Parameter(Mandatory = $true)][string]$StateDir,
    [string]$ConfigPath = (Join-Path $PSScriptRoot 'config.psd1')
)

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Load helper modules (same path conventions as the main scripts)
# ---------------------------------------------------------------------------
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Import-Module -Name (Join-Path $scriptRoot 'CrefoLib\Logger.psm1') -Global -Force
Import-Module -Name (Join-Path $scriptRoot 'CrefoLib\Config.psm1') -Global -Force
Import-Module -Name (Join-Path $scriptRoot 'CrefoLib\Database\index.psm1') -Global -Force
Import-Module -Name (Join-Path $scriptRoot 'CrefoLib\StateStore.psm1') -Global -Force
Import-Module -Name (Join-Path $scriptRoot 'CrefoLib\Snapshot.psm1') -Global -Force
Import-Module -Name (Join-Path $scriptRoot 'CrefoLib\ApiArchive.psm1') -Global -Force

$cfg = Import-CrefoConfig -ConfigPath $ConfigPath
Initialize-Logger -LogFilePath (Join-Path $cfg['LogDir'] ("rebuild_{0}.log" -f (Get-Date -Format 'yyyyMMdd_HHmmss'))) -Level $cfg['LogLevel'] -Console $true

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
function Get-ArchiveDataFile {
    [CmdletBinding()]
    param([string]$Endpoint)
    $dir = Join-Path $ArchiveDir $Endpoint
    if (-not (Test-Path -LiteralPath $dir)) { return @() }
    return @(Get-ChildItem -LiteralPath $dir -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like '*_data.json' })
}

function Read-JsonFile {
    [CmdletBinding()]
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    try {
        return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    }
    catch {
        Write-CrefoWarn ("Could not read archive file '{0}': {1}" -f $Path, $_.Exception.Message)
        return $null
    }
}

function Parse-RunStampFromPath {
    [CmdletBinding()]
    param([string]$Path)
    if ($Path -match '([^/\\]+)\\[^/\\]+_data\.json$') { return $Matches[1] }
    if ($Path -match '([^/\\]+)/[^/\\]+_data\.json$') { return $Matches[1] }
    return ''
}

function Parse-SequenceFromName {
    [CmdletBinding()]
    param([string]$Name)
    if ($Name -match '_(\d{5})_data\.json$') { return [long]$Matches[1] }
    return 0
}

# ---------------------------------------------------------------------------
# 1. Reconstruct account list from list-debitor archive
# ---------------------------------------------------------------------------
Write-CrefoInfo "=== Rebuilding database from archive ==="
Write-CrefoInfo ("Archive dir : {0}" -f $ArchiveDir)
Write-CrefoInfo ("State dir   : {0}" -f $StateDir)

$accountById = @{}
$latestProbe = $null

$listFiles = Get-ArchiveDataFile -Endpoint 'list-debitor'
Write-CrefoInfo ("Found {0} list-debitor data file(s)." -f $listFiles.Count)

foreach ($f in $listFiles) {
    $data = Read-JsonFile -Path $f.FullName
    if ($null -eq $data -or $null -eq $data.items) { continue }
    foreach ($item in @($data.items)) {
        if ($null -eq $item.id) { continue }
        $id = [int]$item.id
        if (-not $accountById.ContainsKey($id)) {
            $accountById[$id] = [PSCustomObject]@{
                id        = $id
                name      = if ($null -ne $item.name) { [string]$item.name } else { '' }
                status    = 'pending'
                error     = $null
                updatedAt = (Get-Date).ToUniversalTime().ToString('o')
            }
        }
    }
    # Keep the newest probe response (highest sequence number) for stats.
    if ($null -eq $latestProbe -or $data.header) {
        $seq = Parse-SequenceFromName -Name $f.Name
        if ($null -eq $latestProbe.sequence -or $seq -gt $latestProbe.sequence) {
            $latestProbe = [PSCustomObject]@{
                sequence = $seq
                header   = $data.header
            }
        }
    }
}

Write-CrefoInfo ("Reconstructed {0} account(s) from archive." -f $accountById.Count)

# ---------------------------------------------------------------------------
# 2. Harvest latest risk snapshot per debtor from archive/risks
# ---------------------------------------------------------------------------
$riskFiles = Get-ChildItem -LiteralPath $ArchiveDir -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like '*_data.json' -and $_.FullName -match 'archive[\\/]risks[\\/]' }

Write-CrefoInfo ("Found {0} risk data file(s)." -f $riskFiles.Count)

$latestRiskById = @{}
foreach ($f in $riskFiles) {
    $data = Read-JsonFile -Path $f.FullName
    if ($null -eq $data -or $null -eq $data.debtorNumber) { continue }
    $id = [int]$data.debtorNumber
    $runStamp = Parse-RunStampFromPath -Path $f.FullName
    $seq = Parse-SequenceFromName -Name $f.Name
    $key = [string]$id
    if (-not $latestRiskById.ContainsKey($key) -or
        $runStamp -gt $latestRiskById[$key].runStamp -or
        ($runStamp -eq $latestRiskById[$key].runStamp -and $seq -gt $latestRiskById[$key].sequence)) {
        $latestRiskById[$key] = [PSCustomObject]@{
            runStamp   = $runStamp
            sequence   = $seq
            data       = $data
            sourcePath = $f.FullName
        }
    }
}

Write-CrefoInfo ("Harvested latest risk snapshot for {0} debtor(s)." -f $latestRiskById.Count)

# ---------------------------------------------------------------------------
# 3. Harvest bulk limit context from archive
# ---------------------------------------------------------------------------
$decisionByDebtor = @{}
$decisionFiles = Get-ArchiveDataFile -Endpoint 'last-limit-decisions'
foreach ($f in $decisionFiles) {
    $data = Read-JsonFile -Path $f.FullName
    if ($null -eq $data) { continue }
    foreach ($decision in @($data)) {
        if ($null -ne $decision -and $null -ne $decision.debtorNumber) {
            $decisionByDebtor[[int]$decision.debtorNumber] = $decision
        }
    }
}
Write-CrefoInfo ("Loaded limit decisions for {0} account(s) from archive." -f $decisionByDebtor.Count)

$desireByDebtor = @{}
$desireFiles = Get-ArchiveDataFile -Endpoint 'open-limit-desires'
foreach ($f in $desireFiles) {
    $data = Read-JsonFile -Path $f.FullName
    if ($null -eq $data) { continue }
    foreach ($desire in @($data)) {
        if ($null -ne $desire -and $null -ne $desire.debtorNumber) {
            $desireByDebtor[[int]$desire.debtorNumber] = $desire
        }
    }
}
Write-CrefoInfo ("Loaded open limit desires for {0} account(s) from archive." -f $desireByDebtor.Count)

# ---------------------------------------------------------------------------
# 4. Rebuild crefo_state.json
# ---------------------------------------------------------------------------
if (-not (Test-Path -LiteralPath $StateDir)) {
    New-Item -ItemType Directory -Path $StateDir -Force | Out-Null
}
$statePath = Join-Path $StateDir 'crefo_state.json'
$accountsForState = @()
foreach ($acct in $accountById.Values | Sort-Object -Property id) {
    $id = [int]$acct.id
    $risk = $latestRiskById[[string]$id]
    if ($null -ne $risk) {
        $d = $risk.data
        $acct | Add-Member -NotePropertyName limit -NotePropertyValue ([double]$d.limit) -Force
        $acct | Add-Member -NotePropertyName purchased -NotePropertyValue ([double]$d.purchasedReceivables) -Force
        $acct | Add-Member -NotePropertyName balance -NotePropertyValue ([double]$d.balance) -Force
        $code = [string]$d.limitCode
        if ([string]::IsNullOrWhiteSpace($code)) { $code = 'N' }
        $acct | Add-Member -NotePropertyName limitCode -NotePropertyValue $code -Force
        $acct | Add-Member -NotePropertyName riskFetched -NotePropertyValue $true -Force
        $acct | Add-Member -NotePropertyName fetchedAt -NotePropertyValue $risk.runStamp -Force
        $acct.status     = 'done'
    }
    else {
        $acct | Add-Member -NotePropertyName limit -NotePropertyValue 0.0 -Force
        $acct | Add-Member -NotePropertyName purchased -NotePropertyValue 0.0 -Force
        $acct | Add-Member -NotePropertyName balance -NotePropertyValue 0.0 -Force
        $acct | Add-Member -NotePropertyName limitCode -NotePropertyValue 'N' -Force
        $acct | Add-Member -NotePropertyName riskFetched -NotePropertyValue $false -Force
        $acct | Add-Member -NotePropertyName fetchedAt -NotePropertyValue $null -Force
        $acct.status     = 'pending'
    }
    $accountsForState += $acct
}
$state = [PSCustomObject]@{
    version              = 1
    updatedAt            = (Get-Date).ToUniversalTime().ToString('o')
    accountListFetchedAt  = (Get-Date).ToUniversalTime().ToString('o')
    accounts             = $accountsForState
}
$state | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $statePath -Encoding UTF8
Write-CrefoInfo ("Wrote state file: {0} ({1} accounts)." -f $statePath, $accountsForState.Count)

# ---------------------------------------------------------------------------
# 5. Initialize and seed crefo.db
# ---------------------------------------------------------------------------
$dbPath = Join-Path $StateDir 'crefo.db'
if (Test-Path -LiteralPath $dbPath) {
    Remove-Item -LiteralPath $dbPath -Force
}
Initialize-CrefoDatabase -DbPath $dbPath
Write-CrefoInfo ("Initialized database: {0}" -f $dbPath)

foreach ($acct in $accountsForState) {
    Save-CrefoAccount -Account $acct
    $snapshotSource = if ($acct.riskFetched) { 'archive' } else { 'short-circuit' }
    Save-CrefoRiskSnapshot -AccountId $id -Risk ([pscustomobject]@{
        limit     = [double]$acct.limit
        purchased = [double]$acct.purchased
        balance   = [double]$acct.balance
        limitCode = [string]$acct.limitCode
        fetchedAt = [string]$acct.fetchedAt
    }) -Source $snapshotSource -RiskFetched $acct.riskFetched
}
Complete-CrefoDatabase
Write-CrefoInfo "Database seeded."

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
$doneCount    = @($accountsForState | Where-Object { $_.status -eq 'done' }).Count
$pendingCount = @($accountsForState | Where-Object { $_.status -eq 'pending' }).Count
$failedCount  = @($accountsForState | Where-Object { $_.status -eq 'failed' }).Count
$stats = Get-CrefoDatabaseStats

Write-CrefoInfo "=== Rebuild complete ==="
Write-CrefoInfo ("Accounts  : {0} total ({1} done, {2} pending, {3} failed)" -f $accountsForState.Count, $doneCount, $pendingCount, $failedCount)
Write-CrefoInfo ("DB rows   : {0} accounts, {1} snapshots, {2} exchanges" -f $stats.accounts, $stats.snapshots, $stats.exchanges)
Write-CrefoInfo ("State dir : {0}" -f $StateDir)
Write-CrefoInfo ("State file: {0}" -f $statePath)
Write-CrefoInfo ("DB file   : {0}" -f $dbPath)
Write-CrefoInfo "Next step: run Start-CrefoExport.ps1 to continue syncing gaps via the API."
