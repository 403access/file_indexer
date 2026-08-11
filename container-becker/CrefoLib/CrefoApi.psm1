# =============================================================================
# CrefoApi.psm1 - OAuth2 authentication and REST calls against the Crefo
# Factoring API. Includes token caching, HTTP status classification,
# retry/backoff for transient failures (so long runs survive API hiccups),
# and archiving of every request/response/data exchange via ApiArchive.psm1.
# =============================================================================

function Get-HttpStatusCode {
    [CmdletBinding()]
    param($Exception)   # the exception thrown by Invoke-WebRequest
    # The status code lives at different places depending on the error type /
    # PowerShell version, so probe a few locations defensively.
    try {
        if ($null -ne $Exception.StatusCode) {
            return [int]$Exception.StatusCode
        }
        if ($Exception.Response) {
            $code = [int]$Exception.Response.StatusCode
            if ($code -eq 0) { $code = [int]$Exception.Response.StatusCode.value__ }
            return $code
        }
    }
    catch { }
    return $null   # no HTTP response (e.g. DNS/timeout) => treat as network error
}

function Get-CrefoAccessToken {
    [CmdletBinding()]
    param(
        [hashtable]$Config,            # configuration including credentials
        [string]$TokenCachePath,       # file to persist the token in (empty = no cache)
        [switch]$Force                 # ignore the cached token and re-authenticate
    )
    # Reuse a cached token if it is still valid (with a 60s safety margin so it
    # never expires mid-request).
    if (-not $Force -and -not [string]::IsNullOrWhiteSpace($TokenCachePath) -and (Test-Path -LiteralPath $TokenCachePath)) {
        try {
            $cached = Get-Content -LiteralPath $TokenCachePath -Raw -Encoding UTF8 | ConvertFrom-Json
            # ConvertFrom-Json already parses the ISO timestamp into a DateTime
            # (Kind=Utc when it has the 'Z' suffix). Using it directly - rather
            # than round-tripping through [string], which drops the 'Z' and
            # makes ToUniversalTime() apply the local offset - keeps the UTC
            # math correct.
            $acquiredUtc = $cached.acquiredAt
            if ($acquiredUtc.Kind -ne 'Utc') { $acquiredUtc = $acquiredUtc.ToUniversalTime() }
            $expiresAt = $acquiredUtc.AddSeconds([int]$cached.expires_in).AddSeconds(-60)
            if ([string]::IsNullOrWhiteSpace([string]$cached.access_token) -eq $false -and (Get-Date).ToUniversalTime() -lt $expiresAt) {
                return [string]$cached.access_token
            }
        }
        catch {
            Write-CrefoDebug ("Cached token invalid, will re-authenticate: {0}" -f $_.Exception.Message)
        }
    }

    # OAuth2 Resource Owner Password flow against the token endpoint.
    $body = @{
        grant_type    = 'password'
        username      = [string]$Config['Username']
        password      = [string]$Config['Password']
        client_id     = [string]$Config['ClientId']
        client_secret = [string]$Config['ClientSecret']
    }
    # Corporate/admin accounts may opt into a specific obligo; if set it must be
    # sent as part of the token request.
    if (-not [string]::IsNullOrWhiteSpace([string]$Config['ObligoNumber'])) {
        $body['obligonumber'] = [string]$Config['ObligoNumber']
    }

    $tokenUrl = ([string]$Config['BaseUrl']).TrimEnd('/') + '/connect/token'
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $response = Invoke-RestMethod -Method Post -Uri $tokenUrl -Body $body -ContentType 'application/x-www-form-urlencoded' -TimeoutSec 60
    $stopwatch.Stop()

    if (-not $response.access_token) {
        throw 'Token endpoint did not return an access_token.'
    }

    $token = [string]$response.access_token

    # Archive the token exchange with all secrets masked out (credentials in
    # the request, the access token in the response) so nothing sensitive
    # lands in the archive folder. Archive writes are skipped if disabled.
    $maskedBody = 'grant_type=password&username=<redacted>&password=<redacted>&client_id=<redacted>&client_secret=<redacted>'
    if (-not [string]::IsNullOrWhiteSpace([string]$Config['ObligoNumber'])) {
        $maskedBody += '&obligonumber=<redacted>'
    }
    $maskedRaw = $response | ConvertTo-Json -Depth 10
    $maskedRaw = $maskedRaw -replace [regex]::Escape($token), 'REDACTED'
    Save-ApiExchange -Name 'token' -Method 'POST' -Uri $tokenUrl -Query @{} `
        -RequestBody $maskedBody -StatusCode 200 -ContentType 'application/json' `
        -ResponseRaw $maskedRaw -Data $null -ElapsedMs $stopwatch.Elapsed.TotalMilliseconds
    # Persist the token + lifetime so the next run can skip re-authentication.
    if (-not [string]::IsNullOrWhiteSpace($TokenCachePath)) {
        $dir = Split-Path -Path $TokenCachePath -Parent
        if ($dir -and -not (Test-Path -LiteralPath $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
        $store = [PSCustomObject]@{
            access_token = $token
            expires_in   = [int]($response.expires_in)
            acquiredAt   = (Get-Date).ToUniversalTime().ToString('o')
        }
        $store | ConvertTo-Json | Set-Content -LiteralPath $TokenCachePath -Encoding UTF8
        # Tokens are sensitive; restrict read access on Unix if we can.
        try {
            if ($IsLinux -or $IsMacOS) { & chmod 600 $TokenCachePath | Out-Null }
        }
        catch { }
    }
    return $token
}

function Invoke-CrefoApi {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration (BaseUrl, MaxRetries, RequestDelayMs)
        [string]$Method = 'GET',            # HTTP method
        [string]$Path,                      # absolute path, e.g. /api/v1/...
        [string]$AccessToken,               # Bearer token to send
        [hashtable]$Query = @{},            # optional query-string parameters
        [scriptblock]$AuthRefresher,        # optional scriptblock returning a fresh token on 401
        [string]$ArchiveName,               # endpoint label used for the request/response archive
        [string]$ArchiveCategory            # optional archive subfolder (e.g. 'risks')
    )
    $maxRetries = [int]$Config['MaxRetries']
    $delayMs = [int]$Config['RequestDelayMs']
    $base = ([string]$Config['BaseUrl']).TrimEnd('/')

    # Build the final URI, appending URL-encoded query parameters.
    $uri = $base + $Path
    if ($Query.Count -gt 0) {
        $pairs = @()
        foreach ($key in $Query.Keys) {
            $pairs += ('{0}={1}' -f $key, [uri]::EscapeDataString([string]$Query[$key]))
        }
        $uri += '?' + ($pairs -join '&')
    }

    $attempt = 0
    $refreshed = $false   # only allow one token refresh per call (avoids refresh loops)
    $lastError = $null
    $stopwatch = [System.Diagnostics.Stopwatch]::new()

    while ($true) {
        $attempt++
        # Back off before retries (not before the first attempt) with a small
        # random jitter to avoid thundering-herd behavior.
        if ($attempt -gt 1 -and $delayMs -gt 0) {
            $jitter = Get-Random -Minimum 0 -Maximum 500
            Start-Sleep -Milliseconds ($delayMs + $jitter)
        }
        $stopwatch.Restart()
        try {
            $headers = @{ Authorization = ("Bearer {0}" -f $AccessToken) }
            # Invoke-WebRequest (rather than Invoke-RestMethod) lets us capture
            # the raw body for archiving. -UseBasicParsing keeps PS 5.1 on the
            # same code path as PowerShell 7.
            $response = Invoke-WebRequest -Method $Method -Uri $uri -Headers $headers -UseBasicParsing -TimeoutSec 120
            $stopwatch.Stop()
            $statusCode = [int]$response.StatusCode
            $contentType = [string]$response.Headers['Content-Type']
            $rawBody = [string]$response.Content
            $parsed = $null
            if (-not [string]::IsNullOrWhiteSpace($rawBody)) {
                $parsed = ConvertFrom-Json -InputObject $rawBody
            }
            # Archive request + raw response + decoded data for this call.
            Save-ApiExchange -Name $ArchiveName -Method $Method -Uri $uri -Query $Query `
                -StatusCode $statusCode -ContentType $contentType -ResponseRaw $rawBody `
                -Data $parsed -ElapsedMs $stopwatch.Elapsed.TotalMilliseconds -IncludeAuthorization `
                -Category $ArchiveCategory
            return $parsed
        }
        catch {
            $stopwatch.Stop()
            $lastError = $_.Exception
            $status = Get-HttpStatusCode -Exception $lastError

            # 401 => the token is invalid/expired; refresh once and retry with the new token.
            if ($status -eq 401 -and $null -ne $AuthRefresher -and -not $refreshed) {
                $refreshed = $true
                Write-CrefoWarn ("Received HTTP 401 on {0}; refreshing access token and retrying." -f $Path)
                $AccessToken = & $AuthRefresher
                continue
            }

            # Transient HTTP errors and network-level failures are retryable.
            $transient = @(408, 429, 500, 502, 503, 504)
            if ($status -in $transient -and $attempt -le ($maxRetries + 1)) {
                Write-CrefoWarn ("Transient error HTTP {0} on {1} (attempt {2}/{3}); retrying." -f $status, $Path, $attempt, ($maxRetries + 1))
                continue
            }
            if ($null -eq $status -and $attempt -le ($maxRetries + 1)) {
                Write-CrefoWarn ("Network error on {0} (attempt {1}/{2}); retrying. {3}" -f $Path, $attempt, ($maxRetries + 1), $_.Exception.Message)
                continue
            }
            # Retries exhausted or a non-retryable error: archive the failed
            # exchange (status + error body) before surfacing to the caller.
            Save-ApiExchange -Name $ArchiveName -Method $Method -Uri $uri -Query $Query `
                -StatusCode $status -ContentType '' -ResponseRaw $_.ErrorDetails.Message `
                -Data $null -ElapsedMs $stopwatch.Elapsed.TotalMilliseconds -IncludeAuthorization `
                -Category $ArchiveCategory
            throw $lastError
        }
    }
}

function Get-CrefoAccounts {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [int]$PageSize = 50,                # items per page (API default: 50)
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'list-debitor'   # archive folder label for these pages
    )
    # Walk all pages of GET /api/v1/DebitorAccounts/list-debitor and collect the
    # (id, name) pairs. Pagination ends when we pass totalPages or receive a
    # short page.
    $all = New-Object System.Collections.Generic.List[object]
    $page = 1
    $totalPages = $null

    while ($true) {
        $response = Invoke-CrefoApi -Config $Config -Method GET `
            -Path '/api/v1/DebitorAccounts/list-debitor' `
            -AccessToken $AccessToken `
            -Query @{ page = $page; pagesize = $PageSize } `
            -AuthRefresher $AuthRefresher `
            -ArchiveName $ArchiveName

        $items = @($response.items)
        foreach ($item in $items) {
            if ($null -ne $item.id) {
                $all.Add([PSCustomObject]@{
                    id   = [int]$item.id
                    name = [string]$item.name
                })
            }
        }

        # Remember totalPages from the very first response (it is stable).
        if ($null -eq $totalPages -and $null -ne $response.header) {
            $totalPages = [int]$response.header.totalPages
        }
        $page++
        if ($null -ne $totalPages -and $page -gt $totalPages) { break }
        if (@($items).Count -lt $PageSize) { break }
    }
    # ToArray() avoids a PowerShell quirk where wrapping a List[object] created
    # via New-Object in @() throws "Argument types do not match".
    return $all.ToArray()
}

function Get-CrefoDebtorRisk {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [int]$DebtorId,                     # debtor account id
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = ''           # optional override for the archive folder label
    )
    # GET /api/v1/DebitorAccounts/{debitor}/risk returns an array; take the
    # first element since we query one specific debtor.
    $path = ('/api/v1/DebitorAccounts/{0}/risk' -f $DebtorId)
    if ([string]::IsNullOrWhiteSpace($ArchiveName)) {
        $ArchiveName = ('debtor-{0}-risk' -f $DebtorId)
    }
    $response = Invoke-CrefoApi -Config $Config -Method GET -Path $path `
        -AccessToken $AccessToken -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName `
        -ArchiveCategory 'risks'
    return @($response)[0]
}

function Get-CrefoLastLimitDecisions {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'last-limit-decisions'   # archive folder label
    )
    # GET /api/v1/last-limit-decisions returns the current completed limit
    # decision per debtor account (single call, no pagination). It is used as a
    # bulk source of 'has a limit or not' so accounts without any decision can
    # skip the per-debtor /risk request entirely.
    $response = Invoke-CrefoApi -Config $Config -Method GET `
        -Path '/api/v1/last-limit-decisions' `
        -AccessToken $AccessToken -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName
    return @($response)
}

Export-ModuleMember -Function 'Get-CrefoAccessToken', 'Invoke-CrefoApi', 'Get-CrefoAccounts', 'Get-CrefoDebtorRisk', 'Get-CrefoLastLimitDecisions'