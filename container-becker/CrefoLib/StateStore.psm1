# =============================================================================
# StateStore.psm1 - persistence + resumability for the Crefo export.
# The state file (JSON) keeps:
#   - the account list snapshot (id, name) so we do not forget existing debtors
#   - per-account progress (status: pending / done / failed, error, updatedAt)
# This is what lets a run resume where it left off without duplicating data.
# =============================================================================

function Get-CrefoState {
    [CmdletBinding()]
    param([string]$Path)   # path of the state JSON file
    if (Test-Path -LiteralPath $Path) {
        try {
            $state = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
            # Older/partial state files may lack some fields; backfill them so
            # the rest of the code can rely on a consistent shape.
            if ($null -eq $state.accounts -or $null -eq $state.accountListFetchedAt) {
                if ($null -eq $state.accounts) {
                    $state | Add-Member -NotePropertyName accounts -NotePropertyValue @() -Force
                }
                if ($null -eq $state.accountListFetchedAt) {
                    $state | Add-Member -NotePropertyName accountListFetchedAt -NotePropertyValue $null -Force
                }
            }
            return $state
        }
        catch {
            # Corrupt state should never block a run - warn and start fresh.
            Write-CrefoWarn ("State file '{0}' could not be read, starting fresh: {1}" -f $Path, $_.Exception.Message)
        }
    }
    # Default empty-state object for the very first run.
    return [PSCustomObject]@{
        version             = 1
        updatedAt           = $null
        accountListFetchedAt = $null
        accounts            = @()
    }
}

function Save-CrefoState {
    [CmdletBinding()]
    param(
        [string]$Path,      # path of the state JSON file
        [object]$State      # the state object to serialize
    )
    $dir = Split-Path -Path $Path -Parent
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    # Stamp the write time so the file doubles as a "last activity" record.
    $State.updatedAt = (Get-Date).ToUniversalTime().ToString('o')
    $State | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $Path -Encoding UTF8
}

function Merge-CrefoAccounts {
    [CmdletBinding()]
    param(
        [object]$State,        # state object that receives the merged accounts
        [object[]]$Accounts    # accounts freshly fetched from the API
    )
    # Index existing accounts by id so we can upsert quickly.
    $existing = @{}
    foreach ($account in @($State.accounts)) {
        if ($null -ne $account -and $null -ne $account.id) {
            $existing[[int]$account.id] = $account
        }
    }

    foreach ($account in @($Accounts)) {
        if ($null -eq $account -or $null -eq $account.id) { continue }
        $id = [int]$account.id
        if ($existing.ContainsKey($id)) {
            # Already known: keep its progress, but refresh the display name
            # in case the company was renamed.
            $current = $existing[$id]
            if ([string]$current.name -ne [string]$account.name) {
                $current.name = $account.name
            }
        }
        else {
            # New debtor: add as 'pending' so it is picked up for processing.
            $existing[$id] = [PSCustomObject]@{
                id        = $id
                name      = [string]$account.name
                status    = 'pending'
                error     = $null
                updatedAt = $null
            }
        }
    }

    # Flatten back to a sorted array (sorted by id for deterministic processing order).
    $State.accounts = @($existing.Values | Sort-Object -Property id)
    return $State
}

function Get-CrefoPendingAccounts {
    [CmdletBinding()]
    param([object]$State)
    # Everything that is not 'done' (i.e. pending or previously failed) is retried.
    return @(@($State.accounts) | Where-Object { $_.status -ne 'done' } | Sort-Object -Property id)
}

function Reset-CrefoAccounts {
    [CmdletBinding()]
    param([object]$State)
    # Marks every account as pending again (used with the -Reset switch).
    foreach ($account in @($State.accounts)) {
        $account.status = 'pending'
        $account.error = $null
        $account.updatedAt = $null
    }
    return $State
}

Export-ModuleMember -Function 'Get-CrefoState', 'Save-CrefoState', 'Merge-CrefoAccounts', 'Get-CrefoPendingAccounts', 'Reset-CrefoAccounts'