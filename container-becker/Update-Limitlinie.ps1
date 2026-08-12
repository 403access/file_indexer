#requires -Version 7.0
# =============================================================================
# Update-Limitlinie.ps1 - standalone downloader for the Crefo Factoring daily
# limit-line CSV. Standalone: only pwsh 7 and one small config file are needed.
#
#   1. Lists every file in the portal's "Tagesabrechnungen" document folder.
#   2. Downloads the newest *_Limitlinie.csv (sorted by created timestamp)
#      unless that exact file is already in the download folder.
#   3. Deletes every other downloaded file so only the most recent remains.
#
# Usage:
#   pwsh -File Update-Limitlinie.ps1                       # use limitlinie-config.psd1
#   pwsh -File Update-Limitlinie.ps1 -ConfigPath ...       # custom config
#   pwsh -File Update-Limitlinie.ps1 -Force                # re-download newest even if present
#
# Exit code: 0 when the newest file is present locally, 1 on any failure.
# =============================================================================

[CmdletBinding()]
param(
    [string]$ConfigPath = (Join-Path $PSScriptRoot 'limitlinie-config.psd1'),
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

if (-not (Test-Path -LiteralPath $ConfigPath)) {
    throw "Config file not found: $ConfigPath"
}
$script:cfg = Import-PowerShellDataFile -LiteralPath $ConfigPath

foreach ($key in @('BaseUrl', 'Username', 'Password', 'ClientId', 'ClientSecret')) {
    if ([string]::IsNullOrWhiteSpace([string]$script:cfg[$key])) {
        throw "Config is missing required setting '$key' ($ConfigPath)."
    }
}

$script:dir       = if ([string]::IsNullOrWhiteSpace([string]$script:cfg['Directory']))  { 'Tagesabrechnungen' } else { [string]$script:cfg['Directory'] }
$script:suffix    = if ([string]::IsNullOrWhiteSpace([string]$script:cfg['FileSuffix'])) { '_Limitlinie.csv' } else { [string]$script:cfg['FileSuffix'] }
$script:token     = $null

# Resolve the download folder (relative paths are anchored to the script dir)
$downloadDirRaw = if ([string]::IsNullOrWhiteSpace([string]$script:cfg['DownloadDir'])) { 'data/documents/limitlinie' } else { [string]$script:cfg['DownloadDir'] }
$script:downloadDir = if ([System.IO.Path]::IsPathRooted($downloadDirRaw)) { $downloadDirRaw } else { Join-Path $PSScriptRoot $downloadDirRaw }

# Optional dbisql (SQL Anywhere) run when a more recent file was found and
# downloaded (not on skips). dbisql accepts only a single command/file origin,
# so the downloaded CSV path is baked into a temp copy of the SQL script (the
# '{csv_path}' placeholder is substituted and the 'PARAMETERS csv_path;' line
# is dropped, since batch mode never populates PARAMETERS from the command
# line). Equivalent to:
#   dbisql -c <DbisqlConnString> -nogui -onerror exit <patchedSql>
# 'DbisqlPath' defaults to 'dbisql' (PATH lookup); 'SqlScript' relative paths
# are anchored to the script dir.
$script:dbisql = $null
if (-not [string]::IsNullOrWhiteSpace([string]$script:cfg['DbisqlConnString'])) {
    $script:dbisqlPath = if ([string]::IsNullOrWhiteSpace([string]$script:cfg['DbisqlPath'])) { 'dbisql' } else { [string]$script:cfg['DbisqlPath'] }
    $script:dbisqlConn = [string]$script:cfg['DbisqlConnString']
    $sqlRaw = [string]$script:cfg['SqlScript']
    if ([string]::IsNullOrWhiteSpace($sqlRaw)) {
        throw "Config sets 'DbisqlConnString' but is missing required setting 'SqlScript' ($ConfigPath)."
    }
    $script:dbisqlSql = if ([System.IO.Path]::IsPathRooted($sqlRaw)) { $sqlRaw } else { Join-Path $PSScriptRoot $sqlRaw }
    if (-not (Test-Path -LiteralPath $script:dbisqlSql)) {
        throw "Configured 'SqlScript' not found: $script:dbisqlSql"
    }
    $script:dbisql = $true
}

# ---------------------------------------------------------------------------
# Auth + API helpers
# ---------------------------------------------------------------------------

function Get-LimitlineToken {
    $body = @{
        grant_type    = 'password'
        username      = [string]$script:cfg['Username']
        password      = [string]$script:cfg['Password']
        client_id     = [string]$script:cfg['ClientId']
        client_secret = [string]$script:cfg['ClientSecret']
    }
    if (-not [string]::IsNullOrWhiteSpace([string]$script:cfg['ObligoNumber'])) {
        $body['obligonumber'] = [string]$script:cfg['ObligoNumber']
    }
    $uri = ([string]$script:cfg['BaseUrl']).TrimEnd('/') + '/connect/token'
    $resp = Invoke-RestMethod -Method Post -Uri $uri -Body $body `
        -ContentType 'application/x-www-form-urlencoded' -TimeoutSec 60
    if (-not $resp.access_token) { throw 'Token endpoint returned no access_token.' }
    return [string]$resp.access_token
}

# GET one API path with retries: re-auth once on 401, backoff on transient
# errors. -Binary streams the response body into $OutFile.
function Invoke-LimitlineApi {
    param(
        [string]$Path,
        [hashtable]$Query = @{},
        [switch]$Binary,
        [string]$OutFile
    )
    $uri = ([string]$script:cfg['BaseUrl']).TrimEnd('/') + $Path
    if ($Query.Count -gt 0) {
        $qs = foreach ($k in $Query.Keys) { '{0}={1}' -f $k, [string]$Query[$k] }
        $uri += '?' + ($qs -join '&')
    }
    for ($attempt = 1; ; $attempt++) {
        try {
            $http = @{
                Method    = 'Get'
                Uri       = $uri
                Headers   = @{ Authorization = "Bearer $script:token" }
                TimeoutSec = 60
            }
            if ($Binary) {
                Invoke-WebRequest @http -OutFile $OutFile | Out-Null
                return
            }
            return Invoke-RestMethod @http
        }
        catch {
            $status = if ($null -ne $_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
            if ($status -eq 401) {
                Write-Host 'Access token rejected; re-authenticating...'
                $script:token = Get-LimitlineToken
                continue
            }
            if (($status -ge 500 -or $status -eq 429) -and $attempt -le 5) {
                $delay = 2 * $attempt
                Write-Warning ("Server returned {0}; retrying in {1}s (attempt {2}/5)..." -f $status, $delay, $attempt)
                Start-Sleep -Seconds $delay
                continue
            }
            if ($status -eq 404) { throw "Not found: $Path" }
            throw ('API call failed ({0}): {1}' -f $uri, $_.Exception.Message)
        }
    }
}

# ---------------------------------------------------------------------------
# Domain logic
# ---------------------------------------------------------------------------

function Confirm-LimitlineFolder {
    $resp = Invoke-LimitlineApi -Path '/api/v1/Documents/list-directory'
    $folders = @($resp.folder)
    foreach ($f in $folders) {
        if ([string]::Equals([string]$f, $script:dir, [StringComparison]::OrdinalIgnoreCase)) { return }
    }
    $available = if ($folders.Count -eq 0) { '(none)' } else { $folders -join ', ' }
    throw ("Folder '{0}' not found on the server. Available folders: {1}" -f $script:dir, $available)
}

function Get-LimitlineDocuments {
    $all = @()
    $page = 1
    while ($true) {
        $resp = Invoke-LimitlineApi -Path ("/api/v1/Documents/{0}/list-document" -f [uri]::EscapeDataString($script:dir)) `
            -Query @{ unread = 'false'; page = $page; pagesize = '100' }
        if ($resp -is [array]) {
            $all += $resp
            break
        }
        $all += @($resp.items)
        $totalPages = if ($null -ne $resp.header -and $null -ne $resp.header.totalPages) { [int]$resp.header.totalPages } else { $page }
        if ($page -ge $totalPages) { break }
        $page++
    }
    return $all
}

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

