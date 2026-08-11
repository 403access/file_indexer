# =============================================================================
# CrefoApi/Request.ps1 - HTTP plumbing for the Crefo Factoring API.
# Invoke-CrefoApi performs a REST call with retry/backoff for transient
# failures, one-shot 401 token refresh, and archiving of every exchange via
# ApiArchive.psm1. Get-HttpStatusCode classifies thrown web exceptions.
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