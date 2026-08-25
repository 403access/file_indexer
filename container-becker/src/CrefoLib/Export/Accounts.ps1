# =============================================================================
# CrefoLib/Export/Accounts.ps1 - state loading and account discovery for the
# exporter. Dot-sourced by Start-CrefoExport.ps1 into the script scope so it can
# share $script:cfg, $script:dbPath, $script:token, $script:authRefresher and
# $script:state with the other Export feature files.
#
# Sync-CrefoAccountList:
#   1. load crefo_state.json into $script:state
#   2. -Reset: forget all progress (JSON state, output CSV, SQLite rows)
#   3. seed the SQLite database from the JSON state (so the CSV is rebuildable
#      even before any /risk runs)
#   4. refresh the account list: full sync on the first run; afterwards probe the
#      production list with pageSize=1 and fetch only the delta. A broken probe
#      degrades to the safe full-list behaviour.
# =============================================================================

function Sync-CrefoAccountList {
    [CmdletBinding()]
    param([switch]$Reset)

    $script:statePath = Join-Path $script:cfg['StateDir'] 'crefo_state.json'
    $script:state = Get-CrefoState -Path $script:statePath

    # -Reset: forget all progress and start from an empty CSV.
    if ($Reset) {
        Write-CrefoWarn 'Reset requested: resetting all accounts from scratch.'
        Reset-CrefoAccounts -State $script:state | Out-Null
        $resetCsv = Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']
        if (Test-Path -LiteralPath $resetCsv) { Remove-Item -LiteralPath $resetCsv -Force }
        # Let the database rebuild from the reset run as well.
        Invoke-CrefoSqlite -DbPath $script:dbPath -Sql 'DELETE FROM risk_snapshots; DELETE FROM accounts;' -ErrorAction SilentlyContinue | Out-Null
    }

    # One-time seed: copy accounts/snapshots already known in JSON state into the
    # database so the CSV can be rebuilt from it even before any /risk runs. This
    # runs before discovery so the delta-sync decision below can read the database.
    Import-CrefoDatabaseFromState -State $script:state

    # Refresh the account list (merges new debtors in, keeps old progress) unless
    # the config disables it. Always fetched on the very first run. When the
    # database already holds entries we note the highest known id, probe the
    # production list with the smallest possible request (pageSize=1) to read the
    # total size, and - only when there is a gap - fetch the difference instead of
    # a full sync. A broken probe falls back to the full list fetch.
    if ($script:cfg['RefreshAccountList'] -or -not $script:state.accountListFetchedAt) {
        $dbSummary = Get-CrefoDatabaseAccountSummary
        $knownCount = if ($dbSummary -and $null -ne $dbSummary.count) { [int]$dbSummary.count } else { 0 }
        $knownMaxId = if ($dbSummary -and $null -ne $dbSummary.highest_id) { [int]$dbSummary.highest_id } else { $null }
        Write-CrefoInfo ("Debtor list check: database holds {0} known account(s), highest id {1}." -f $knownCount, $knownMaxId)

        if ($knownCount -eq 0) {
            # No database entries yet - retrieve the whole list as always.
            Write-CrefoInfo 'Database has no accounts yet - fetching the full debitor list.'
            $accounts = Get-CrefoAccounts -Config $script:cfg -AccessToken $script:token -PageSize $script:cfg['PageSize'] -AuthRefresher $script:authRefresher
            Write-CrefoInfo ("Found {0} debitor account(s)." -f @($accounts).Count)
        }
        else {
            try {
                # Probe the production size with a pageSize=1 request, compare it to
                # how many accounts we already have, and fetch only the surplus.
                $probe = Get-CrefoDebtorListStats -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher
                $gap = [int]$probe.TotalItems - $knownCount
                if ($gap -le 0) {
                    Write-CrefoInfo ("Debtor list unchanged ({0} accounts); keeping cached list, skipping full sync." -f $knownCount)
                    $accounts = @()
                }
                else {
                    Write-CrefoInfo ("Debtor list grew by {0} (production {1} vs. database {2}); fetching only the difference." -f $gap, $probe.TotalItems, $knownCount)
                    $accounts = Get-CrefoAccounts -Config $script:cfg -AccessToken $script:token -PageSize $script:cfg['PageSize'] -AuthRefresher $script:authRefresher -StartIndex $knownCount -MaxCount $gap
                    Write-CrefoInfo ("Delta from account list: {0} new account(s)." -f @($accounts).Count)
                }
            }
            catch {
                # Probe or delta failed (server hiccup / weird response): degrade to
                # the safe full-list behaviour rather than missing new debtors.
                Write-CrefoWarn ("Account list probe failed ({0}); falling back to full list sync." -f $_.Exception.Message)
                $accounts = Get-CrefoAccounts -Config $script:cfg -AccessToken $script:token -PageSize $script:cfg['PageSize'] -AuthRefresher $script:authRefresher
                Write-CrefoInfo ("Found {0} debitor account(s)." -f @($accounts).Count)
            }
        }
        Merge-CrefoAccounts -State $script:state -Accounts $accounts | Out-Null
        $script:state.accountListFetchedAt = (Get-Date).ToUniversalTime().ToString('o')
        Save-CrefoState -Path $script:statePath -State $script:state
    }
}