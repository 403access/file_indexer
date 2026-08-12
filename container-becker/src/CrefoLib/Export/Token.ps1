# =============================================================================
# CrefoLib/Export/Token.ps1 - authentication bootstrap for the exporter.
# Dot-sourced by Start-CrefoExport.ps1 into the script scope so it can share
# $script:cfg, $script:token and $script:authRefresher with the API module and
# the other Export feature files.
#
# Initialize-CrefoExportToken:
#   1. set up the token cache path and the Get-AppToken wrapper
#   2. obtain an access token (reuses the disk cache unless -ForceToken)
#   3. install $script:authRefresher for the API module's 401 handling
# =============================================================================

# Small wrapper so the token cache path/config only live in one place.
function Get-AppToken {
    param([bool]$Force = $false)
    return Get-CrefoAccessToken -Config $script:cfg -TokenCachePath $script:tokenCachePath -Force:$Force
}

function Initialize-CrefoExportToken {
    [CmdletBinding()]
    param([switch]$ForceToken)

    $script:tokenCachePath = Join-Path $script:cfg['StateDir'] 'crefo_token_cache.json'

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
}