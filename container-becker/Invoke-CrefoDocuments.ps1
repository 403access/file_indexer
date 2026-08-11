# =============================================================================
# Invoke-CrefoDocuments.ps1
# Retrieves (lists + downloads) the Crefo Factoring documents for the
# authenticated obligo/debtor context into the configured DocumentsDir:
#
#   1. authenticate (OAuth2 password flow, cached token)
#   2. list the submission documents (GET /Submission/list-document, unread=false
#      so nothing is hidden behind the API's "marked as read" flag) and download
#      each file to <DocumentsDir>/submission/<name>
#   3. list the available document folders (GET /Documents/list-directory) and,
#      for every folder, list + download its files to <DocumentsDir>/<folder>/<name>
#   4. persist a small index (sheet | name | size) in StateDir so unchanged
#      files are skipped on subsequent runs (incremental, resumable)
#
# Downloads are written to a temporary <name>.part file and renamed only after
# the transfer completed, so a crashed/interrupted run never leaves a partial
# file that a later run would mistake for a complete download.
#
# Run with: pwsh -File Invoke-CrefoDocuments.ps1 [-Reset] [-ForceToken] [-ConfigPath <path>]
# Exit code: 0 = success, 1 = at least one document failed (re-run to retry).
# =============================================================================

[CmdletBinding()]
param(
    [string]$ConfigPath = (Join-Path $PSScriptRoot 'config.psd1'),  # path to config.psd1
    [switch]$Reset,                                                # forget the download index, re-download everything
    [switch]$ForceToken                                            # ignore cached token, re-authenticate
)

# Fail fast: any unhandled error stops the script instead of continuing blind.
$ErrorActionPreference = 'Stop'

# Same module set as the exporter: the API module calls the logger/archive, so
# they must be importable from any module scope.
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Logger.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Config.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\CsvFormat.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Database.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\ApiArchive.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\CrefoApi.psm1') -Global -Force

# ---------------------------------------------------------------------------
# Configuration loading & logging setup
# ---------------------------------------------------------------------------

$script:cfg = Import-CrefoConfig -ConfigPath $ConfigPath

# One log file per run (timestamped) so a run's history is never overwritten.
$script:logFile = Join-Path $script:cfg['LogDir'] ("crefo_documents_{0}.log" -f (Get-Date -Format 'yyyyMMdd_HHmmss'))
Initialize-Logger -LogFilePath $script:logFile -Level $script:cfg['LogLevel'] -Console $true

Write-CrefoInfo ("=== Crefo Factoring documents retrieval started ===")
Write-CrefoInfo ("Config       : " + $ConfigPath)
Write-CrefoInfo ("Base URL     : " + $script:cfg['BaseUrl'])
Write-CrefoInfo ("Documents dir: " + $script:cfg['DocumentsDir'])
Write-CrefoInfo ("State file   : " + (Join-Path $script:cfg['StateDir'] 'crefo_documents_state.json'))
Write-CrefoInfo ("Log file     : " + $script:logFile)
Write-CrefoInfo ("Archive dir  : " + $script:cfg['ArchiveDir'] + "  (enabled=" + $script:cfg['ArchiveRequests'] + ")")

# Persist every API request/response/data exchange to disk (configurable).
Initialize-ApiArchive -Enabled $script:cfg['ArchiveRequests'] -RootDir $script:cfg['ArchiveDir']

# Local SQLite store: keeps the api_exchanges audit log that archives ride on.
Initialize-CrefoDatabase -DbPath (Join-Path $script:cfg['StateDir'] 'crefo.db')

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
# Download index (incremental skip of unchanged files)
# ---------------------------------------------------------------------------

$statePath = Join-Path $script:cfg['StateDir'] 'crefo_documents_state.json'

# The index maps "<sheet>/<name>" -> { sheet; name; size; downloadedAt } where
# "sheet" is the local subfolder (e.g. 'submission' or a Documents folder name).
function Get-DocumentIndex {
    if (Test-Path -LiteralPath $statePath) {
        try {
            $index = Get-Content -LiteralPath $statePath -Raw -Encoding UTF8 | ConvertFrom-Json
            if ($null -eq $index.downloaded) {
                # Backfill the expected shape when loaded from a fresh file.
                $index | Add-Member -NotePropertyName downloaded -NotePropertyValue @{} -Force
            }
            else {
                # ConvertFrom-Json turns the "downloaded" object back into a
                # PSCustomObject; restore a plain hashtable so entries can still
                # be added with $index.downloaded[$key] = ... on this run.
                $table = @{}
                foreach ($p in $index.downloaded.PSObject.Properties) { $table[$p.Name] = $p.Value }
                $index.downloaded = $table
            }
            return $index
        }
        catch {
            Write-CrefoWarn ("Document index '{0}' could not be read, starting fresh: {1}" -f $statePath, $_.Exception.Message)
        }
    }
    return [PSCustomObject]@{ version = 1; updatedAt = $null; downloaded = @{} }
}

