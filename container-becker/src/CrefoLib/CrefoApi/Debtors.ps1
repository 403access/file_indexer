# =============================================================================
# CrefoApi/Debtors.ps1 - risk / limit data for debtor accounts.
# Get-CrefoDebtorRisk fetches the per-debtor limit/purchase snapshot (each
# call archived under risks/<outer>/<inner>/ buckets). Get-CrefoLastLimitDecisions
# and Get-CrefoOpenLimitDesires are the cheap bulk "has a limit context"
# endpoints used to short-circuit accounts that need no /risk request.
# =============================================================================

function Get-CrefoDebtorRisk {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [int]$DebtorId,                     # debtor account id
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = '',              # optional override for the archive folder label
        [int]$ArchiveRangeStep = 1000,          # outer bucket size for the archive id-range grouping
        [int]$ArchiveInnerStep = 100            # inner bucket size nested inside the outer bucket
    )
    # GET /api/v1/DebitorAccounts/{debitor}/risk returns an array; take the
    # first element since we query one specific debtor.
    $path = ('/api/v1/DebitorAccounts/{0}/risk' -f $DebtorId)
    if ([string]::IsNullOrWhiteSpace($ArchiveName)) {
        $ArchiveName = ('debtor-{0}-risk' -f $DebtorId)
    }
    # Risk archives are bucketed under risks/<outer-range>/<inner-range>/ by
    # debtor id, e.g. outer step 1000 + inner step 100 puts id 1234 into
    # risks/1000-1999/1200-1299/.
    $outerStart = [int][math]::Floor($DebtorId / $ArchiveRangeStep) * $ArchiveRangeStep
    $innerStart = [int][math]::Floor($DebtorId / $ArchiveInnerStep) * $ArchiveInnerStep
    $category = ('risks/{0}-{1}/{2}-{3}' -f `
        $outerStart, ($outerStart + $ArchiveRangeStep - 1), `
        $innerStart, ($innerStart + $ArchiveInnerStep - 1))
    $response = Invoke-CrefoApi -Config $Config -Method GET -Path $path `
        -AccessToken $AccessToken -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName `
        -ArchiveCategory $category
    return @($response)[0]
}

function Get-CrefoLastLimitDecisions {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'last-limit-decisions'   # archive folder label
    )
    # GET /api/v1/last-limit-decisions returns the current completed limit
    # decision per debtor account (single call, no pagination). It is used as a
    # bulk source of 'has a limit or not' so accounts without any decision can
    # skip the per-debtor /risk request entirely.
    $response = Invoke-CrefoApi -Config $Config -Method GET `
        -Path '/api/v1/last-limit-decisions' `
        -AccessToken $AccessToken -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName
    return @($response)
}

function Get-CrefoOpenLimitDesires {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'open-limit-desires'   # archive folder label
    )
    # GET /api/v1/open-limit-desires returns the current open (undecided /
    # in-progress) limit requests. Combined with last-limit-decisions it forms
    # the complete set of accounts with a live limit context: accounts in the
    # open pipeline can have purchases even before a decision is completed, so
    # they must never be short-circuited to a zero row.
    $response = Invoke-CrefoApi -Config $Config -Method GET `
        -Path '/api/v1/open-limit-desires' `
        -AccessToken $AccessToken -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName
    return @($response)
}