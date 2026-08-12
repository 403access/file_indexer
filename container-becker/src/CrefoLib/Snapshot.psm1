# =============================================================================
# Snapshot.psm1 - account risk-snapshot handling for the daily sync:
#   - Set-AccountField / Set-AccountSnapshot stamp the last known /risk values
#     onto an account object (so a CSV row can be re-emitted without a call)
#   - Get-RefreshDecision / Test-ShouldRefreshRisk decide whether an account's
#     snapshot must be re-fetched this run (change detection, staleness cap)
# =============================================================================

# Adds/updates a field on an account object. State accounts are deserialized
# JSON (or literal PSCustomObject) and only accept assignment to existing
# properties; new snapshot fields must be added as note properties.
function Set-AccountField {
    [CmdletBinding()]
    param(
        [object]$Account,
        [string]$Name,
        [object]$Value
    )
    $Account | Add-Member -NotePropertyName $Name -NotePropertyValue $Value -Force
}

# Stores the last known risk values onto the account object so a CSV row can be
# re-emitted later without a network call. With -Risk $null (the short-circuit
# path) the account is stamped as the explicit N / zero snapshot.
function Set-AccountSnapshot {
    [CmdletBinding()]
    param(
        [object]$Account,
        [object]$Risk
    )
    if ($null -ne $Risk) {
        Set-AccountField -Account $Account -Name 'limit' -Value ([double]$Risk.limit)
        Set-AccountField -Account $Account -Name 'purchased' -Value ([double]$Risk.purchasedReceivables)
        Set-AccountField -Account $Account -Name 'balance' -Value ([double]$Risk.balance)
        $code = [string]$Risk.limitCode
        if ([string]::IsNullOrWhiteSpace($code)) { $code = 'N' }
        Set-AccountField -Account $Account -Name 'limitCode' -Value $code
        Set-AccountField -Account $Account -Name 'riskFetched' -Value $true
        Set-AccountField -Account $Account -Name 'fetchedAt' -Value ((Get-Date).ToUniversalTime().ToString('o'))
    }
    else {
        # No risk request made (short-circuit): explicit N / zero snapshot.
        Set-AccountField -Account $Account -Name 'limit' -Value 0.0
        Set-AccountField -Account $Account -Name 'purchased' -Value 0.0
        Set-AccountField -Account $Account -Name 'balance' -Value 0.0
        Set-AccountField -Account $Account -Name 'limitCode' -Value 'N'
        Set-AccountField -Account $Account -Name 'riskFetched' -Value $false
    }
    return $Account
}

