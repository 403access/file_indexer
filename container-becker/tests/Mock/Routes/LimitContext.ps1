# =============================================================================
# tests/Mock/Routes/LimitContext.ps1 - /api/v1/last-limit-decisions and
# /api/v1/open-limit-desires endpoints.
# Dot-sourced by tests/Mock/Mock-Server.ps1.
# =============================================================================

function Invoke-MockDecisionsRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    if ($Path -ne '/api/v1/last-limit-decisions') { return $false }
    $script:counts.decisions = (Get-CountValue 'decisions') + 1
    if ($faults['Decisions500']) {
        Write-RequestLog 'GET' $Path $Query 500
        Send-Json $Ctx @{ error = 'decisions fault (simulated)' } 500
    }
    else {
        Write-RequestLog 'GET' $Path $Query 200
        Send-Json $Ctx @($decisions)
    }
    return $true
}

function Invoke-MockDesiresRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    if ($Path -ne '/api/v1/open-limit-desires') { return $false }
    $script:counts.desires = (Get-CountValue 'desires') + 1
    if ($faults['Desires500']) {
        Write-RequestLog 'GET' $Path $Query 500
        Send-Json $Ctx @{ error = 'desires fault (simulated)' } 500
    }
    else {
        Write-RequestLog 'GET' $Path $Query 200
        Send-Json $Ctx @($desires)
    }
    return $true
}
