# =============================================================================
# tests/Mock/Routes/NotFound.ps1 - catch-all 404 handler.
# Dot-sourced by tests/Mock/Mock-Server.ps1.
# =============================================================================

function Invoke-MockNotFoundRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    Write-RequestLog 'GET' $Path $Query 404 $script:mockDebtor
    $Ctx.Response.StatusCode = 404
    $Ctx.Response.Close()
}
