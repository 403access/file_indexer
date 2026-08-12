# =============================================================================
# tests/Mock/Routes/Token.ps1 - /connect/token endpoint.
# Dot-sourced by tests/Mock/Mock-Server.ps1.
# =============================================================================

function Invoke-MockTokenRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    if ($Path -ne '/connect/token') { return $false }
    $script:counts.token = (Get-CountValue 'token') + 1
    Send-Json $Ctx @{
        access_token = ("mock-token-$([guid]::NewGuid().ToString('N'))")
        token_type   = 'Bearer'
        expires_in   = 3600
        isCorporateLogin = $false
    }
    return $true
}