function Save-DocumentIndex {
    param([object]$Index)
    $Index.updatedAt = (Get-Date).ToUniversalTime().ToString('o')
    $Index | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $statePath -Encoding UTF8
}

if ($Reset) {
    Write-CrefoWarn 'Reset requested: clearing the document download index.'
    $script:docIndex = Get-DocumentIndex
    $script:docIndex.downloaded = @{}
    Save-DocumentIndex -Index $script:docIndex
}
else {
    $script:docIndex = Get-DocumentIndex
}

# ---------------------------------------------------------------------------
# Download helpers
# ---------------------------------------------------------------------------

# Never trust an API-provided file name for the local path: replace every
# character outside a safe filename set and block '.'/'..'-style traversal.
function ConvertTo-SafeDocumentName {
    param([string]$Value)
    $safe = [string]$Value -replace '[^A-Za-z0-9._ -]', '_'
    $safe = $safe -replace '\.\.', '_'
    $safe = $safe -replace '^\.', '_' -replace '\.$', '_'
    if ([string]::IsNullOrWhiteSpace($safe)) { $safe = '_' }
    return $safe
}

# Downloads one document into <DocumentsDir>/<sheet>/<name>, skipping it when an
# identical (same name + size) record already exists. $Fetcher is a scriptblock
# taking -Name/-OutFile that performs the actual API download (one per family).
# Returns $true when the file was actually downloaded, $false when it was a skip.
function Receive-CrefoDocument {
    param(
        [string]$Sheet,                     # local subfolder under DocumentsDir
        [string]$Name,                      # API file name
        [int]$Size,                         # file size reported by the API listing
        [scriptblock]$Fetcher               # & $Fetcher -Name $Name -OutFile <path>
    )
    $safeName = ConvertTo-SafeDocumentName -Value $Name
    $sheetDir = Join-Path $script:cfg['DocumentsDir'] $Sheet
    $target = Join-Path $sheetDir $safeName
    $key = '{0}/{1}' -f $Sheet, $Name
    $known = $script:docIndex.downloaded.$key

    # Nothing changed since the last run -> skip the transfer entirely.
    if ($null -ne $known -and [int]$known.size -eq $Size -and (Test-Path -LiteralPath $target)) {
        Write-CrefoInfo ("Already downloaded {0} ({1} bytes); skipping." -f $key, $Size)
        return $false
    }

    # Download to a <name>.part file first, then rename once it is complete -
    # a partial file must never be mistaken for a finished download.
    $part = Join-Path $sheetDir ($safeName + '.part')
    Remove-Item -LiteralPath $part -Force -ErrorAction SilentlyContinue
    $bytes = & $Fetcher -Name $Name -OutFile $part
    if (-not (Test-Path -LiteralPath $part)) {
        throw ("Download of {0} produced no output file." -f $key)
    }
    Move-Item -LiteralPath $part -Destination $target -Force

    $script:docIndex.downloaded[$key] = [pscustomobject]@{
        sheet        = $Sheet
        name         = $Name
        size         = [int]$Size
        downloadedAt = (Get-Date).ToUniversalTime().ToString('o')
    }
    Write-CrefoInfo ("Downloaded {0} ({1} bytes)." -f $key, $bytes)
    # Persist after every document so a crashed run stays resumable.
    Save-DocumentIndex -Index $script:docIndex
    return $true
}