Write-Host "== Crefo limit-line download =="
Write-Host ("Config     : {0}" -f $ConfigPath)
Write-Host ("Folder     : {0}" -f $script:dir)
Write-Host ("File suffix: {0}" -f $script:suffix)
Write-Host ("Download   : {0}" -f $script:downloadDir)

$script:token = Get-LimitlineToken

Confirm-LimitlineFolder

$docs = @(Get-LimitlineDocuments)
Write-Host ("Listed {0} file(s) in folder '{1}'." -f $docs.Count, $script:dir)

$suffixPattern = $script:suffix
$candidates = @($docs | Where-Object {
    $_.name -and ([string]$_.name).EndsWith($suffixPattern, [StringComparison]::OrdinalIgnoreCase)
})
if ($candidates.Count -eq 0) {
    throw "No file ending in '$script:suffix' found in folder '$script:dir'."
}
Write-Host ("Matched {0} limit-line file(s)." -f $candidates.Count)
foreach ($c in $candidates | Sort-Object -Property @{ Expression = { try { [datetime]$_.created } catch { [datetime]::MinValue } } } -Descending) {
    Write-Host ("  {0}  ({1})" -f $c.name, $c.created)
}

$latest = $candidates | Sort-Object -Property @{ Expression = { try { [datetime]$_.created } catch { [datetime]::MinValue } } } -Descending | Select-Object -First 1
$keepName = [string]$latest.name
$outFile = Join-Path $script:downloadDir $keepName

