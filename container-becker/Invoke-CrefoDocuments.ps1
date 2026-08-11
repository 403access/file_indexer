# =============================================================================
# Invoke-CrefoDocuments.ps1 - thin wrapper around the Documents module.
# Handles parameter binding, config/logging/archive/database setup, and the
# final exit code; the actual listing/download orchestration lives in
# CrefoLib/Documents/Documents.ps1 so it can be unit-tested in isolation.
# =============================================================================

[CmdletBinding()]
param(
    [string]$ConfigPath = (Join-Path $PSScriptRoot 'config.psd1'),
    [switch]$Reset,
    [switch]$ForceToken
)

$ErrorActionPreference = 'Stop'

Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Logger.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Config.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\CsvFormat.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Database.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\ApiArchive.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\CrefoApi.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'CrefoLib\Documents.psm1') -Global -Force

# ---------------------------------------------------------------------------
# Configuration loading & logging setup
# ---------------------------------------------------------------------------

$script:cfg = Import-CrefoConfig -ConfigPath $ConfigPath

$script:logFile = Join-Path $script:cfg['LogDir'] ("crefo_documents_{0}.log" -f (Get-Date -Format 'yyyyMMdd_HHmmss'))
Initialize-Logger -LogFilePath $script:logFile -Level $script:cfg['LogLevel'] -Console $true

Write-CrefoInfo ("=== Crefo Factoring documents retrieval started ===")
Write-CrefoInfo ("Config       : " + $ConfigPath)
Write-CrefoInfo ("Base URL     : " + $script:cfg['BaseUrl'])
Write-CrefoInfo ("Documents dir: " + $script:cfg['DocumentsDir'])
Write-CrefoInfo ("State file   : " + (Join-Path $script:cfg['StateDir'] 'crefo_documents_state.json'))
Write-CrefoInfo ("Log file     : " + $script:logFile)
Write-CrefoInfo ("Archive dir  : " + $script:cfg['ArchiveDir'] + "  (enabled=" + $script:cfg['ArchiveRequests'] + ")")

Initialize-ApiArchive -Enabled $script:cfg['ArchiveRequests'] -RootDir $script:cfg['ArchiveDir']
Initialize-CrefoDatabase -DbPath (Join-Path $script:cfg['StateDir'] 'crefo.db')

# ---------------------------------------------------------------------------
# Authentication
# ---------------------------------------------------------------------------

$script:tokenCachePath = Join-Path $script:cfg['StateDir'] 'crefo_token_cache.json'

function Get-AppToken {
    param([bool]$Force = $false)
    return Get-CrefoAccessToken -Config $script:cfg -TokenCachePath $script:tokenCachePath -Force:$Force
}

$script:token = Get-AppToken -Force:$ForceToken
$tokenSource = if ($ForceToken) { 're-authenticated (forced)' } elseif (Test-Path -LiteralPath $script:tokenCachePath) { 'cached' } else { 'fresh login' }
Write-CrefoInfo ("Access token : " + $tokenSource)

$script:authRefresher = {
    Write-CrefoWarn 'Access token is invalid or expired; requesting a fresh token.'
    $script:token = Get-AppToken -Force $true
    return $script:token
}

# ---------------------------------------------------------------------------
# Download index
# ---------------------------------------------------------------------------

$statePath = Join-Path $script:cfg['StateDir'] 'crefo_documents_state.json'
$script:docIndex = Get-DocumentIndex -StatePath $statePath

if ($Reset) {
    Write-CrefoWarn 'Reset requested: clearing the document download index.'
    $script:docIndex.downloaded = @{}
    Save-DocumentIndex -Index $script:docIndex -StatePath $statePath
}

# ---------------------------------------------------------------------------
# Fetcher / list closures (capture script-scoped cfg/token/authRefresher so
# the module can invoke them without knowing about the script's internals)
# ---------------------------------------------------------------------------

$submissionList = {
    param([int]$Page, [int]$PageSize)
    Get-CrefoSubmissionDocuments -Config $script:cfg -AccessToken $script:token `
        -AuthRefresher $script:authRefresher -Unread $false -Page $Page -PageSize $PageSize
}

$submissionFetcher = {
    param([string]$Name, [string]$OutFile)
    Get-CrefoSubmissionDocument -Config $script:cfg -AccessToken $script:token `
        -AuthRefresher $script:authRefresher -Document $Name -OutFile $OutFile
}

$getDocumentDirectories = {
    Get-CrefoDocumentDirectories -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher
}

$folderListFactory = {
    param([string]$Folder)
    return {
        param([int]$Page, [int]$PageSize)
        Get-CrefoDocumentList -Config $script:cfg -AccessToken $script:token `
            -AuthRefresher $script:authRefresher -Directory $Folder -Unread $false -Page $Page -PageSize $PageSize
    }
}

$folderFetcherFactory = {
    param([string]$Folder)
    return {
        param([string]$Name, [string]$OutFile)
        Get-CrefoDocumentDownload -Config $script:cfg -AccessToken $script:token `
            -AuthRefresher $script:authRefresher -Directory $Folder -Document $Name -OutFile $OutFile
    }
}

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

$result = Invoke-CrefoDocuments -Config $script:cfg `
    -DocIndex $script:docIndex `
    -SubmissionList $submissionList `
    -SubmissionFetcher $submissionFetcher `
    -GetDocumentDirectories $getDocumentDirectories `
    -FolderListFactory $folderListFactory `
    -FolderFetcherFactory $folderFetcherFactory `
    -StatePath $statePath

if ($result.Failed -gt 0) {
    Write-CrefoWarn 'Some documents failed; re-run this script later to retry them.'
    exit 1
}
exit 0
