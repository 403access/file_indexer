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
    $limitDisplay = if ($null -ne $Account.limit_value -and $Account.limit_value -ne '') { [math]::Round([double]$Account.limit_value, 2).ToString('N2') } else { '(none)' }
    $purchasedDisplay = if ($null -ne $Account.purchased -and $Account.purchased -ne '') { [math]::Round([double]$Account.purchased, 2).ToString('N2') } else { '(none)' }
    $balanceDisplay = if ($null -ne $Account.balance -and $Account.balance -ne '') { [math]::Round([double]$Account.balance, 2).ToString('N2') } else { '(none)' }
    $codeDisplay = if ($null -ne $Account.limit_code -and $Account.limit_code -ne '') { $Account.limit_code } else { '(none)' }
    $fetchedDisplay = if ($null -ne $Account.fetched_at -and $Account.fetched_at -ne '') { $Account.fetched_at } else { '(none)' }
    $sourceDisplay = if ($null -ne $Account.source -and $Account.source -ne '') { $Account.source } else { '(none)' }
    Write-Host ("Limit   : {0}" -f $limitDisplay)
    Write-Host ("Purchased: {0}" -f $purchasedDisplay)
    Write-Host ("Balance : {0}" -f $balanceDisplay)
    Write-Host ("Code    : {0}" -f $codeDisplay)
    Write-Host ("Fetched : {0}" -f $fetchedDisplay)
    Write-Host ("Source  : {0}" -f $sourceDisplay)

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
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-10} {6,-8}" -f 'ID', 'Name', 'Status', 'Limit', 'Gekauft', 'Balance', 'Code')
    Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-10} {6,-8}" -f '--------', '------------------------------', '----------', '------------', '----------', '----------', '--------')

    foreach ($r in $Rows) {
        if ($r.name.Length -gt 28) {
            $name = $r.name.Substring(0, 28) + '..'
        }
        else {
            $name = $r.name
        }
        $limitDisplay = if ($r.limit_value) { [math]::Round([double]$r.limit_value, 2).ToString('N2') } else { '(none)' }
        $purchasedDisplay = if ($r.purchased) { [math]::Round([double]$r.purchased, 2).ToString('N2') } else { '(none)' }
        $balanceDisplay = if ($r.balance) { [math]::Round([double]$r.balance, 2).ToString('N2') } else { '(none)' }
        $codeDisplay = if ($r.limit_code) { $r.limit_code } else { '(none)' }
        $fetchedDisplay = if ($r.fetched_at) { $r.fetched_at.Substring(0, [math]::Min(8, $r.fetched_at.Length)) } else { '(none)' }
        Write-Host ("{0,-8} {1,-30} {2,-10} {3,-12} {4,-10} {5,-10} {6,-8}" -f `
            $r.id, $name, $r.status, $limitDisplay, $purchasedDisplay, $balanceDisplay, $codeDisplay)
    }
    Write-Host ""
    Write-Host ("{0} row(s)" -f $Rows.Count)
}
