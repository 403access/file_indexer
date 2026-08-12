# =============================================================================
# tests/Mock/Routes/Risk.ps1 - /api/v1/DebitorAccounts/{id}/risk endpoint.
# Dot-sourced by tests/Mock/Mock-Server.ps1.
# =============================================================================

function Invoke-MockRiskRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    if ($Path -notmatch '^/api/v1/DebitorAccounts/(\d+)/risk$') { return $false }
    $id = [int]$Matches[1]
    $script:mockDebtor = [string]$id
    $script:counts.risk = (Get-CountValue 'risk') + 1

    if ($id -in @($faults['Risk500Ids'])) {
        Write-RequestLog 'GET' $Path $Query 500 $script:mockDebtor
        Send-Json $Ctx @{ error = 'risk fault (simulated)' } 500
    }
    elseif ($id -in @($faults['Risk401OnceIds']) -and -not $script:spoken401.Contains($script:mockDebtor)) {
        $script:spoken401.Add($script:mockDebtor) | Out-Null
        Write-RequestLog 'GET' $Path $Query 401 $script:mockDebtor
        Send-Json $Ctx @{ error = 'unauthorized (simulated)' } 401
    }
    elseif ($id -eq [int]$faults['Risk500OnceId'] -and -not $script:risk500OnceSpoken) {
        $script:risk500OnceSpoken = $true
        Write-RequestLog 'GET' $Path $Query 500 $script:mockDebtor
        Send-Json $Ctx @{ error = 'transient fault (simulated)' } 500
    }
    else {
        Write-RequestLog 'GET' $Path $Query 200 $script:mockDebtor
        Send-Json $Ctx @(Get-RiskForId -Id $id)
    }
    return $true
}
