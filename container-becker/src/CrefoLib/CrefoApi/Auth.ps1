# =============================================================================
# CrefoApi/Auth.ps1 - OAuth2 authentication for the Crefo Factoring API.
# Token retrieval (Resource Owner Password flow) with disk caching, secret
# masking for the archive, and restricted file permissions on Unix.
# =============================================================================

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