# Decides whether the account's /risk snapshot must be re-fetched for this run.
# Returns [pscustomobject]@{ ShouldRefresh, Reason } so the caller can log why.
function Get-RefreshDecision {
    [CmdletBinding()]
    param(
        [hashtable]$Cfg,
        [object]$Account,
        [bool]$HasDecision,
        [object]$Decision,
        [bool]$InOpenDesires,
        [bool]$DecisionsChanged = $true,
        [object[]]$ForceRanges = @()   # debtor id ranges that must be re-fetched
    )
    $D = {
        param([bool]$Refresh, [string]$Reason)
        return [pscustomobject]@{ ShouldRefresh = $Refresh; Reason = $Reason }
    }

    # Explicit refetch ranges win over every other consideration: an account
    # whose id falls in a listed range is always re-fetched this run.
    foreach ($range in @($ForceRanges)) {
        if ([int]$Account.id -ge [int]$range.Min -and [int]$Account.id -le [int]$range.Max) {
            return & $D $true ("refetch range {0}-{1}" -f $range.Min, $range.Max)
        }
    }

    # New or previously failed accounts are always fetched.
    if ($Account.status -in @('pending', 'failed')) {
        return & $D $true ("status '{0}'" -f $Account.status)
    }

    # Accounts from a run before the snapshot feature have no stored data yet:
    # fetch only when they have a live limit context, otherwise they fall into
    # the short-circuit path below.
    if ($null -eq $Account.limitCode) {
        if ($HasDecision -or $InOpenDesires) {
            return & $D $true 'fresh account, live limit context (decision or open desire)'
        }
        return & $D $false 'fresh account, no limit context (short-circuit row)'
    }

    # RefreshAll: refetch every account with a limit context each run.
    if ($Cfg['SyncMode'] -eq 'RefreshAll') {
        if ($HasDecision -or $InOpenDesires) {
            return & $D $true 'RefreshAll mode, account has limit context'
        }
        return & $D $false 'RefreshAll mode, no limit context (short-circuit row)'
    }

    # --- Incremental mode below -------------------------------------------------
    $storedCode = [string]$Account.limitCode
    $storedLimit = [double]$Account.limit
    if ([string]::IsNullOrWhiteSpace($storedCode)) { $storedCode = 'N' }
    $storedActive = ($storedCode -ne 'N') -or ($storedLimit -gt 0.0)

    # Decision removed while the account previously had one: refetch, because
    # purchases may still exist and only /risk knows. Skip this when the bulk
    # decisions set is unchanged since the last run (resumability guard).
    if (-not $HasDecision -and -not $InOpenDesires -and $storedActive -and $DecisionsChanged) {
        return & $D $true 'completed decision removed, account previously had a limit'
    }

    if ($HasDecision) {
        # Limit/decision changed since our last snapshot?
        $decisionCode = [string]$Decision.limitCode
        if ([string]::IsNullOrWhiteSpace($decisionCode)) { $decisionCode = 'N' }
        $decisionLimit = [double]$Decision.currentLimit
        if ($storedCode -ne $decisionCode) {
            return & $D $true ("limit code changed ({0} -> {1})" -f $storedCode, $decisionCode)
        }
        if ([math]::Abs($storedLimit - $decisionLimit) -gt 0.001) {
            return & $D $true ("current limit changed ({0:N2} -> {1:N2})" -f $storedLimit, $decisionLimit)
        }
    }
    else {
        # No decision, but the account sits in the open-limit pipeline: refresh.
        if ($InOpenDesires) {
            return & $D $true 'no completed decision, but account is in the open limit pipeline'
        }
    }

    # Staleness cap: refetch everything past MaxAgeDays regardless of changes.
    $maxAgeDays = [int]$Cfg['MaxAgeDays']
    if ($maxAgeDays -gt 0 -and $Account.riskFetched) {
        try {
            $fetchedUtc = [datetime]$Account.fetchedAt
            if ($fetchedUtc.Kind -ne 'Utc') { $fetchedUtc = $fetchedUtc.ToUniversalTime() }
            $age = (Get-Date).ToUniversalTime() - $fetchedUtc
            if ($age.TotalDays -ge $maxAgeDays) {
                return & $D $true ("snapshot older than MaxAgeDays ({0:N1} days)" -f $age.TotalDays)
            }
        }
        catch { }
    }
    return & $D $false 'snapshot fresh, decision unchanged'
}

# Boolean convenience wrapper (returns just the fetch/no-fetch decision).
function Test-ShouldRefreshRisk {
    [CmdletBinding()]
    param(
        [hashtable]$Cfg,
        [object]$Account,
        [bool]$HasDecision,
        [object]$Decision,
        [bool]$InOpenDesires,
        [bool]$DecisionsChanged = $true,
        [object[]]$ForceRanges = @()
    )
    return (Get-RefreshDecision -Cfg $Cfg -Account $Account -HasDecision $HasDecision -Decision $Decision -InOpenDesires $InOpenDesires -DecisionsChanged $DecisionsChanged -ForceRanges $ForceRanges).ShouldRefresh
}

Export-ModuleMember -Function 'Set-AccountField', 'Set-AccountSnapshot', 'Get-RefreshDecision', 'Test-ShouldRefreshRisk'