New-Item -ItemType Directory -Path $script:downloadDir -Force | Out-Null

if ((Test-Path -LiteralPath $outFile) -and -not $Force) {
    Write-Host ("Already downloaded (skip): {0}" -f $keepName)
}
else {
    try {
        Invoke-LimitlineApi -Path ("/api/v1/Documents/{0}/{1}" -f [uri]::EscapeDataString($script:dir), [uri]::EscapeDataString($keepName)) `
            -Binary -OutFile $outFile
    }
    catch {
        if (Test-Path -LiteralPath $outFile) { Remove-Item -LiteralPath $outFile -Force }
        throw
    }
    Write-Host ("Downloaded: {0}  ({1} bytes)" -f $keepName, (Get-Item -LiteralPath $outFile).Length)
    if ($script:dbisql) {
        $csvEscaped = $outFile -replace "'", "''"
        $sqlText = [System.IO.File]::ReadAllText($script:dbisqlSql)
        $patched = ($sqlText -replace "(?im)^[ \t]*PARAMETERS[ \t]+csv_path[ \t]*;?[ \t\r\n]*", '') -replace '\{csv_path\}', $csvEscaped
        $tmpSql = Join-Path ([System.IO.Path]::GetTempPath()) ("update_from_csv_{0}.sql" -f [guid]::NewGuid().ToString('N'))
        [System.IO.File]::WriteAllText($tmpSql, $patched)
        try {
            Write-Host ("Running dbisql on new download: {0} -c {1} -nogui -onerror exit {2}" -f $script:dbisqlPath, $script:dbisqlConn, $tmpSql)
            & $script:dbisqlPath -c $script:dbisqlConn -nogui -onerror exit $tmpSql
            Write-Host ("dbisql exit code: {0}" -f $LASTEXITCODE)
            if ($LASTEXITCODE -ne 0) { throw "dbisql failed with exit code $LASTEXITCODE" }
        }
        finally {
            Remove-Item -LiteralPath $tmpSql -Force -ErrorAction SilentlyContinue
        }
    }
}

$removed = 0
foreach ($f in Get-ChildItem -LiteralPath $script:downloadDir -File) {
    if ($f.Name -ne $keepName) {
        Remove-Item -LiteralPath $f.FullName -Force
        $removed++
    }
}
if ($removed -gt 0) {
    Write-Host ("Deleted {0} older download(s); keeping only '{1}'." -f $removed, $keepName)
}
else {
    Write-Host ("No old downloads to remove; keeping '{0}'." -f $keepName)
}

Write-Host "== Done =="