# Walks all pages of a paginated document listing and downloads every item.
# $List is a scriptblock taking -Page/-PageSize and returning { header, items }.
# Per-document failures are reported and counted, they do not abort the sheet.
function Receive-CrefoDocumentSheet {
    param(
        [string]$Sheet,                     # local subfolder under DocumentsDir
        [scriptblock]$List,                 # & $List -Page $p -PageSize $ps -> { header, items }
        [scriptblock]$Fetcher               # per-item fetcher (see Receive-CrefoDocument)
    )
    $page = 1
    $totalCount = 0
    $downloaded = 0
    while ($true) {
        $pageResult = & $List -Page $page -PageSize 50
        $items = @($pageResult.items)
        foreach ($item in $items) {
            $name = [string]$item.name
            if ([string]::IsNullOrWhiteSpace($name)) { continue }
            $size = if ($null -ne $item.PSObject.Properties['size']) { [int]$item.size } else { 0 }
            $totalCount++
            try {
                if (Receive-CrefoDocument -Sheet $Sheet -Name $name -Size $size -Fetcher $Fetcher) { $downloaded++ }
            }
            catch {
                $script:failedDocs++
                Write-CrefoError ("Document {0}/{1} failed: {2}" -f $Sheet, $name, $_.Exception.Message)
            }
        }
        # Last page reached when totalPages runs out or a page came back short.
        $totalPages = if ($null -ne $pageResult.header -and $null -ne $pageResult.header.totalPages) { [int]$pageResult.header.totalPages } else { $page }
        $page++
        if ($page -gt $totalPages) { break }
        if ($items.Count -eq 0) { break }
        if ($page -gt 1000) {   # safety cap: never loop forever on a broken API
            Write-CrefoWarn ("Document listing for '{0}' hit the page safety cap; stopping." -f $Sheet)
            break
        }
    }
    Write-CrefoInfo ("Sheet '{0}': {1} file(s), {2} downloaded." -f $Sheet, $totalCount, $downloaded)
    return $downloaded
}

# ---------------------------------------------------------------------------
# Main flow
# ---------------------------------------------------------------------------

$script:failedDocs = 0
$script:totalDownloaded = 0

# Fetcher for the submission family. Config/token/auth live in $script: scope,
# so a scriptblock created here (script scope) can reach them from anywhere.
$submissionFetcher = {
    param([string]$Name, [string]$OutFile)
    Get-CrefoSubmissionDocument -Config $script:cfg -AccessToken $script:token `
        -AuthRefresher $script:authRefresher -Document $Name -OutFile $OutFile
}
$submissionList = {
    param([int]$Page, [int]$PageSize)
    Get-CrefoSubmissionDocuments -Config $script:cfg -AccessToken $script:token `
        -AuthRefresher $script:authRefresher -Unread $false -Page $Page -PageSize $PageSize
}

# 1. Submission documents (the per-obligo submission folder).
try {
    Write-CrefoInfo 'Listing submission documents...'
    $script:totalDownloaded += (Receive-CrefoDocumentSheet -Sheet 'submission' -List $submissionList -Fetcher $submissionFetcher)
}
catch {
    $script:failedDocs++
    Write-CrefoError ("Submission document retrieval failed: {0}" -f $_.Exception.Message)
    # The submission folder may simply not exist for this obligo (404): that is
    # not fatal, the generic folders below still run.
    if ($_.Exception.Message -notmatch '404') { throw }
}

# 2. Generic Documents folders (e.g. submissions/reminders/aob).
try {
    Write-CrefoInfo 'Listing available document folders...'
    $foldersResp = Get-CrefoDocumentDirectories -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher
    $folders = @($foldersResp.folder | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
    Write-CrefoInfo ("Document folders from API: {0}." -f ($folders -join ', '))

    foreach ($folder in $folders) {
        $sheet = ConvertTo-SafeDocumentName -Value ([string]$folder)
        # $folder is a script-scope variable (foreach at top level), so the
        # scriptblocks below can close over it without any $using: tricks.
        $folderList = {
            param([int]$Page, [int]$PageSize)
            Get-CrefoDocumentList -Config $script:cfg -AccessToken $script:token `
                -AuthRefresher $script:authRefresher -Directory $folder -Unread $false -Page $Page -PageSize $PageSize
        }
        $folderFetcher = {
            param([string]$Name, [string]$OutFile)
            Get-CrefoDocumentDownload -Config $script:cfg -AccessToken $script:token `
                -AuthRefresher $script:authRefresher -Directory $folder -Document $Name -OutFile $OutFile
        }
        $script:totalDownloaded += (Receive-CrefoDocumentSheet -Sheet $sheet -List $folderList -Fetcher $folderFetcher)
    }
}
catch {
    $script:failedDocs++
    Write-CrefoError ("Document folder retrieval failed: {0}" -f $_.Exception.Message)
    # If nothing could be downloaded at all this is a hard failure; if some
    # sheets already made it, keep what we have and still report the failure.
    if ($script:totalDownloaded -eq 0) { throw }
    Write-CrefoWarn 'Some folders failed, but previously downloaded documents are kept.'
}

Write-CrefoInfo ("Documents retrieval finished: downloaded={0} failed={1}." -f $script:totalDownloaded, $script:failedDocs)
if ($script:failedDocs -gt 0) {
    Write-CrefoWarn 'Some documents failed; re-run this script later to retry them.'
    exit 1
}
exit 0