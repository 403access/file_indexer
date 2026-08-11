# =============================================================================
# tests/Mock/Routes/Accounts.ps1 - /api/v1/DebitorAccounts/list-debitor endpoint.
# Dot-sourced by tests/Mock/Mock-Server.ps1.
# =============================================================================

function Invoke-MockAccountsRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    if ($Path -ne '/api/v1/DebitorAccounts/list-debitor') { return $false }
    $pageSize = if ($null -eq $Query['pagesize']) { 50 } else { [int]$Query['pagesize'] }
    $page = if ($null -eq $Query['page']) { 1 } else { [int]$Query['page'] }
    $isProbe = ($pageSize -le 1)
    if ($isProbe) {
        $script:counts.probe = (Get-CountValue 'probe') + 1
        if ($faults['Probe500']) {
            Write-RequestLog 'GET' $Path $Query 500
            Send-Json $Ctx @{ error = 'probe fault (simulated)' } 500
        }
        else {
            Write-RequestLog 'GET' $Path $Query 200
            Send-Json $Ctx @{
                header = @{
                    currentPage  = 1
                    itemsPerPage = $pageSize
                    totalItems   = [int]$accounts.Count
                    totalPages   = if ($pageSize -ge 1) { [math]::Ceiling([double]$accounts.Count / [double]$pageSize) } else { 1 }
                }
                items  = @()
            }
        }
    }
    else {
        $script:counts.list = (Get-CountValue 'list') + 1
        Write-RequestLog 'GET' $Path $Query 200
        $totalPages = [math]::Ceiling([double]$accounts.Count / [double]$pageSize)
        $start = ($page - 1) * $pageSize
        $items = @($accounts | Select-Object -Skip $start -First $pageSize)
        Send-Json $Ctx @{
            header = @{
                currentPage  = [int]$page
                itemsPerPage = [int]$pageSize
                totalItems   = [int]$accounts.Count
                totalPages   = [int]$totalPages
            }
            items  = $items
        }
    }
    return $true
}
