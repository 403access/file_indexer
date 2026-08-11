# =============================================================================
# ApiArchive.psm1 - persists every API exchange (request + response + data)
# to disk so calls can be audited or replayed later without hitting the API.
#
# Layout per archived call (three files, all optional except request):
#   <ArchiveDir>/<endpoint>/<timestamp>_<seq>_request.json
#   <ArchiveDir>/<endpoint>/<timestamp>_<seq>_response.json
#   <ArchiveDir>/<endpoint>/<timestamp>_<seq>_data.json   (decoded body only)
# Calls can be grouped under a category subfolder via Save-ApiExchange -Category:
#   <ArchiveDir>/<category>/<endpoint>/<timestamp>_<seq>_*.json
# (e.g. all risk calls land under <ArchiveDir>/risks/debtor-<id>-risk/).
#
# Callers must pass already-sanitized request bodies / tokens: secrets such as
# passwords, client secrets and access tokens are expected to be redacted
# BEFORE they reach Save-ApiExchange (see CrefoApi.psm1).
# =============================================================================

$script:ArchiveEnabled = $false
$script:ArchiveRoot    = $null
$script:ArchiveCounter = 0

function Initialize-ApiArchive {
    [CmdletBinding()]
    param(
        [bool]$Enabled = $true,          # master switch (config: ArchiveRequests)
        [string]$RootDir                 # top-level archive directory
    )
    if ($Enabled -and -not [string]::IsNullOrWhiteSpace($RootDir)) {
        if (-not (Test-Path -LiteralPath $RootDir)) {
            New-Item -ItemType Directory -Path $RootDir -Force | Out-Null
        }
        $script:ArchiveEnabled = $true
        $script:ArchiveRoot = $RootDir
    }
    else {
        $script:ArchiveEnabled = $false
    }
}

# Turns a free-form endpoint label into a filesystem-safe folder name.
function ConvertTo-SafeName {
    [CmdletBinding()]
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) { return 'unlabeled' }
    return ($Value -replace '[^A-Za-z0-9._-]', '_')
}

function Write-ArchiveFile {
    [CmdletBinding()]
    param(
        [string]$Path,
        [object]$Content
    )
    # Archiving must never break the actual export, so failures are logged and
    # swallowed rather than raised.
    try {
        $Content | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $Path -Encoding UTF8
    }
    catch {
        Write-CrefoDebug ("Archive write failed for '{0}': {1}" -f $Path, $_.Exception.Message)
    }
}

function Save-ApiExchange {
    [CmdletBinding()]
    param(
        [string]$Name,                  # endpoint label, e.g. 'list-debitor'
        [string]$Method,                # HTTP method
        [string]$Uri,                   # full URL including query string
        [hashtable]$Query,              # query parameters as name/value pairs
        [string]$RequestBody,           # request body text (pre-redacted), or $null
        [int]$StatusCode,               # HTTP status code of the response
        [string]$ContentType,           # response content type, if known
        [string]$ResponseRaw,           # raw response body text (pre-redacted)
        [object]$Data,                  # decoded data object for the _data.json
        [double]$ElapsedMs,             # request duration for the response file
        [switch]$IncludeAuthorization,  # record a redacted Authorization header
        [string]$Category               # optional subfolder grouping (e.g. 'risks')
    )
    if (-not $script:ArchiveEnabled) { return }

    $script:ArchiveCounter++
    $stamp = Get-Date -Format 'yyyyMMdd_HHmmss_fff'
    $sequence = '{0:D5}' -f $script:ArchiveCounter
    # Category subfolder (e.g. risks) nests below the archive root; the endpoint
    # label itself is always sanitized so it stays a single folder level.
    $dir = $script:ArchiveRoot
    if (-not [string]::IsNullOrWhiteSpace($Category)) {
        $dir = Join-Path $dir (ConvertTo-SafeName $Category)
    }
    $dir = Join-Path $dir (ConvertTo-SafeName $Name)
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $base = Join-Path $dir ($stamp + '_' + $sequence)

    # Never persist an actual bearer token - only a marker.
    $headers = @{ }
    if ($IncludeAuthorization) { $headers['Authorization'] = 'Bearer REDACTED' }

    Write-ArchiveFile -Path ($base + '_request.json') -Content ([PSCustomObject]@{
        timestamp = (Get-Date).ToUniversalTime().ToString('o')
        method    = $Method
        url       = $Uri
        query     = $Query
        headers   = $headers
        body      = $RequestBody
    })

    Write-ArchiveFile -Path ($base + '_response.json') -Content ([PSCustomObject]@{
        timestamp   = (Get-Date).ToUniversalTime().ToString('o')
        statusCode  = $StatusCode
        contentType = $ContentType
        elapsedMs   = [math]::Round($ElapsedMs, 1)
        body        = $ResponseRaw
    })

    if ($null -ne $Data) {
        Write-ArchiveFile -Path ($base + '_data.json') -Content $Data
    }
}

Export-ModuleMember -Function 'Initialize-ApiArchive', 'Save-ApiExchange'