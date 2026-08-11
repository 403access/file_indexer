# =============================================================================
# CrefoLib/Inspect/Format.ps1 - output formatting functions for inspection.
# Dot-sourced by CrefoLib/Inspect/index.psm1.
# =============================================================================

function Show-DatabaseStats {
    [CmdletBinding()]
    param([object]$StatsObject)
    foreach ($prop in $StatsObject.PSObject.Properties) {
        Write-Host ("{0,-20} {1}" -f $prop.Name, $prop.Value)
    }
}

function Show-InspectAccountDetail {
    [CmdletBinding()]
    param([object]$Account, [switch]$History, [string]$DbPath)
    Write-Host ("Account : {0} ({1})" -f $Account.id, $Account.name)
    Write-Host ("Status  : {0}" -f $Account.status)
    if ($Account.error) { Write-Host ("Error   : {0}" -f $Account.error) }
    Write-Host ("Created : {0}" -f $Account.created_at)
    Write-Host ("Updated : {0}" -f $Account.updated_at)
    Write-Host ("Limit   : {0}" -f $Account.limit_value)
    Write-Host ("Purchased: {0}" -f $Account.purchased)
    Write-Host ("Balance : {0}" -f $Account.balance)
    Write-Host ("Code    : {0}" -f $Account.limit_code)
    Write-Host ("Fetched : {0}" -f $Account.fetched_at)
    Write-Host ("Source  : {0}" -f $Account.source)

    if ($History) {
        Write-Host ""
        Write-Host "Snapshot history:"
        $hist = Get-InspectAccountHistory -DbPath $DbPath -AccountId $Account.id
        if ($hist.Count -eq 0) {
            Write-Host "  (none)"
        }
        else {
            foreach ($h in $hist) {
                Write-Host ("  [{0}] limit={1} purchased={2} balance={3} code={4} fetched={5} source={6}" -f `
                    $h.id, $h.limit_value, $h.purchased, $h.balance, $h.limit_code, $h.fetched_at, $h.source)
            }
        }
    }
}

function Show-InspectAccountList {
    [CmdletBinding()]
    param([object[]]$Rows)
    if ($Rows.Count -eq 0) {
        Write-Host "No accounts found."
        return
    }
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f 'ID', 'Name', 'Status', 'Limit', 'Code', 'Fetched')
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f '--------', '------------------------------', '----------', '------------', '----------', '--------')

    foreach ($r in $Rows) {
        if ($r.name.Length -gt 28) {
            $name = $r.name.Substring(0, 28) + '..'
        }
        else {
            $name = $r.name
        }
        $limitDisplay = if ($r.limit_value) { [math]::Round([double]$r.limit_value, 2).ToString('N2') } else { '0,00' }
        $fetchedDisplay = if ($r.fetched_at) { $r.fetched_at.Substring(0, [math]::Min(8, $r.fetched_at.Length)) } else { '' }
        Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-8}" -f `
            $r.id, $name, $r.status, $limitDisplay, $r.limit_code, $fetchedDisplay)
    }
    Write-Host ""
    Write-Host ("{0} row(s)" -f $Rows.Count)
}
